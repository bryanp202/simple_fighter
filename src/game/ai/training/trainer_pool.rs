use std::{collections::VecDeque, io::Write, time::Instant};

use candle_core::{Device, Result, Tensor};
use candle_nn::{Sequential, VarMap};
use rand::rngs::ThreadRng;

use crate::game::{Side, ai::{env::Environment, ppo::{PPOAgent, RolloutBuffer, get_agent_action}, save_model}};

const MAX_POOL_SIZE: usize = 20;
const WINRATE_THRESH: f32 = 0.60;
const WINRATE_WINDOW: usize = 32;
const MIN_ROUNDS_PER_TRAINER: usize = 16;
const MAX_GAMES: usize = 3000;
const STEPS_PER_EPOCH: usize = 8_000;

const EPOCHS: usize = 32;
const BEST_AGENT_OUTPUT_PATH: &str = "./ai/ppo/best_NEW.safetensors";
const RUNNER_UP_OUTPUT_PATH: &str = "./ai/ppo/runner_up_NEW.safetensors";

struct Trainer {
    policy: Sequential,
    var_map: VarMap,
}

impl Trainer {
    fn from_ppo_aget(agent: PPOAgent) -> Self {
        let (policy, var_map) = agent.into_policy();
        Self {
            policy,
            var_map,
        }
    }

    fn save(&self, filename: &str) -> Result<()> {
        save_model(&self.var_map, filename)
    }
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
        if self.history.len() >= WINRATE_WINDOW {
            let (old_wins, old_games) = self.history.pop_back().unwrap();
            self.wins -= old_wins;
            self.window_games -= old_games;
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

#[allow(dead_code)]
pub fn train(mut env: Environment<'_>, device: Device, start: Instant) -> Result<()> {
    let mut trainer_pool = TrainerPool::new();
    let first_trainer = Trainer::from_ppo_aget(PPOAgent::new(&device)?);
    trainer_pool.push(first_trainer);

    let mut rng = rand::rng();
    let mut buffer = RolloutBuffer::new(STEPS_PER_EPOCH);
    let mut game_history = GameHistory::new();

    'challenger_loop: for epoch in 1..EPOCHS + 1 {
        let mut challenger = PPOAgent::new(&device)?;
        let min_games = MIN_ROUNDS_PER_TRAINER * trainer_pool.count();

        println!("Challenger #{epoch}, Trainers: {}", trainer_pool.count());

        while game_history.total_games() < min_games || game_history.win_rate() < WINRATE_THRESH {
            if game_history.total_games() >= MAX_GAMES {
                if trainer_pool.count() < MAX_POOL_SIZE {
                    print!("\nWARNING: Adding subpar trainer to the pool");
                    break;
                } else {
                    println!("\nWARNING: Abandoning challenger");
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
                    challenger_side,
                    &mut env,
                    &challenger,
                    trainer,
                    &mut buffer,
                    &device,
                    &mut rng,
                )?;
                challenger.update(&buffer, &device)?;
                buffer.reset();
                wins += round_score;
            }

            game_history.push(wins, trainer_pool.count());
            print!(
                "\r\x1b[KRounds: {}, WindowRounds: {}, winrate: {}",
                game_history.total_games(),
                game_history.window_games(),
                game_history.win_rate()
            );
            std::io::stdout().flush().unwrap();
        }
        println!();
        challenger.save(BEST_AGENT_OUTPUT_PATH)?;

        let new_trainer = Trainer::from_ppo_aget(challenger);
        trainer_pool.push(new_trainer);
        game_history.clear();
    }

    println!("Completed in {:?} secs", start.elapsed());
    let (best, runner_up) = trainer_pool.get_best();
    best.save(BEST_AGENT_OUTPUT_PATH)?;
    runner_up.save(RUNNER_UP_OUTPUT_PATH)?;
    Ok(())
}

/// Returns (wins, games)
fn fight_trainer(
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
                let (_, _, v1) = challenger.step(&last_obs, rng)?;
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
    let (action1, logprob, state_val) = challenger.step(obs, rng)?;
    buffer.push_agent(action1, logprob, state_val);
    let action2 = get_agent_action(&trainer.policy, obs_inv, rng)?;

    Ok((action1, action2))
}