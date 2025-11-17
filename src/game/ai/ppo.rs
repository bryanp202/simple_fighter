use candle_core::{D, DType, Device, IndexOp, Result, Tensor};
use candle_nn::{
    Activation, AdamW, Module, Optimizer, Sequential, VarBuilder, VarMap, linear, ops::softmax, seq
};
use rand::{Rng, distr::weighted::WeightedIndex, rngs::ThreadRng};

use crate::game::{
    ai::{ACTION_SPACE, STATE_VECTOR_LEN, save_model},
};

const HIDDEN_COUNT: usize = 256;
const LEARNING_RATE_ACTOR: f64 = 0.00005;
const LEARNING_RATE_CRITIC: f64 = 0.00005;
const GAMMA: f32 = 0.996;
const EPS_CLIP: f32 = 0.2;
const GAE_LAMBDA: f32 = 0.97;

const K_EPOCHS: usize = 20;
const TARGET_KL: f32 = 0.01;

pub struct RolloutBuffer {
    steps_per_epoch: usize,
    obs: Vec<Tensor>,

    actions: Vec<u32>,
    logprobs: Vec<f32>,
    rewards: Vec<f32>,
    state_values: Vec<f32>,
    advantage: Vec<f32>,
    ret: Vec<f32>,

    path_start_idx: usize,
    ptr: usize,
}

fn normalized_adv(adv: Tensor) -> Result<Tensor> {
    let adv_mean = adv.mean_keepdim(0)?;
    let adv_std = adv.var_keepdim(0)?.sqrt()?;
    adv.broadcast_sub(&adv_mean)?.broadcast_div(&adv_std)
}

impl RolloutBuffer {
    pub fn new(steps_per_epoch: usize) -> Self {
        Self {
            steps_per_epoch,
            obs: Vec::with_capacity(steps_per_epoch),
            actions: Vec::with_capacity(steps_per_epoch),
            logprobs: Vec::with_capacity(steps_per_epoch),
            rewards: Vec::with_capacity(steps_per_epoch),
            state_values: Vec::with_capacity(steps_per_epoch),
            advantage: vec![0.0; steps_per_epoch],
            ret: vec![0.0; steps_per_epoch],
            path_start_idx: 0,
            ptr: 0,
        }
    }

    pub fn finish_path(&mut self, value1: f32) {
        let path_range = self.path_start_idx..self.ptr;
        let idx = self.path_start_idx;

        // Agent1
        let rews = &self.rewards[path_range.clone()];
        let vals = &self.state_values[path_range.clone()];
        compute_gae(&mut self.advantage[path_range], rews, vals, value1);
        compute_return(&mut self.ret, idx, rews, value1);

        self.path_start_idx = self.ptr;
    }

    /// Returns (obs, action, return, logprob, norm_adv)
    pub fn get(&self, device: &Device) -> Result<(Tensor, Tensor, Tensor, Tensor, Tensor)> {
        let obs = Tensor::stack(&self.obs, 0)?;

        let act = Tensor::from_slice(&self.actions, self.steps_per_epoch, device)?;
        let ret = Tensor::from_slice(&self.ret, self.steps_per_epoch, device)?;
        let logprob = Tensor::from_slice(&self.logprobs, self.steps_per_epoch, device)?;
        let adv = Tensor::from_slice(&self.advantage, self.steps_per_epoch, device)?;
        let norm_adv = normalized_adv(adv)?;

        Ok((obs, act, ret, logprob, norm_adv))
    }

    pub fn reset(&mut self) {
        self.obs.clear();
        self.actions.clear();
        self.logprobs.clear();
        self.rewards.clear();
        self.state_values.clear();
        self.ptr = 0;
        self.path_start_idx = 0;
    }

    pub fn push_agent(&mut self, action: u32, logprob: f32, state_val: f32) {
        self.actions.push(action);
        self.logprobs.push(logprob);
        self.state_values.push(state_val);
    }

