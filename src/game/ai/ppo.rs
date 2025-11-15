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

const OPPONENT_FREEZE_EPOCHS: usize = 25;
const EPOCHS: usize = 10_000;
const STEPS_PER_EPOCH: usize = 6_000;
const HIDDEN_COUNT: usize = 256;
const LEARNING_RATE_ACTOR: f64 = 0.000_01;
const LEARNING_RATE_CRITIC: f64 = 0.000_03;
const GAMMA: f32 = 0.994;
const K_EPOCHS: usize = 20;
const EPS_CLIP: f32 = 0.2;
const GAE_LAMBDA: f32 = 0.97;
const TARGET_KL: f32 = 0.01;

const EPOCH_PRINT_STEP: usize = EPOCHS / 1_000;

#[derive(Default)]
struct RolloutBuffer {
    obs: Vec<Tensor>,
    obs_inv: Vec<Tensor>,

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
            obs: Vec::with_capacity(STEPS_PER_EPOCH),
            obs_inv: Vec::with_capacity(STEPS_PER_EPOCH),
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

        // Agent1
        let rews = &self.rewards_1[path_range.clone()];
        let vals = &self.state_values_1[path_range.clone()];
        compute_gae(
            &mut self.advantage_1[path_range.clone()],
            rews,
            vals,
            value1,
        );
        compute_return(&mut self.return_1[path_range.clone()], rews, value1);

        // Agent2
        let rews = &self.rewards_2[path_range.clone()];
        let vals = &self.state_values_2[path_range.clone()];
        compute_gae(
            &mut self.advantage_2[path_range.clone()],
            rews,
            vals,
            value2,
        );
        compute_return(&mut self.return_2[path_range], rews, value2);

