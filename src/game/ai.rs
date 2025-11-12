use std::{cmp::Ordering, collections::VecDeque};

use candle_core::{DType, Device, Module, Result, Tensor};
use candle_nn::{Activation, AdamW, Optimizer, Sequential, VarBuilder, VarMap, linear, seq};
use rand::{Rng, distr::Uniform};
use sdl3::render::FPoint;

use crate::game::{
    GameContext, GameState, PlayerInputs,
    input::{ButtonFlag, Direction, InputHistory, Inputs},
    scene::gameplay::{GameplayScene, during_round::DuringRound},
};

const AGENT1_OUTPUT_PATH: &str = "./resources/scenes/dqn_agent1_weights_NEW.safetensors";
const AGENT2_OUTPUT_PATH: &str = "./resources/scenes/dqn_agent2_weights_NEW.safetensors";
const SAVE_INTERVAL: usize = 5000;

const STATE_VECTOR_LEN: usize = 35 + 35 + 3;
const HIDDEN_COUNT: usize = 256;
const ACTION_SPACE: usize = 9 * 8;
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

// REWARDS
const ROUND_WIN_SCORE: f32 = 25.0;
const ROUND_LOSE_SCORE: f32 = -0.5;
const ROUND_TIE_SCORE: f32 = -50.0;

/// Not a zero sum game
///
/// Return value as is represents the reward for agent1, and the negation is the reward for agent2
fn step_reward(
    current_frame: usize,
    agent1_start_hp: f32,
    agent1_end_hp: f32,
    agent1_start_combo: f32,
    agent1_end_combo: f32,
    agent1_last_hit_time: &mut usize,
    agent1_pos: FPoint,
    agent2_start_hp: f32,
    agent2_end_hp: f32,
    agent2_start_combo: f32,
    agent2_end_combo: f32,
    agent2_last_hit_time: &mut usize,
    agent2_pos: FPoint,
    old_score: (u32, u32),
    new_score: (u32, u32),
    timer: f32,
) -> Reward {
    let (round_rwd1, round_rwd2) = match new_score.0.cmp(&new_score.1) {
        Ordering::Less => {
            // Player 2 wins
            if agent1_end_hp <= 0.0 {
                (ROUND_LOSE_SCORE, ROUND_WIN_SCORE * (1.0 + timer) / 2.0)
            } else {
                (ROUND_LOSE_SCORE * 10.0, ROUND_WIN_SCORE / 100.0)
            }
        }
        Ordering::Equal => {
            // Tie, figure out if game still going
            if new_score.0 > old_score.0 {
                (ROUND_TIE_SCORE, ROUND_TIE_SCORE)
            } else {
                (-0.01, -0.01)
            }
        }
        Ordering::Greater => {
            // Player 1 wins
            if agent2_end_hp <= 0.0 {
                (ROUND_WIN_SCORE * (1.0 + timer) / 2.0, ROUND_LOSE_SCORE)
            } else {
                (ROUND_WIN_SCORE / 100.0, ROUND_LOSE_SCORE * 10.0)
            }
        }
    };

    if agent2_start_hp != agent2_end_hp {
        *agent1_last_hit_time = current_frame;
    }
    if agent1_start_hp != agent1_end_hp {
        *agent2_last_hit_time = current_frame;
    }
    let passive_penalty1 = (current_frame - *agent1_last_hit_time) as f32 / 100_000.0;
    let passive_penalty2 = (current_frame - *agent2_last_hit_time) as f32 / 100_000.0;

    let corner_penalty1 = ((agent1_pos.x.abs() > 400.0) as u8) as f32 / 100.0;
    let corner_penalty2 = ((agent2_pos.x.abs() > 400.0) as u8) as f32 / 100.0;

    let dmg_rwd1 = (agent2_start_hp - agent2_end_hp) * 10.0;
    let dmg_rwd2 = (agent1_start_hp - agent1_end_hp) * 10.0;

    let combo_rwd1 = (agent1_start_combo - agent1_end_combo).max(0.0) * 10.0;
    let combo_rwd2 = (agent2_start_combo - agent2_end_combo).max(0.0) * 10.0;

    let distance_reward = 1.0 / (agent1_pos.x - agent2_pos.x).abs().max(80.0);

    let (hp_rwd1, hp_rwd2) = match agent1_end_hp.total_cmp(&agent2_end_hp) {
        Ordering::Less => (0.0, 0.0005),
        Ordering::Equal => (-0.001, -0.001),
        Ordering::Greater => (0.0005, 0.0),
    };

    let agent1 = distance_reward + round_rwd1 + dmg_rwd1 + hp_rwd1 + combo_rwd1
        - passive_penalty1
        - corner_penalty1;
    let agent2 = distance_reward + round_rwd2 + dmg_rwd2 + hp_rwd2 + combo_rwd2
        - passive_penalty2
        - corner_penalty2;

    Reward { agent1, agent2 }
}

