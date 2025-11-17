use std::{collections::VecDeque, time::Instant};

use candle_core::{D, DType, Device, IndexOp, Result, Tensor};
use candle_nn::{
    Activation, AdamW, Module, Optimizer, Sequential, VarBuilder, VarMap, linear, ops::softmax, seq,
};
use rand::{Rng, distr::weighted::WeightedIndex, rngs::ThreadRng};

use crate::game::{
    Side,
    ai::{ACTION_SPACE, STATE_VECTOR_LEN, env::Environment, save_model},
};

const BEST_AGENT_OUTPUT_PATH: &str = "./ai/ppo/best_NEW.safetensors";
const RUNNER_UP_OUTPUT_PATH: &str = "./ai/ppo/runner_up_NEW.safetensors";
const SAVE_INTERVAL: usize = 1;

const EPOCHS: usize = 32;
const STEPS_PER_EPOCH: usize = 8_000;
const HIDDEN_COUNT: usize = 256;
const LEARNING_RATE_ACTOR: f64 = 0.00005;
const LEARNING_RATE_CRITIC: f64 = 0.00005;
const GAMMA: f32 = 0.996;
const K_EPOCHS: usize = 20;
const EPS_CLIP: f32 = 0.2;
const GAE_LAMBDA: f32 = 0.97;
const TARGET_KL: f32 = 0.01;

const MAX_POOL_SIZE: usize = 16;
const WINRATE_THRESH: f32 = 0.60;
const WINRATE_WINDOW: usize = 32;
const MIN_ROUNDS_PER_TRAINER: usize = 8;
const MAX_GAMES: usize = 2048;
const EPOCH_PRINT_STEP: usize = 0;

struct Trainer {
    policy: Sequential,
    var_map: VarMap,
}

struct TrainerPool {
    trainers: VecDeque<Trainer>,
}

impl TrainerPool {
    fn new() -> Self {
        Self {
            trainers: VecDeque::new(),
        }
    }

    fn push(&mut self, trainer: Trainer) {
        if self.trainers.len() == MAX_POOL_SIZE {
            self.trainers.pop_back();
        }
        self.trainers.push_front(trainer);
    }

    fn iter(&self) -> impl Iterator<Item = &Trainer> {
        self.trainers.iter()
    }

    fn count(&self) -> usize {
        self.trainers.len()
    }

    fn get_best(&self) -> (&Trainer, &Trainer) {
        (
            self.trainers.get(0).unwrap(),
            self.trainers.get(1).unwrap()
        )
    }
}

struct RolloutBuffer {
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
    fn new() -> Self {
        Self {
            obs: Vec::with_capacity(STEPS_PER_EPOCH),
            actions: Vec::with_capacity(STEPS_PER_EPOCH),
            logprobs: Vec::with_capacity(STEPS_PER_EPOCH),
            rewards: Vec::with_capacity(STEPS_PER_EPOCH),
            state_values: Vec::with_capacity(STEPS_PER_EPOCH),
            advantage: vec![0.0; STEPS_PER_EPOCH],
            ret: vec![0.0; STEPS_PER_EPOCH],
            path_start_idx: 0,
            ptr: 0,
        }
    }

