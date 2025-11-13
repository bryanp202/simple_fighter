use candle_core::{D, DType, Device, IndexOp, Result, Tensor};
use candle_nn::{
    Activation, AdamW, Module, Optimizer, Sequential, VarBuilder, VarMap, linear, ops::softmax, seq
};
use rand::{Rng, distr::weighted::WeightedIndex, rngs::ThreadRng};

use crate::game::{
    GameContext, GameState, PlayerInputs,
    ai::{
        ACTION_SPACE, DuelFloat, STATE_VECTOR_LEN, env_step, map_ai_action,
        save_model, serialize_observation,
    },
    input::{InputHistory, Inputs},
    scene::gameplay::during_round::DuringRound,
};

const AGENT1_OUTPUT_PATH: &str = "./resources/scenes/ppo_agent1_weights_NEW.safetensors";
const AGENT2_OUTPUT_PATH: &str = "./resources/scenes/ppo_agent2_weights_NEW.safetensors";
const SAVE_INTERVAL: usize = 5000;

const EPOCHS: usize = 1_000;
const STEPS_PER_EPOCH: usize = 6_000;
const HIDDEN_COUNT: usize = 256;
const LEARNING_RATE_ACTOR: f64 = 0.0003;
const LEARNING_RATE_CRITIC: f64 = 0.001;
const GAMMA: f32 = 0.99;
const K_EPOCHS: usize = 80;
const EPS_CLIP: f32 = 0.2;
const GAE_LAMBDA: f32 = 0.95;
const ENTROPY_COEF: f32 = 0.01;
const VF_COEF: f32 = 0.5;

const EPOCH_PRINT_STEP: usize = EPOCHS / 100;

#[derive(Default)]
struct RolloutBuffer {
    states: Vec<Tensor>,

    actions_1: Vec<u32>,
    actions_2: Vec<u32>,

    logprobs_1: Vec<f32>,
    logprobs_2: Vec<f32>,

    rewards_1: Vec<f32>,
    rewards_2: Vec<f32>,

    state_values_1: Vec<f32>,
    state_values_2: Vec<f32>,

    advantage_1: Vec<f32>,
    advantage_2: Vec<f32>,

    return_1: Vec<f32>,
    return_2: Vec<f32>,

    path_start_idx: usize,
    ptr: usize,
}

impl RolloutBuffer {
    fn new() -> Self {
        Self::default()
    }

    fn finish_path(&mut self, value1: f32, value2: f32) {
        let path_range = self.path_start_idx..self.ptr;

        // Agent1
        let rews = &self.rewards_1[path_range.clone()];
        let vals = &self.state_values_1[path_range.clone()];
        compute_gae(&mut self.advantage_1, rews, vals, value1);
        compute_return(&mut self.return_1, rews, value1);

        // Agent2
        let rews = &self.rewards_2[path_range.clone()];
        let vals = &self.state_values_2[path_range];
        compute_gae(&mut self.advantage_2, rews, vals, value2);
        compute_return(&mut self.return_2, rews, value2);

        self.path_start_idx = self.ptr;
    }

    fn get(&mut self) {
        self.actions_1.clear();
        self.actions_2.clear();
        self.logprobs_1.clear();
        self.logprobs_2.clear();
        self.rewards_1.clear();
        self.rewards_2.clear();
        self.state_values_1.clear();
        self.state_values_2.clear();
        self.return_1.clear();
        self.return_2.clear();
        self.advantage_1.clear();
        self.advantage_2.clear();
        self.states.clear();
    }

    fn push_env(&mut self, state: Tensor, rewards: DuelFloat) {
        self.states.push(state);
        self.rewards_1.push(rewards.agent1);
        self.rewards_2.push(rewards.agent2);

        self.ptr += 1;
    }

    fn push_agent1(&mut self, action: u32, logprob: f32, state_val: f32) {
        self.actions_1.push(action);
        self.logprobs_1.push(logprob);
        self.state_values_1.push(state_val);
    }

    fn push_agent2(&mut self, action: u32, logprob: f32, state_val: f32) {
        self.actions_2.push(action);
        self.logprobs_2.push(logprob);
        self.state_values_2.push(state_val);
    }
}