    pub fn push_env(&mut self, obs: Tensor, reward: f32) {
        self.obs.push(obs);
        self.rewards.push(reward);

        self.ptr += 1;
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

    #[allow(dead_code)]
    /// Step, but only the action
    fn act(&self, obs: &Tensor, rng: &mut rand::rngs::ThreadRng) -> Result<u32> {
        let estimates = self.actor.forward(&obs.unsqueeze(0)?)?.detach();
        let action_probs = softmax(&estimates, D::Minus1)?.squeeze(0)?.detach();
        let weights = action_probs.to_vec1::<f32>()?;
        let action = rng.sample(WeightedIndex::new(weights).unwrap());

        Ok(action as u32)
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

pub struct PPOAgent {
    // Current Policy
    policy: ActorCritic,
    actor_map: VarMap,
    _critic_map: VarMap,
    actor_optimizer: AdamW,
    critic_optimizer: AdamW,
}

impl PPOAgent {
    pub fn new(device: &Device) -> Result<Self> {
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

    pub fn into_policy(self) -> (Sequential, VarMap) {
        (self.policy.actor, self.actor_map)
    }

    pub fn save(&self, filename: &str) -> Result<()> {
        save_model(&self.actor_map, filename)
    }

    /// Returns loss_po and approx_kl
    /// Unsqueeze actions before calling this
    pub fn compute_loss_pi(
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
    pub fn compute_loss_v(&self, obs: &Tensor, ret: &Tensor) -> Result<Tensor> {
        let val = self.policy.v(obs)?;
        let loss_v = (val - ret)?.sqr()?.mean_all()?;
        Ok(loss_v)
    }

    /// Action, logp_a, state_val
    pub fn step(&self, obs: &Tensor, rng: &mut ThreadRng) -> Result<(u32, f32, f32)> {
        self.policy.step(obs, rng)
    }

    pub fn update(
        &mut self,
        buffer: &RolloutBuffer,
        device: &Device,
    ) -> Result<()> {
        let (obs_batch, actions, ret, logp_old, adv) = buffer.get(device)?;
        let actions = actions.unsqueeze(1)?;

        for _ in 0..K_EPOCHS {
            let (loss_pi, kl) = self.compute_loss_pi(&obs_batch, &actions, &adv, &logp_old)?;
            if kl > 1.5 * TARGET_KL {
                break;
            }

            self.actor_optimizer.backward_step(&loss_pi)?;
        }

        for _ in 0..K_EPOCHS {
            let loss_v = self.compute_loss_v(&obs_batch, &ret)?;
            self.critic_optimizer.backward_step(&loss_v)?;
        }

        Ok(())
    }
}

pub fn get_agent_action(agent: &Sequential, obs: &Tensor, rng: &mut ThreadRng) -> Result<u32> {
    let estimates = agent.forward(&obs.unsqueeze(0)?)?.detach();
    let action_probs = softmax(&estimates, D::Minus1)?.squeeze(0)?.detach();
    let weights = action_probs.to_vec1::<f32>()?;
    Ok(rng.sample(WeightedIndex::new(weights).unwrap()) as u32)
}

//----------------//
/* Multithreading */
//----------------//

// struct SimulatorPool {
//     /// (Wins, games, training data)
//     receivers: Vec<mpsc::Receiver<(usize, usize, Box<RolloutBuffer>)>>,
//     /// (Challenger, Trainer)
//     senders: Vec<mpsc::Sender<(Arc<VarMap>, Arc<VarMap>)>>,
//     barrier: Arc<Barrier>,
// }

// impl SimulatorPool {
//     fn new(context: Arc<GameContext>, device: Arc<Device>) -> Self {
//         let barrier = Arc::new(Barrier::new(MAX_POOL_SIZE + 1));
//         let mut receivers = Vec::with_capacity(MAX_POOL_SIZE);
//         let mut senders = Vec::with_capacity(MAX_POOL_SIZE);

//         for _ in 0..MAX_POOL_SIZE {
//             let (local_tx, thread_rx) = mpsc::channel();
//             let (thread_tx, local_rx) = mpsc::channel();
//             let barrier = barrier.clone();
//             let context = context.clone();
//             let device = device.clone();

//             receivers.push(local_rx);
//             senders.push(local_tx);

//             thread::spawn(move || simulator_thread(context, thread_rx, thread_tx, barrier, device));

//         }

//         Self {
//             receivers,
//             senders,
//             barrier,
//         }
//     }

//     fn train_challenger(&self, challenger: Arc<RwLock<PPOAgent>>, trainers: &mut TrainerPool) {
//         for tx in &self.senders {

//         }
//     }
// }

// fn simulator_thread(
//     context: Arc<GameContext>,
//     receiver: mpsc::Receiver<(Arc<VarMap>, Arc<VarMap>)>,
//     sender: mpsc::Sender<(usize, usize, Box<RolloutBuffer>)>,
//     barrier: Arc<Barrier>,
//     device: Arc<Device>,
// ) {
//     let (h1, player1_inputs) = input::new_inputs(PLAYER1_BUTTONS, PLAYER1_DIRECTIONS);
//     let (h2, player2_inputs) = input::new_inputs(PLAYER2_BUTTONS, PLAYER2_DIRECTIONS);
//     let player1 = character::State::new(0.0, FPoint::new(0.0, 0.0), Side::Left);
//     let player2 = character::State::new(0.0, FPoint::new(0.0, 0.0), Side::Left);

//     let mut state = GameState { player1_inputs, player2_inputs, player1, player2 };
//     let mut inputs = PlayerInputs { player1: h1, player2: h2 };

//     let mut rng = rand::rng();
//     let mut env = Environment::new(&context, &mut inputs, &mut state);

//     loop {
//         let Ok((actor, critic, trainer)) = receiver.recv() else {
//             return;
//         };

//         let challenger = ActorCritic::from_var_maps(challenger);
//         let trainer = ActorCritic::from_var_map(&trainer);
//         let mut buffer = Box::new(RolloutBuffer::new());

//         let (wins, games) = fight_trainer(&mut env, &challenger, trainer, &mut buffer, &device, &mut rng).unwrap();

//         sender.send((wins, games, buffer)).unwrap();

//         barrier.wait();
//     }
// }
