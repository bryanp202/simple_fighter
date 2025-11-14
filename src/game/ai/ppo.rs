use std::time::Instant;

use candle_core::{D, DType, Device, IndexOp, Result, Tensor};
use candle_nn::{
    Activation, AdamW, Module, Optimizer, Sequential, VarBuilder, VarMap, linear, ops::softmax, seq,
};
use rand::{Rng, distr::weighted::WeightedIndex, rngs::ThreadRng};

use crate::game::ai::{ACTION_SPACE, DuelFloat, STATE_VECTOR_LEN, env::Environment, save_model};

const AGENT1_OUTPUT_PATH: &str = "./resources/scenes/ppo_agent1_weights_NEW.safetensors";
const AGENT2_OUTPUT_PATH: &str = "./resources/scenes/ppo_agent2_weights_NEW.safetensors";
const SAVE_INTERVAL: usize = 500;

const OPPONENT_FREEZE_EPOCHS: usize = 20;
const EPOCHS: usize = 10_000;
const STEPS_PER_EPOCH: usize = 6_000;
const HIDDEN_COUNT: usize = 256;
const LEARNING_RATE_ACTOR: f64 = 0.0003;
const LEARNING_RATE_CRITIC: f64 = 0.0005;
const GAMMA: f32 = 0.99;
const K_EPOCHS: usize = 20;
const EPS_CLIP: f32 = 0.2;
const GAE_LAMBDA: f32 = 0.95;
const TARGET_KL: f32 = 0.01;

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

fn normalized_adv(adv: Tensor) -> Result<Tensor> {
    let adv_mean = adv.mean_keepdim(0)?;
    let adv_std = adv.var_keepdim(0)?.sqrt()?;
    adv.broadcast_sub(&adv_mean)?.broadcast_div(&adv_std)
}

impl RolloutBuffer {
    fn new() -> Self {
        Self {
            states: Vec::with_capacity(STEPS_PER_EPOCH),
            actions_1: Vec::with_capacity(STEPS_PER_EPOCH),
            actions_2: Vec::with_capacity(STEPS_PER_EPOCH),
            logprobs_1: Vec::with_capacity(STEPS_PER_EPOCH),
            logprobs_2: Vec::with_capacity(STEPS_PER_EPOCH),
            rewards_1: Vec::with_capacity(STEPS_PER_EPOCH),
            rewards_2: Vec::with_capacity(STEPS_PER_EPOCH),
            state_values_1: Vec::with_capacity(STEPS_PER_EPOCH),
            state_values_2: Vec::with_capacity(STEPS_PER_EPOCH),
            advantage_1: vec![0.0; STEPS_PER_EPOCH],
            advantage_2: vec![0.0; STEPS_PER_EPOCH],
            return_1: vec![0.0; STEPS_PER_EPOCH],
            return_2: vec![0.0; STEPS_PER_EPOCH],
            path_start_idx: 0,
            ptr: 0,
        }
    }

    fn finish_path(&mut self, value1: f32, value2: f32) {
        let path_range = self.path_start_idx..self.ptr;
        let idx = self.path_start_idx;

        // Agent1
        let rews = &self.rewards_1[path_range.clone()];
        let vals = &self.state_values_1[path_range.clone()];
        compute_gae(&mut self.advantage_1, idx, rews, vals, value1);
        compute_return(&mut self.return_1, idx, rews, value1);

        // Agent2
        let rews = &self.rewards_2[path_range.clone()];
        let vals = &self.state_values_2[path_range];
        compute_gae(&mut self.advantage_2, idx, rews, vals, value2);
        compute_return(&mut self.return_2, idx, rews, value2);

        self.path_start_idx = self.ptr;
    }

    fn get_obs(&self) -> Result<Tensor> {
        Tensor::stack(&self.states.clone(), 0)
    }

    /// Returns (action, return, logprob, norm_adv)
    fn get_agent1(&self, device: &Device) -> Result<(Tensor, Tensor, Tensor, Tensor)> {
        let act = Tensor::from_iter(self.actions_1.clone(), device)?;
        let ret = Tensor::from_iter(self.return_1.clone(), device)?;
        let logprob = Tensor::from_iter(self.logprobs_1.clone(), device)?;
        let adv = Tensor::from_iter(self.advantage_1.clone(), device)?;
        let norm_adv = normalized_adv(adv)?;

        Ok((act, ret, logprob, norm_adv))
    }

    /// Returns (action, return, logprob, norm_adv)
    fn get_agent2(&self, device: &Device) -> Result<(Tensor, Tensor, Tensor, Tensor)> {
        let act = Tensor::from_iter(self.actions_2.clone(), device)?;
        let ret = Tensor::from_iter(self.return_2.clone(), device)?;
        let logprob = Tensor::from_iter(self.logprobs_2.clone(), device)?;
        let adv = Tensor::from_iter(self.advantage_2.clone(), device)?;
        let norm_adv = normalized_adv(adv)?;

        Ok((act, ret, logprob, norm_adv))
    }

