use std::{collections::VecDeque, time::Instant};

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::{
    Activation, AdamW, Module, Optimizer, Sequential, VarBuilder, VarMap, linear, seq,
};
use rand::{Rng, distr::Uniform};

use crate::game::ai::{
    ACTION_SPACE, Actions, DuelFloat, STATE_VECTOR_LEN, copy_var_map, env::Environment, save_model,
};

const AGENT1_OUTPUT_PATH: &str = "./resources/scenes/dqn_agent1_weights_NEW.safetensors";
const AGENT2_OUTPUT_PATH: &str = "./resources/scenes/dqn_agent2_weights_NEW.safetensors";
const SAVE_INTERVAL: usize = 5000;

const HIDDEN_COUNT: usize = 256;
const LEARNING_RATE: f64 = 0.0001;
const EPISODES: usize = 25_000;
const BATCH_SIZE: usize = 256;
/// Number of steps before copying over agent to target network
const TARGET_UPDATE_INTERVAL: usize = BATCH_SIZE * 64;
const REPLAY_SIZE: usize = BATCH_SIZE * 256;
const GAMMA: f64 = 0.99;
const START_E: f64 = 0.8;
const END_E: f64 = 0.05;
const EPSILON_RANGE: usize = EPISODES;
const EPISODE_PRINT_STEP: usize = EPISODES / 1_000;

type GameMemory = (Tensor, Actions, DuelFloat, bool, Tensor); // Init state, actions, reward, terminal_inverse, next state
struct ReplayMemory {
    memory: VecDeque<GameMemory>,
    count: usize,
}