    fn finish_path(&mut self, value1: f32) {
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
    fn get(&self, device: &Device) -> Result<(Tensor, Tensor, Tensor, Tensor, Tensor)> {
        let obs = Tensor::stack(&self.obs, 0)?;

        let act = Tensor::from_slice(&self.actions, STEPS_PER_EPOCH, device)?;
        let ret = Tensor::from_slice(&self.ret, STEPS_PER_EPOCH, device)?;
        let logprob = Tensor::from_slice(&self.logprobs, STEPS_PER_EPOCH, device)?;
        let adv = Tensor::from_slice(&self.advantage, STEPS_PER_EPOCH, device)?;
        let norm_adv = normalized_adv(adv)?;

        Ok((obs, act, ret, logprob, norm_adv))
    }

    fn reset(&mut self) {
        self.obs.clear();
        self.actions.clear();
        self.logprobs.clear();
        self.rewards.clear();
        self.state_values.clear();
        self.ptr = 0;
        self.path_start_idx = 0;
    }

    fn push_agent(&mut self, action: u32, logprob: f32, state_val: f32) {
        self.actions.push(action);
        self.logprobs.push(logprob);
        self.state_values.push(state_val);
    }

    fn push_env(&mut self, obs: Tensor, reward: f32) {
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

struct PPOAgent {
    // Current Policy
    policy: ActorCritic,
    actor_map: VarMap,
    _critic_map: VarMap,
    actor_optimizer: AdamW,
    critic_optimizer: AdamW,
}

unsafe impl Sync for PPOAgent {}
unsafe impl Send for PPOAgent {}

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
    let mut trainer_pool = TrainerPool::new();
    let first_trainer = PPOAgent::new(&device)?;
    let first_trainer = Trainer {
        policy: first_trainer.policy.actor,
        var_map: first_trainer.actor_map,
    };
    trainer_pool.push(first_trainer);

    let mut rng = rand::rng();
    let mut buffer = RolloutBuffer::new();
    let mut game_history = GameHistory::new();

    'challenger_loop: for epoch in 1..EPOCHS + 1 {
        let mut challenger = PPOAgent::new(&device)?;

        let min_games = MIN_ROUNDS_PER_TRAINER * trainer_pool.count();

        while game_history.total_games() < min_games || game_history.win_rate() < WINRATE_THRESH {
            if game_history.total_games() >= MAX_GAMES {
                if trainer_pool.count() < MAX_POOL_SIZE {
                    println!("WARNING: Adding subpar trainer to the pool");
                    break;
                } else {
                    println!("WARNING: Abandoning challenger");
                    game_history.clear();
                    continue 'challenger_loop;
                }
            }
            let mut wins = 0;
            let challenger_side = if epoch.is_multiple_of(2) {
                Side::Left
            } else {
                Side::Right
            };

            for trainer in trainer_pool.iter() {
                let round_score = fight_trainer(
                    epoch,
                    &start,
                    challenger_side,
                    &mut env,
                    &challenger,
                    trainer,
                    &mut buffer,
                    &device,
                    &mut rng,
                )?;
                update_challenger(&mut challenger, &mut buffer, &device)?;
                wins += round_score;
            }

            game_history.push(wins, trainer_pool.count());
            println!(
                "Rounds: {}, WindowRounds: {}, winrate: {}",
                game_history.total_games(),
                game_history.window_games(),
                game_history.win_rate()
            );
        }

        if epoch.is_multiple_of(SAVE_INTERVAL) {
            challenger.save(BEST_AGENT_OUTPUT_PATH)?;
        }

        let new_trainer = Trainer {
            policy: challenger.policy.actor,
            var_map: challenger.actor_map,
        };
        trainer_pool.push(new_trainer);
        game_history.clear();
    }

    println!("Completed in {:?} secs", start.elapsed());
    let (best, runner_up) = trainer_pool.get_best();
    save_model(&best.var_map, BEST_AGENT_OUTPUT_PATH)?;
    save_model(&runner_up.var_map, RUNNER_UP_OUTPUT_PATH)?;
    Ok(())
}

/// Returns (wins, games)
fn fight_trainer(
    epoch: usize,
    start: &Instant,
    challenger_side: Side,
    env: &mut Environment,
    challenger: &PPOAgent,
    trainer: &Trainer,
    buffer: &mut RolloutBuffer,
    device: &Device,
    rng: &mut ThreadRng,
) -> Result<usize> {
    let mut wins = 0;
    let mut loses = 0;

    for step in 0..STEPS_PER_EPOCH {
        let (obs, obs_inv) = env.obs_with_inv(device)?;
        let actions = take_agent_turns(challenger, trainer, buffer, &obs, &obs_inv, rng)?;

        // Update environment
        let (terminal, rewards) = env.step(actions);
        buffer.push_env(obs, rewards.agent1);

        let epoch_ended = step == STEPS_PER_EPOCH - 1;
        if terminal || epoch_ended {
            let v1 = if !terminal {
                let last_obs = env.obs(device)?;
                let (_, _, v1) = challenger.policy.step(&last_obs, rng)?;
                v1
            } else {
                0.0
            };

            if terminal {
                if env.agent1_winner() {
                    wins += 1;
                } else {
                    loses += 1;
                }                

                if epoch.is_multiple_of(EPOCH_PRINT_STEP) {
                    env.display(epoch, start.elapsed());
                }
            }

            buffer.finish_path(v1);
            env.reset_on_side(challenger_side);
        }
    }

    // Check if more wins than loses
    let more_wins = wins > loses;
    Ok(more_wins as usize)
}

fn take_agent_turns(
    challenger: &PPOAgent,
    trainer: &Trainer,
    buffer: &mut RolloutBuffer,
    obs: &Tensor,
    obs_inv: &Tensor,
    rng: &mut ThreadRng,
) -> Result<(u32, u32)> {
    let (action1, logprob, state_val) = challenger.policy.step(obs, rng)?;
    buffer.push_agent(action1, logprob, state_val);
    let action2 = get_agent_action(&trainer.policy, obs_inv, rng)?;

    Ok((action1, action2))
}

fn update_challenger(
    challenger: &mut PPOAgent,
    buffer: &mut RolloutBuffer,
    device: &Device,
) -> Result<()> {
    let (obs_batch, actions, ret, logp_old, adv) = buffer.get(device)?;
    let actions = actions.unsqueeze(1)?;

    for _ in 0..K_EPOCHS {
        let (loss_pi, kl) = challenger.compute_loss_pi(&obs_batch, &actions, &adv, &logp_old)?;
        if kl > 1.5 * TARGET_KL {
            break;
        }

        challenger.actor_optimizer.backward_step(&loss_pi)?;
    }

    for _ in 0..K_EPOCHS {
        let loss_v = challenger.compute_loss_v(&obs_batch, &ret)?;
        challenger.critic_optimizer.backward_step(&loss_v)?;
    }

    buffer.reset();

    Ok(())
}

pub fn get_agent_action(agent: &Sequential, obs: &Tensor, rng: &mut ThreadRng) -> Result<u32> {
    let estimates = agent.forward(&obs.unsqueeze(0)?)?.detach();
    let action_probs = softmax(&estimates, D::Minus1)?.squeeze(0)?.detach();
    let weights = action_probs.to_vec1::<f32>()?;
    Ok(rng.sample(WeightedIndex::new(weights).unwrap()) as u32)
}

struct GameHistory {
    history: VecDeque<(usize, usize)>,
    wins: usize,
    window_games: usize,
    total_games: usize,
}

impl GameHistory {
    fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(WINRATE_WINDOW),
            wins: 0,
            window_games: 0,
            total_games: 0,
        }
    }

    fn push(&mut self, wins: usize, games: usize) {
        if self.history.len() > WINRATE_WINDOW {
            let (wins, games) = self.history.pop_back().unwrap();
            self.wins -= wins;
            self.window_games -= games;
        }
        self.history.push_front((wins, games));
        self.wins += wins;
        self.window_games += games;
        self.total_games += games;
    }

    fn total_games(&self) -> usize {
        self.total_games
    }

    fn window_games(&self) -> usize {
        self.window_games
    }

    fn win_rate(&self) -> f32 {
        self.wins as f32 / self.window_games as f32
    }

    fn clear(&mut self) {
        self.window_games = 0;
        self.wins = 0;
        self.total_games = 0;
        self.history.clear();
    }
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