    fn reset(&mut self) {
        self.states.clear();
        self.actions_1.clear();
        self.actions_2.clear();
        self.logprobs_1.clear();
        self.logprobs_2.clear();
        self.rewards_1.clear();
        self.rewards_2.clear();
        self.state_values_1.clear();
        self.state_values_2.clear();
        self.ptr = 0;
        self.path_start_idx = 0;
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

fn compute_gae(
    adv_vec: &mut [f32],
    idx: usize,
    rewards: &[f32],
    state_values: &[f32],
    bootstrap: f32,
) {
    let len = rewards.len();

    let mut last_advantage = rewards[len - 1] + GAMMA * bootstrap - state_values[len - 1];
    adv_vec[idx + len - 1] = last_advantage;

    for t in (0..len - 1).rev() {
        let delta = rewards[t] + GAMMA * state_values[t + 1] - state_values[t];
        last_advantage = delta + GAMMA * GAE_LAMBDA * last_advantage;
        adv_vec[idx + t] = last_advantage;
    }
}

fn compute_return(ret_vec: &mut [f32], idx: usize, rewards: &[f32], bootstrap: f32) {
    let len = rewards.len();

    let mut last_return = bootstrap;

    for t in (0..len).rev() {
        last_return = rewards[t] + GAMMA * last_return;
        ret_vec[idx + t] = last_return;
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
                .add(linear(
                    STATE_VECTOR_LEN,
                    HIDDEN_COUNT,
                    actor_vb.pp("actor_in"),
                )?)
                .add(Activation::Relu)
                .add(linear(
                    HIDDEN_COUNT,
                    HIDDEN_COUNT,
                    actor_vb.pp("actor_hidden"),
                )?)
                .add(Activation::Relu)
                .add(linear(
                    HIDDEN_COUNT,
                    ACTION_SPACE,
                    actor_vb.pp("actor_out"),
                )?),
            critic: seq()
                .add(linear(
                    STATE_VECTOR_LEN,
                    HIDDEN_COUNT,
                    critic_vb.pp("critic_in"),
                )?)
                .add(Activation::Relu)
                .add(linear(
                    HIDDEN_COUNT,
                    HIDDEN_COUNT,
                    critic_vb.pp("critic_hidden"),
                )?)
                .add(Activation::Relu)
                .add(linear(HIDDEN_COUNT, 1, critic_vb.pp("critic_out"))?),
        };

        Ok((ac, actor_map, critic_map))
    }

    /// Action, logp_a, state_val
    fn step(&self, obs: &Tensor, rng: &mut rand::rngs::ThreadRng) -> Result<(u32, f32, f32)> {
        let estimates = self.actor.forward(&obs.unsqueeze(0)?)?.detach();
        let action_probs = softmax(&estimates, D::Minus1)?.squeeze(0)?.detach();
        let weights = action_probs.to_vec1::<f32>()?;
        let action = rng.sample(WeightedIndex::new(weights).unwrap());
        let state_val = self
            .critic
            .forward(&obs.unsqueeze(0)?)?
            .squeeze(0)?
            .squeeze(0)?
            .detach()
            .to_scalar()?;

        let logp_a = action_probs.i(action)?.to_scalar::<f32>()?.ln();

        Ok((action as u32, logp_a, state_val))
    }

    /// Prob distributions for each state, logp for each action
    /// Unscreeze actions before calling this
    fn pi(&self, obs_batch: &Tensor, actions: &Tensor) -> Result<(Tensor, Tensor)> {
        let estimates = self.actor.forward(obs_batch)?;
        let action_probs = softmax(&estimates, D::Minus1)?;

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
    _critic_map: VarMap,
    actor_optimizer: AdamW,
    critic_optimizer: AdamW,
}

impl PPOAgent {
    fn new(device: &Device) -> Result<Self> {
        let (policy, actor_map, _critic_map) = ActorCritic::new(device)?;

        let actor_optimizer = AdamW::new_lr(actor_map.all_vars(), LEARNING_RATE_ACTOR)?;
        let critic_optimizer = AdamW::new_lr(_critic_map.all_vars(), LEARNING_RATE_CRITIC)?;

        Ok(Self {
            policy,
            actor_map,
            _critic_map,
            actor_optimizer,
            critic_optimizer,
        })
    }

    fn save(&self, filename: &str) -> Result<()> {
        println!("Saved policy to file: {filename}");
        save_model(&self.actor_map, filename)
    }

    /// Returns loss_po and approx_kl
    /// Unsqueeze actions before calling this
    fn compute_loss_pi(
        &self,
        obs: &Tensor,
        actions: &Tensor,
        adv: &Tensor,
        logp_old: &Tensor,
    ) -> Result<(Tensor, f32)> {
        let (_pi, logp) = self.policy.pi(obs, actions)?;
        let ratio = (&logp - logp_old)?.exp()?;
        let clip_adv = (ratio.clamp(1.0 - EPS_CLIP, 1.0 + EPS_CLIP) * adv)?;
        let loss_pi = (ratio * adv)?.minimum(&clip_adv)?.mean_all()?.neg()?;

        let approx_kl = (logp_old - logp)?.mean_all()?.to_scalar()?;

        Ok((loss_pi, approx_kl))
    }

    /// Returns loss_v
    fn compute_loss_v(&self, obs: &Tensor, ret: &Tensor) -> Result<Tensor> {
        let val = self.policy.v(obs)?;
        let loss_v = (val - ret)?.sqr()?.mean_all()?;
        Ok(loss_v)
    }
}

#[allow(dead_code)]
pub fn train(mut env: Environment<'_>, device: Device, start: Instant) -> Result<()> {
    let mut agent1 = PPOAgent::new(&device)?;
    let mut agent2 = PPOAgent::new(&device)?;

    let mut rng = rand::rng();
    let mut buffer = RolloutBuffer::new();

    let mut first_episode = true;

    for epoch in 1..EPOCHS + 1 {
        for step in 0..STEPS_PER_EPOCH {
            let observation = env.obs(&device)?;
            let actions = take_agent_turns(&agent1, &agent2, &mut buffer, &observation, &mut rng)?;

            // Update environment
            let (terminal, rewards) = env.step(actions);
            buffer.push_env(observation, rewards);

            let epoch_ended = step == STEPS_PER_EPOCH - 1;

            if terminal || epoch_ended {
                let (v1, v2) = if !terminal {
                    let last_obs = env.obs(&device)?;
                    let (_, _, v1) = agent1.policy.step(&last_obs, &mut rng)?;
                    let (_, _, v2) = agent2.policy.step(&last_obs, &mut rng)?;
                    (v1, v2)
                } else {
                    (0.0, 0.0)
                };

                buffer.finish_path(v1, v2);

                if first_episode && epoch % EPOCH_PRINT_STEP == 0 {
                    first_episode = false;

                    env.display(epoch, start.elapsed());
                }

                if epoch_ended && epoch % SAVE_INTERVAL == 0 {
                    agent1.save(AGENT1_OUTPUT_PATH)?;
                    agent2.save(AGENT2_OUTPUT_PATH)?;
                }

                env.reset();
            }
        }

        // Update only one agent at a time
        update_agent(&mut agent1, &mut agent2, &mut buffer, epoch, &device)?;
        first_episode = true;

        env.reset();
    }

    println!("Completed in {:?} secs", start.elapsed());
    agent1.save(AGENT1_OUTPUT_PATH)?;
    agent2.save(AGENT2_OUTPUT_PATH)?;
    Ok(())
}

fn take_agent_turns(
    agent1: &PPOAgent,
    agent2: &PPOAgent,
    buffer: &mut RolloutBuffer,
    observation: &Tensor,
    rng: &mut ThreadRng,
) -> Result<(u32, u32)> {
    let (action1, logprob, state_val) = agent1.policy.step(observation, rng)?;
    buffer.push_agent1(action1, logprob, state_val);
    let (action2, logprob, state_val) = agent2.policy.step(observation, rng)?;
    buffer.push_agent2(action2, logprob, state_val);

    Ok((action1, action2))
}

fn update_agent(
    agent1: &mut PPOAgent,
    agent2: &mut PPOAgent,
    buffer: &mut RolloutBuffer,
    epoch: usize,
    device: &Device,
) -> Result<()> {
    let obs_batch = buffer.get_obs()?;

    let (agent, data) = if epoch % (OPPONENT_FREEZE_EPOCHS * 2) < OPPONENT_FREEZE_EPOCHS {
        (agent1, buffer.get_agent1(device)?)
    } else {
        (agent2, buffer.get_agent2(device)?)
    };

    update_single_agent(agent, &obs_batch, data)?;

    buffer.reset();

    Ok(())
}

fn update_single_agent(
    agent: &mut PPOAgent,
    obs_batch: &Tensor,
    data: (Tensor, Tensor, Tensor, Tensor),
) -> Result<()> {
    let (actions, ret, logp_old, adv) = data;
    let actions = actions.unsqueeze(1)?;

    for _ in 0..K_EPOCHS {
        let (loss_pi, kl) = agent.compute_loss_pi(obs_batch, &actions, &adv, &logp_old)?;
        if kl > 1.5 * TARGET_KL {
            break;
        }

        agent.actor_optimizer.backward_step(&loss_pi)?;
    }

    for _ in 0..K_EPOCHS {
        let loss_v = agent.compute_loss_v(obs_batch, &ret)?;
        agent.critic_optimizer.backward_step(&loss_v)?;
    }

    Ok(())
}

pub fn get_agent_action(agent: &Sequential, obs: &Tensor, rng: &mut ThreadRng) -> Result<u32> {
    let estimates = agent.forward(&obs.unsqueeze(0)?)?.detach();
    let action_probs = softmax(&estimates, D::Minus1)?.squeeze(0)?.detach();
    let weights = action_probs.to_vec1::<f32>()?;
    Ok(rng.sample(WeightedIndex::new(weights).unwrap()) as u32)
}