type Action = u32;
#[derive(Clone, Copy)]
struct Actions {
    agent1: Action,
    agent2: Action,
}

#[derive(Clone, Copy, Debug)]
struct Reward {
    agent1: f32,
    agent2: f32,
}

type GameMemory = (Tensor, Actions, Reward, bool, Tensor); // Init state, actions, reward, terminal_inverse, next state
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

fn save_model(var_map: &VarMap, filename: &str) -> Result<()> {
    if let Some(parent) = std::path::Path::new(filename).parent() {
        std::fs::create_dir_all(parent)?;
    }
    var_map.save(filename)?;
    println!("Model weights saved successfully to {}", filename);
    Ok(())
}

pub fn train(
    context: &GameContext,
    inputs: &mut PlayerInputs,
    state: &mut GameState,
) -> Result<()> {
    let device = Device::Cpu;
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
    let mut accumulate_rewards = Reward {
        agent1: 0.0,
        agent2: 0.0,
    };
    let mut scene = DuringRound::new((0, 0));
    let mut observation = serialize_observation(&device, scene.timer(), context, state)?;
    let mut agent1_last_hit = 0;
    let mut agent2_last_hit = 0;

    while episode < EPISODES {
        let epsilon = get_epsilon(episode);
        let agent1_action = take_agent_turn(
            &mut rng,
            &agent1,
            &mut inputs.player1,
            &mut state.player1_inputs,
            &observation,
            epsilon,
        )?;
        let agent2_action = take_agent_turn(
            &mut rng,
            &agent2,
            &mut inputs.player2,
            &mut state.player2_inputs,
            &observation,
            epsilon,
        )?;

        let old_score = scene.score();
        let agent1_start_hp = state.player1.hp_per(&context.player1);
        let agent1_combo_start = state.player1.combo_scaling();
        let agent2_start_hp = state.player2.hp_per(&context.player2);
        let agent2_combo_start = state.player2.combo_scaling();
        // Step
        let terminal = scene.update(context, state).is_some();
        let new_score = scene.score();
        let current_frame = scene.current_frame();
        let agent1_end_hp = state.player1.hp_per(&context.player1);
        let agent2_end_hp = state.player2.hp_per(&context.player2);
        let timer = scene.timer();
        let rewards = step_reward(
            current_frame,
            agent1_start_hp,
            agent1_end_hp,
            agent1_combo_start,
            state.player1.combo_scaling(),
            &mut agent1_last_hit,
            state.player1.pos(),
            agent2_start_hp,
            agent2_end_hp,
            agent2_combo_start,
            state.player2.combo_scaling(),
            &mut agent2_last_hit,
            state.player2.pos(),
            old_score,
            new_score,
            timer,
        );

        let start_state = observation;
        observation = serialize_observation(&device, scene.timer(), context, state)?;
        let new_memory = (
            start_state,
            Actions {
                agent1: agent1_action,
                agent2: agent2_action,
            },
            rewards,
            !terminal,
            observation.clone(),
        );
        replay_memory.append(new_memory);
        accumulate_rewards.agent1 += rewards.agent1;
        accumulate_rewards.agent2 += rewards.agent2;
        step += 1;

        if step % TARGET_UPDATE_INTERVAL == 0 {
            copy_target_agent(&var_map1, &mut target_var_map1)?;
            copy_target_agent(&var_map2, &mut target_var_map2)?;
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
                println!("___________________________");
                println!("EPISODE: {episode}");
                println!("Accumulate game sum: {accumulate_rewards:?}");
                println!("Round timer: {}", 1.0 - scene.timer());
                println!("Agent1: {:?}", state.player1);
                println!("---------------------------");
                println!("Agent2: {:?}", state.player2);
                println!("___________________________\n");
            }

            // Reset Stuff
            accumulate_rewards = Reward {
                agent1: 0.0,
                agent2: 0.0,
            };
            agent1_last_hit = 0;
            agent2_last_hit = 0;
            scene = DuringRound::new((0, 0));
            state.reset(context);
            inputs.reset_player1();
            inputs.reset_player2();

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

pub fn serialize_observation(
    device: &Device,
    timer: f32,
    context: &GameContext,
    state: &GameState,
) -> Result<Tensor> {
    let global_inputs = [
        timer,
        (state.player1.pos().x - state.player2.pos().x).abs() / context.stage.width(),
        (state.player1.pos().y - state.player2.pos().y).abs() / context.stage.width(),
    ];
    let agent1_state = state.player1.serialize(&context.player1, &context.stage);
    let agent2_state = state.player2.serialize(&context.player2, &context.stage);

    let state_iter = global_inputs
        .into_iter()
        .chain(agent1_state)
        .chain(agent2_state);

    Tensor::from_iter(state_iter, device)
}

fn take_agent_turn(
    rng: &mut rand::rngs::ThreadRng,
    agent: &candle_nn::Sequential,
    inputs_history: &mut InputHistory,
    inputs: &mut Inputs,
    obs: &Tensor,
    epsilon: f64,
) -> Result<Action> {
    let agent_action = get_ai_action(rng, agent, obs, epsilon)?;
    let (dir, buttons) = map_ai_action(agent_action);

    inputs_history.skip();
    inputs_history.append_input(0, dir, buttons);

    inputs.update(
        inputs_history.held_buttons(),
        inputs_history.parse_history(),
    );

    Ok(agent_action)
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
    let estimated_rewards = agent.forward(&states)?;
    let x = estimated_rewards.gather(&actions, 1)?;
    let expected_rewards = target_agent.forward(&next_states)?.detach();
    let y = expected_rewards.max_keepdim(1)?;
    let y = (y * GAMMA * non_final_mask + rewards)?;
    let loss = candle_nn::loss::mse(&x, &y)?;
    optimizer.backward_step(&loss)?;
    Ok(())
}

fn copy_target_agent(source: &VarMap, destination: &mut VarMap) -> Result<()> {
    destination.set(
        source
            .data()
            .try_lock()
            .expect("Failed to lock source varmap")
            .iter()
            .map(|(name, tensor)| (name, tensor.detach())),
    )?;

    Ok(())
}

pub fn get_ai_action(
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

pub fn map_ai_action(ai_action: u32) -> (Direction, ButtonFlag) {
    // Numpad notation -1
    let dir = match ai_action % 9 {
        0 => Direction::DownLeft,
        1 => Direction::Down,
        2 => Direction::DownRight,
        3 => Direction::Left,
        4 => Direction::Neutral,
        5 => Direction::Right,
        6 => Direction::UpLeft,
        7 => Direction::Up,
        8 => Direction::UpRight,
        _ => panic!("Math broke"),
    };
    let buttons = ButtonFlag::from_bits_retain(ai_action as u8 / 9);

    (dir, buttons)
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