impl ReplayMemory {
    pub fn new() -> Self {
        Self {
            memory: VecDeque::with_capacity(REPLAY_SIZE),
            count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.memory.len()
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn reset_count(&mut self) {
        self.count = 0;
    }

    pub fn append(&mut self, new_memory: GameMemory) {
        self.count += 1;
        self.memory.push_front(new_memory);
        if self.memory.len() > REPLAY_SIZE {
            self.memory.pop_back();
        }
    }

    pub fn get(&self, index: usize) -> GameMemory {
        let mem = self.memory.get(index).unwrap();
        (mem.0.clone(), mem.1, mem.2, mem.3, mem.4.clone())
    }
}

#[allow(dead_code)]
pub fn train(mut env: Environment<'_>, device: Device, start: Instant) -> Result<()> {
    let mut rng = rand::rng();

    let var_map1 = VarMap::new();
    let var_map2 = VarMap::new();
    let agent1 = make_model(&var_map1, &device)?;
    let agent2 = make_model(&var_map2, &device)?;

    let mut target_var_map1 = VarMap::new();
    let mut target_var_map2 = VarMap::new();
    let target_agent1 = make_model(&target_var_map1, &device)?;
    let target_agent2 = make_model(&target_var_map2, &device)?;

    let mut optimizer1 = AdamW::new_lr(var_map1.all_vars(), LEARNING_RATE)?;
    let mut optimizer2 = AdamW::new_lr(var_map2.all_vars(), LEARNING_RATE)?;

    let mut replay_memory = ReplayMemory::new();

    let mut episode = 0;
    let mut step = 0;
    let mut observation = env.obs(&device)?;

    while episode < EPISODES {
        let epsilon = get_epsilon(episode);
        let action1 = get_ai_action(&mut rng, &agent1, &observation, epsilon)?;
        let action2 = get_ai_action(&mut rng, &agent1, &observation, epsilon)?;

        let (terminal, rewards) = env.step((action1, action2));

        let start_state = observation;
        observation = env.obs(&device)?;
        let new_memory = (
            start_state,
            Actions {
                agent1: action1,
                agent2: action2,
            },
            rewards,
            !terminal,
            observation.clone(),
        );
        replay_memory.append(new_memory);
        step += 1;

        if step % TARGET_UPDATE_INTERVAL == 0 {
            copy_var_map(&var_map1, &mut target_var_map1)?;
            copy_var_map(&var_map2, &mut target_var_map2)?;
        }

        if replay_memory.len() >= REPLAY_SIZE && replay_memory.count() >= BATCH_SIZE {
            train_agents(
                &mut rng,
                &device,
                &agent1,
                &target_agent1,
                &agent2,
                &target_agent2,
                &mut optimizer1,
                &mut optimizer2,
                &replay_memory,
            )?;

            replay_memory.reset_count();
        }

        if terminal {
            episode += 1;

            if episode % EPISODE_PRINT_STEP == 0 {
                env.display(episode, start.elapsed());
            }

            // Reset Stuff
            env.reset();

            if episode % SAVE_INTERVAL == 0 {
                save_model(&var_map1, AGENT1_OUTPUT_PATH)?;
                save_model(&var_map2, AGENT2_OUTPUT_PATH)?;
                println!("NOTE: Saved at checkpoint episode: {episode}");
            }
        }
    }

    save_model(&var_map1, AGENT1_OUTPUT_PATH)?;
    save_model(&var_map2, AGENT2_OUTPUT_PATH)?;
    println!("Total steps: {step}");

    Ok(())
}

fn get_epsilon(episode: usize) -> f64 {
    START_E - (START_E - END_E) * (episode as f64 / EPSILON_RANGE as f64).min(1.0)
}

fn train_agents(
    rng: &mut rand::rngs::ThreadRng,
    device: &Device,
    agent1: &candle_nn::Sequential,
    target_agent1: &candle_nn::Sequential,
    agent2: &candle_nn::Sequential,
    target_agent2: &candle_nn::Sequential,
    optimizer1: &mut AdamW,
    optimizer2: &mut AdamW,
    memory: &ReplayMemory,
) -> Result<()> {
    let batch = rng
        .sample_iter(Uniform::new(0, REPLAY_SIZE).expect("Bad uniform range"))
        .take(BATCH_SIZE)
        .map(|i| memory.get(i))
        .collect::<Vec<_>>();

    let states = batch.iter().map(|e| &e.0).collect::<Vec<_>>();
    let states = Tensor::stack(&states, 0)?;

    let agent1_actions =
        Tensor::from_iter(batch.iter().map(|e| e.1.agent1), device)?.unsqueeze(1)?;
    let agent2_actions =
        Tensor::from_iter(batch.iter().map(|e| e.1.agent2), device)?.unsqueeze(1)?;
    let agent1_rewards =
        Tensor::from_iter(batch.iter().map(|e| e.2.agent1), device)?.unsqueeze(1)?;
    let agent2_rewards =
        Tensor::from_iter(batch.iter().map(|e| e.2.agent2), device)?.unsqueeze(1)?;
    let non_final_mask =
        Tensor::from_iter(batch.iter().map(|e| e.3 as usize as f32), device)?.unsqueeze(1)?;

    let next_states = batch.iter().map(|e| &e.4).collect::<Vec<_>>();
    let next_states = Tensor::stack(&next_states, 0)?;

    train_single_agent(
        agent1,
        target_agent1,
        &states,
        &agent1_actions,
        &agent1_rewards,
        &next_states,
        &non_final_mask,
        optimizer1,
    )?;
    train_single_agent(
        agent2,
        target_agent2,
        &states,
        &agent2_actions,
        &agent2_rewards,
        &next_states,
        &non_final_mask,
        optimizer2,
    )?;

    Ok(())
}

fn train_single_agent(
    agent: &candle_nn::Sequential,
    target_agent: &candle_nn::Sequential,
    states: &Tensor,
    actions: &Tensor,
    rewards: &Tensor,
    next_states: &Tensor,
    non_final_mask: &Tensor,
    optimizer: &mut AdamW,
) -> Result<()> {
    let estimated_rewards = agent.forward(states)?;
    let x = estimated_rewards.gather(actions, 1)?;
    let expected_rewards = target_agent.forward(next_states)?.detach();
    let y = expected_rewards.max_keepdim(1)?;
    let y = (y * GAMMA * non_final_mask + rewards)?;
    let loss = candle_nn::loss::mse(&x, &y)?;
    optimizer.backward_step(&loss)?;
    Ok(())
}

fn get_ai_action(
    rng: &mut rand::rngs::ThreadRng,
    agent: &candle_nn::Sequential,
    obs: &Tensor,
    epsilon: f64,
) -> Result<u32> {
    let ai_action = if rng.random::<f64>() < epsilon {
        rng.random_range(0..ACTION_SPACE as u32)
    } else {
        let agent_est = agent.forward(&obs.unsqueeze(0)?)?;
        agent_est.squeeze(0)?.argmax(0)?.to_scalar()?
    };

    Ok(ai_action)
}

pub fn make_model(var_map: &VarMap, device: &Device) -> Result<Sequential> {
    let vb = VarBuilder::from_varmap(var_map, DType::F32, device);

    let agent1 = seq()
        .add(linear(STATE_VECTOR_LEN, HIDDEN_COUNT, vb.pp("linear_in"))?)
        .add(Activation::Relu)
        .add(linear(HIDDEN_COUNT, HIDDEN_COUNT, vb.pp("hidden"))?)
        .add(Activation::Relu)
        .add(linear(HIDDEN_COUNT, ACTION_SPACE, vb.pp("linear_out"))?);

    Ok(agent1)
}