struct ActorCritic {
    actor: Sequential,
    critic: Sequential,
}

impl ActorCritic {
    /// (ActorCritic, ActorMap, CriticMap)
    fn new(device: &Device) -> Result<(Self, VarMap, VarMap)> {
        let actor_map = VarMap::new();
        let critic_map = VarMap::new();

        let actor_vb = VarBuilder::from_varmap(&actor_map, DType::F32, device);
        let critic_vb = VarBuilder::from_varmap(&critic_map, DType::F32, device);
        let ac = Self {
            actor: seq()
                .add(linear(STATE_VECTOR_LEN, HIDDEN_COUNT, actor_vb.pp("actor_in"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, HIDDEN_COUNT, actor_vb.pp("actor_hidden"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, ACTION_SPACE, actor_vb.pp("actor_out"))?),
            critic: seq()
                .add(linear(STATE_VECTOR_LEN, HIDDEN_COUNT, critic_vb.pp("critic_in"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, HIDDEN_COUNT, critic_vb.pp("critic_hidden"))?)
                .add(Activation::Sigmoid)
                .add(linear(HIDDEN_COUNT, 1, critic_vb.pp("critic_out"))?),
        };

        Ok((ac, actor_map, critic_map))
    }

    /// Action, state_val, logp_a
    fn step(&self, obs: &Tensor, rng: &mut rand::rngs::ThreadRng) -> Result<(u32, f32, f32)> {
        let estimates = self.actor.forward(&obs.unsqueeze(0)?)?;
        let action_probs = softmax(&estimates.squeeze(0)?, 0)?;
        let weights = action_probs.to_vec1::<f32>()?;
        let action = rng.sample(WeightedIndex::new(weights).unwrap());

        let state_val = self.critic.forward(obs)?.to_scalar()?;

        let logp_a = action_probs.i(action)?.to_scalar::<f32>()?.ln();

        Ok((action as u32, state_val, logp_a))
    }

    /// Action, state_val, logp_a
    fn act(&self, obs: &Tensor, rng: &mut rand::rngs::ThreadRng) -> Result<u32> {
        let estimates = self.actor.forward(&obs.unsqueeze(0)?)?;
        let action_probs = softmax(&estimates.squeeze(0)?, 0)?;
        let weights = action_probs.to_vec1::<f32>()?;
        let action = rng.sample(WeightedIndex::new(weights).unwrap());

        Ok(action as u32)
    }

    /// Prob distributions for each state, logp for each action
    fn pi(&self, obs_batch: &Tensor, actions: &Tensor) -> Result<(Tensor, Tensor)> {
        let estimates = self.actor.forward(obs_batch)?;
        let action_probs = softmax(&estimates.squeeze(0)?, 0)?;
        
        let logp_a = action_probs.gather(actions, 1)?.log()?.squeeze(D::Minus1)?;

        Ok((action_probs, logp_a))
    }

    /// State values for each obs
    fn v(&self, obs_batch: &Tensor) -> Result<Tensor> {
        self.critic.forward(obs_batch)?.squeeze(D::Minus1)
    }
}

struct PPOAgent {
    // Current Policy
    policy: ActorCritic,
    actor_map: VarMap,
    critic_map: VarMap,
    actor_optimizer: AdamW,
    critic_optimizer: AdamW,
}

impl PPOAgent {
    fn new(device: &Device) -> Result<Self> {
        let (policy, actor_map, critic_map) = ActorCritic::new(device)?;

        let actor_optimizer = AdamW::new_lr(actor_map.all_vars(), LEARNING_RATE_ACTOR)?;
        let critic_optimizer = AdamW::new_lr(critic_map.all_vars(), LEARNING_RATE_CRITIC)?;

        Ok(Self {
            policy,
            actor_map,
            critic_map,
            actor_optimizer,
            critic_optimizer,
        })
    }

    fn save(&self, filename: &str) -> Result<()> {
        println!("Saved policy to file: {filename}");
        save_model(&self.actor_map, filename)
    }
}

#[allow(dead_code)]
pub fn train(
    context: &GameContext,
    inputs: &mut PlayerInputs,
    state: &mut GameState,
) -> Result<()> {
    let start = std::time::Instant::now();
    let device = Device::Cpu;
    let mut agent1 = PPOAgent::new(&device)?;
    let mut agent2 = PPOAgent::new(&device)?;

    let mut scene = DuringRound::new((0, 0));
    let mut rng = rand::rng();
    let mut buffer = RolloutBuffer::new();

    let mut agent1_last_hit = 0;
    let mut agent2_last_hit = 0;
    let mut accumulate_rewards = DuelFloat::default();

    for epoch in 0..EPOCHS {
        for step in 0..STEPS_PER_EPOCH {
            let observation = serialize_observation(&device, scene.timer(), context, state)?;
            take_agent_turns(
                inputs,
                state,
                &agent1,
                &agent2,
                &mut buffer,
                &observation,
                &mut rng,
            )?;

            // Update environment
            let (terminal, rewards) = env_step(
                context,
                state,
                &mut scene,
                &mut agent1_last_hit,
                &mut agent2_last_hit,
            );
            buffer.push_env(observation, rewards);
            accumulate_rewards.agent1 += rewards.agent1;
            accumulate_rewards.agent2 += rewards.agent2;

            if terminal || step == STEPS_PER_EPOCH - 1 {
                let (v1, v2) = if !terminal {
                    let last_obs = serialize_observation(&device, scene.timer(), context, state)?;
                    let (_, v1, _) = agent1.policy.step(&last_obs, &mut rng)?;
                    let (_, v2, _) = agent2.policy.step(&last_obs, &mut rng)?;
                    (v1, v2)
                } else {
                    (0.0, 0.0)
                };

                buffer.finish_path(v1, v2);

                // Reset Stuff
                accumulate_rewards = DuelFloat::default();
                agent1_last_hit = 0;
                agent2_last_hit = 0;
                scene = DuringRound::new((0, 0));
                state.reset(context);
                inputs.reset_player1();
                inputs.reset_player2();
                state.player1_inputs.reset();
                state.player2_inputs.reset();
            }
        }

        if epoch % EPOCH_PRINT_STEP == 0 {
            println!("___________________________");
            println!("EPOCH: {epoch}, TIME: {:?}", start.elapsed());
            println!("Accumulate game sum: {accumulate_rewards:?}");
            println!("Round timer: {}", 1.0 - scene.timer());
            println!("Agent1: {:?}", state.player1);
            println!("~~~~~~~~~~~~~~~~~~~~~~~~~~~");
            println!("Agent2: {:?}", state.player2);
            println!("___________________________\n");
        }

        if epoch % SAVE_INTERVAL == 0 {
            agent1.save(AGENT1_OUTPUT_PATH)?;
            agent2.save(AGENT2_OUTPUT_PATH)?;
        }

        update_agents(&mut agent1, &mut agent2, &mut buffer, &device)?;
    }

    println!("Completed in {:?} secs", start.elapsed());
    agent1.save(AGENT1_OUTPUT_PATH)?;
    agent2.save(AGENT2_OUTPUT_PATH)?;
    Ok(())
}

fn take_agent_turns(
    inputs_history: &mut PlayerInputs,
    state: &mut GameState,
    agent1: &PPOAgent,
    agent2: &PPOAgent,
    buffer: &mut RolloutBuffer,
    observation: &Tensor,
    rng: &mut ThreadRng,
) -> Result<()> {
    let (action1, logprob, state_val) = agent1.policy.act(observation, rng)?;
    buffer.push_agent1(action1, logprob, state_val);
    let (action2, logprob, state_val) = agent2.policy.act(observation, rng)?;
    buffer.push_agent2(action2, logprob, state_val);

    take_agent_turn(
        &mut inputs_history.player1,
        &mut state.player1_inputs,
        action1,
    );
    take_agent_turn(
        &mut inputs_history.player2,
        &mut state.player2_inputs,
        action2,
    );

    Ok(())
}


fn compute_gae(
    adv_vec: &mut Vec<f32>,
    rewards: &[f32],
    state_values: &[f32],
    bootstrap: f32,
) {
    let idx = adv_vec.len();
    let len = rewards.len();
    adv_vec.resize(idx + len, 0.0);

    let mut last_advantage = rewards[len - 1] + GAMMA * bootstrap - state_values[len - 1];
    adv_vec[idx + len - 1] = last_advantage;

    for t in (0..len - 1).rev() {
        let delta = rewards[t] + GAMMA * state_values[t + 1] - state_values[t];
        last_advantage = delta + GAMMA * GAE_LAMBDA * last_advantage;
        adv_vec[idx + t] = last_advantage;
    }
}

fn compute_return(
    ret_vec: &mut Vec<f32>,
    rewards: &[f32],
    bootstrap: f32,
) {
    let idx = ret_vec.len();
    let len = rewards.len();
    ret_vec.resize(idx + len, 0.0);
    let mut last_return = bootstrap;

    for t in (0..len).rev() {
        last_return = rewards[t] + GAMMA * last_return;
        ret_vec[idx + t] = last_return;
    }
}

fn update_agents(agent1: &mut PPOAgent, agent2: &mut PPOAgent, buffer: &mut RolloutBuffer, device: &Device) -> Result<()> {
    agent1.sync_old_policy()?;
    agent2.sync_old_policy()?;

    let states = Tensor::stack(&buffer.states, 0)?.detach();
    let ent_coef = Tensor::from_iter([ENTROPY_COEF], device)?;
    let vf_coef = Tensor::from_iter([VF_COEF], device)?;

    let advantages = compute_gae(
        &buffer.rewards_1,
        &buffer.state_values_1,
        &buffer.non_terminal_mask,
        device,
    )?.detach();
    let actions =
        Tensor::from_iter(buffer.actions_1.clone(), device)?.unsqueeze(1)?.detach();
    let old_logprobs =
        Tensor::from_iter(buffer.logprobs_1.clone(), device)?.unsqueeze(1)?.detach();
    let old_state_values =
        Tensor::from_iter(buffer.state_values_1.clone(), device)?.unsqueeze(1)?.detach();

    let returns = (&advantages + &old_state_values)?.detach();

    for _ in 0..K_EPOCHS {
        let (logprobs, state_values, entropy) = agent1.policy.evaluate(&states, &actions)?;
        let state_values = state_values.squeeze(1)?;
        let ratios = (logprobs - &old_logprobs)?.exp()?;

        let adv_mean = advantages.mean_all()?;
        let adv_std = (advantages.var(0)? + 1e-8)?.sqrt()?;
        let mb_advantages = ((&advantages - adv_mean)? / adv_std)?;

        let surr1 = (&ratios * &mb_advantages)?;
        let surr2 = (ratios.clamp(1.0 - EPS_CLIP, 1.0 + EPS_CLIP)? * &mb_advantages)?;
        let pg_loss = surr1.maximum(&surr2)?.neg()?.mean_all()?;

        let v_loss = (&state_values - &returns)?.powf(2.0)?.mean_all()?;

        let entropy_loss = entropy.mean_all()?;

        let loss = ((pg_loss - (&entropy_loss * &ent_coef)?)? + (v_loss * &vf_coef))?;

        agent1.actor_optimizer.backward_step(&loss)?;
        agent1.critic_optimizer.backward_step(&loss);
    }

    Ok(())
}

pub fn get_agent_action(agent: &Sequential, obs: &Tensor, rng: &mut ThreadRng) -> Result<u32> {
    let estimates = agent.forward(&obs.unsqueeze(0)?)?;
    let action_probs = softmax(&estimates.squeeze(0)?, 0)?;
    let weights = action_probs.to_vec1::<f32>()?;
    Ok(rng.sample(WeightedIndex::new(weights).unwrap()) as u32)
}

pub fn take_agent_turn(inputs_history: &mut InputHistory, inputs: &mut Inputs, action: u32) {
    let (dir, buttons) = map_ai_action(action);

    inputs_history.skip();
    inputs_history.append_input(0, dir, buttons);

    inputs.update(
        inputs_history.held_buttons(),
        inputs_history.parse_history(),
    );
}
