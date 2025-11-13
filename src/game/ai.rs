use std::cmp::Ordering;

use candle_core::{Device, Result, Tensor};
use candle_nn::{Sequential, VarMap};
use sdl3::render::FPoint;

use crate::game::{GameContext, GameState};
pub mod dqn;
pub mod ppo;

// Environment
const STATE_VECTOR_LEN: usize = 35 + 35 + 3;
const ACTION_SPACE: usize = 9 * 8;

// REWARDS
const ROUND_WIN_SCORE: f32 = 25.0;
const ROUND_LOSE_SCORE: f32 = -0.5;
const ROUND_TIE_SCORE: f32 = -50.0;

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

pub fn load_model(filepath: &str, device: &Device) -> Result<(VarMap, Sequential)> {
    let mut var_map = VarMap::new();
    var_map.load(filepath)?;
    let agent = dqn::make_model(&var_map, device)?;
    Ok((var_map, agent))
}

fn save_model(var_map: &VarMap, filename: &str) -> Result<()> {
    if let Some(parent) = std::path::Path::new(filename).parent() {
        std::fs::create_dir_all(parent)?;
    }
    var_map.save(filename)?;
    println!("Model weights saved successfully to {}", filename);
    Ok(())
}

fn copy_var_map(source: &VarMap, destination: &mut VarMap) -> Result<()> {
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