        self.path_start_idx = self.ptr;
    }

    /// Returns ((obs, obs_inv), returns)
    fn get_obs_ret(&self, device: &Device) -> Result<((Tensor, Tensor), Tensor)> {
        let len = self.return_1.len() + self.return_2.len();
        let mut ret_cat = Vec::with_capacity(len);
        ret_cat.extend_from_slice(&self.return_1);
        ret_cat.extend_from_slice(&self.return_2);

        let obs = Tensor::stack(&self.obs, 0)?;
        let obs_inv = Tensor::stack(&self.obs_inv, 0)?;

        let ret = Tensor::from_slice(&ret_cat, len, device)?;

        Ok(((obs, obs_inv), ret))
    }

    /// Returns (action, logprob, norm_adv)
    fn get_agent1(&self, device: &Device) -> Result<(Tensor, Tensor, Tensor)> {
        let act = Tensor::from_slice(&self.actions_1, self.actions_1.len(), device)?;
        let logprob = Tensor::from_slice(&self.logprobs_1, self.logprobs_1.len(), device)?;
        let adv = Tensor::from_slice(&self.advantage_1, self.advantage_1.len(), device)?;
        let norm_adv = normalized_adv(adv)?;

        Ok((act, logprob, norm_adv))
    }

    /// Returns (action, logprob, norm_adv)
    fn get_agent2(&self, device: &Device) -> Result<(Tensor, Tensor, Tensor)> {
        let act = Tensor::from_slice(&self.actions_2, self.actions_2.len(), device)?;
        let logprob = Tensor::from_slice(&self.logprobs_2, self.logprobs_2.len(), device)?;
        let adv = Tensor::from_slice(&self.advantage_2, self.advantage_2.len(), device)?;
        let norm_adv = normalized_adv(adv)?;

        Ok((act, logprob, norm_adv))
    }

    fn reset(&mut self) {
        self.obs.clear();
        self.obs_inv.clear();
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

    fn push_env(&mut self, obs: Tensor, obs_inv: Tensor, rewards: DuelFloat) {
        self.obs.push(obs);
        self.obs_inv.push(obs_inv);
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

fn compute_gae(adv_vec: &mut [f32], rewards: &[f32], state_values: &[f32], bootstrap: f32) {
    let len = rewards.len();

    let mut last_advantage = rewards[len - 1] + GAMMA * bootstrap - state_values[len - 1];
    adv_vec[len - 1] = last_advantage;

    for t in (0..len - 1).rev() {
        let delta = rewards[t] + GAMMA * state_values[t + 1] - state_values[t];
        last_advantage = delta + GAMMA * GAE_LAMBDA * last_advantage;
        adv_vec[t] = last_advantage;
    }
}

fn compute_return(ret_vec: &mut [f32], rewards: &[f32], bootstrap: f32) {
    let len = rewards.len();

    let mut last_return = bootstrap;

    for t in (0..len).rev() {
        last_return = rewards[t] + GAMMA * last_return;
        ret_vec[t] = last_return;
    }
}

struct PPOCritic {
    critic: Sequential,
    _var_map: VarMap,
    optimizer: AdamW,
}

impl PPOCritic {
    /// (ActorCritic, ActorMap, CriticMap)
    fn new(device: &Device) -> Result<Self> {
        let var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, device);
        let critic = seq()
            .add(linear(STATE_VECTOR_LEN, HIDDEN_COUNT, vb.pp("critic_in"))?)
            .add(Activation::Relu)
            .add(linear(HIDDEN_COUNT, HIDDEN_COUNT, vb.pp("critic_hidden"))?)
            .add(Activation::Relu)
            .add(linear(HIDDEN_COUNT, 1, vb.pp("critic_out"))?);

        let optimizer = AdamW::new_lr(var_map.all_vars(), LEARNING_RATE_CRITIC)?;

        Ok(Self {
            critic,
            _var_map: var_map,
            optimizer,
        })
    }

    fn step(&self, obs: &Tensor) -> Result<f32> {
        self.critic
            .forward(&obs.unsqueeze(0)?)?
            .squeeze(0)?
            .squeeze(0)?
            .detach()
            .to_scalar()
    }

    /// Should receive the returns and overservation
    fn update(&mut self, obs_batch: &Tensor, ret_batch: &Tensor) -> Result<()> {
        let ret_batch = ret_batch.detach();
        for _ in 0..K_EPOCHS {
            let val = self.critic.forward(&obs_batch)?.squeeze(D::Minus1)?;
            let loss_v = (val - &ret_batch)?.sqr()?.mean_all()?;
            self.optimizer.backward_step(&loss_v)?;
        }

        Ok(())
    }
}

struct PPOActor {
    policy: Sequential,
    var_map: VarMap,
    optimizer: AdamW,
}

impl PPOActor {
    fn new(device: &Device) -> Result<Self> {
        let var_map = VarMap::new();

        let vb = VarBuilder::from_varmap(&var_map, DType::F32, device);
        let policy = seq()
            .add(linear(STATE_VECTOR_LEN, HIDDEN_COUNT, vb.pp("actor_in"))?)
            .add(Activation::Relu)
            .add(linear(HIDDEN_COUNT, HIDDEN_COUNT, vb.pp("actor_hidden"))?)
            .add(Activation::Relu)
            .add(linear(HIDDEN_COUNT, ACTION_SPACE, vb.pp("actor_out"))?);

        let optimizer = AdamW::new_lr(var_map.all_vars(), LEARNING_RATE_ACTOR)?;

        Ok(Self {
            policy,
            var_map,
            optimizer,
        })
    }

    /// Returns loss_po and approx_kl
    /// Unsqueeze actions before calling this
    fn compute_loss_pi(
        &self,
        obs_batch: &Tensor,
        actions: &Tensor,
        adv: &Tensor,
        logp_old: &Tensor,
    ) -> Result<(Tensor, f32)> {
        let estimates = self.policy.forward(obs_batch)?;
        let action_probs = softmax(&estimates, D::Minus1)?;
        let logp = action_probs.gather(actions, 1)?.log()?.squeeze(D::Minus1)?;

        let ratio = (&logp - logp_old)?.exp()?;
        let clip_adv = (ratio.clamp(1.0 - EPS_CLIP, 1.0 + EPS_CLIP) * adv)?;
        let loss_pi = (ratio * adv)?.minimum(&clip_adv)?.mean_all()?.neg()?;

        let approx_kl = (logp_old - logp)?.mean_all()?.to_scalar()?;

        Ok((loss_pi, approx_kl))
    }

    /// Returns (action, logprob)
    fn step(&self, obs: &Tensor, rng: &mut rand::rngs::ThreadRng) -> Result<(u32, f32)> {
        let estimates = self.policy.forward(&obs.unsqueeze(0)?)?.detach();
        let action_probs = softmax(&estimates, D::Minus1)?.squeeze(0)?.detach();
        let weights = action_probs.to_vec1::<f32>()?;
        let action = rng.sample(WeightedIndex::new(weights).unwrap());

        let logp_a = action_probs.i(action)?.to_scalar::<f32>()?.ln();

        Ok((action as u32, logp_a))
    }

    fn update(&mut self, obs_batch: &Tensor, data: (Tensor, Tensor, Tensor)) -> Result<()> {
        let (actions, logp_old, adv) = data;
        let actions = actions.unsqueeze(1)?;

        for _ in 0..K_EPOCHS {
            let (loss_pi, kl) = self.compute_loss_pi(obs_batch, &actions, &adv, &logp_old)?;
            if kl > 1.5 * TARGET_KL {
                break;
            }

            self.optimizer.backward_step(&loss_pi)?;
        }

        Ok(())
    }

    fn save(&self, filename: &str) -> Result<()> {
        save_model(&self.var_map, filename)
    }
}

#[allow(dead_code)]
pub fn train(mut env: Environment<'_>, device: Device, start: Instant) -> Result<()> {
    let mut agent1 = PPOActor::new(&device)?;
    let mut agent2 = PPOActor::new(&device)?;
    let mut critic = PPOCritic::new(&device)?;

    let mut rng = rand::rng();
    let mut buffer = RolloutBuffer::new();

    let mut first_episode = true;

    for epoch in 1..EPOCHS + 1 {
        for step in 0..STEPS_PER_EPOCH {
            let (obs, obs_inv) = env.obs_with_inv(&device)?;
            let actions = take_agent_turns(
                &agent1,
                &agent2,
                &critic,
                &mut buffer,
                &obs,
                &obs_inv,
                &mut rng,
            )?;

            // Update environment
            let (terminal, rewards) = env.step(actions);
            buffer.push_env(obs, obs_inv, rewards);

            let epoch_ended = step == STEPS_PER_EPOCH - 1;

            if terminal || epoch_ended {
                let (v1, v2) = if !terminal {
                    let (last_obs, last_obs_inv) = env.obs_with_inv(&device)?;
                    let v1 = critic.step(&last_obs)?;
                    let v2 = critic.step(&last_obs_inv)?;
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
        update_agent(
            &mut agent1,
            &mut agent2,
            &mut critic,
            &mut buffer,
            epoch,
            &device,
        )?;
        first_episode = true;

        env.reset_rng(&mut rng);
    }

    println!("Completed in {:?} secs", start.elapsed());
    agent1.save(AGENT1_OUTPUT_PATH)?;
    agent2.save(AGENT2_OUTPUT_PATH)?;
    Ok(())
}

fn take_agent_turns(
    agent1: &PPOActor,
    agent2: &PPOActor,
    critic: &PPOCritic,
    buffer: &mut RolloutBuffer,
    obs: &Tensor,
    obs_inv: &Tensor,
    rng: &mut ThreadRng,
) -> Result<(u32, u32)> {
    let (action1, logprob) = agent1.step(obs, rng)?;
    let state_val = critic.step(obs)?;
    buffer.push_agent1(action1, logprob, state_val);

    let (action2, logprob) = agent2.step(obs, rng)?;
    let state_val = critic.step(obs_inv)?;
    buffer.push_agent2(action2, logprob, state_val);

    Ok((action1, action2))
}

fn update_agent(
    agent1: &mut PPOActor,
    agent2: &mut PPOActor,
    critic: &mut PPOCritic,
    buffer: &mut RolloutBuffer,
    epoch: usize,
    device: &Device,
) -> Result<()> {
    let ((obs, obs_inv), ret_batch) = buffer.get_obs_ret(device)?;
    let obs_batch = Tensor::cat(&[&obs, &obs_inv], 0)?;

    let (agent, data) = if epoch % (OPPONENT_FREEZE_EPOCHS * 2) < OPPONENT_FREEZE_EPOCHS {
        (agent1, buffer.get_agent1(device)?)
    } else {
        (agent2, buffer.get_agent2(device)?)
    };

    agent.update(&obs, data)?;

    // Update critic with both actors
    critic.update(&obs_batch, &ret_batch)?;

    buffer.reset();

    Ok(())
}

pub fn get_agent_action(agent: &Sequential, obs: &Tensor, rng: &mut ThreadRng) -> Result<u32> {
    let estimates = agent.forward(&obs.unsqueeze(0)?)?.detach();
    let action_probs = softmax(&estimates, D::Minus1)?.squeeze(0)?.detach();
    let weights = action_probs.to_vec1::<f32>()?;
    Ok(rng.sample(WeightedIndex::new(weights).unwrap()) as u32)
}
