use candle_core::{Device, Result, Tensor};
use candle_nn::{Sequential, VarMap};
use rand::rngs::ThreadRng;

use crate::game::{
    GameContext, GameState, PlayerInputs, input::{ButtonFlag, Direction, InputHistory, Inputs}
};
mod dqn;
mod env;
mod ppo;

// Environment
const STATE_VECTOR_LEN: usize = 35 + 35 + 3;
const ACTION_SPACE: usize = 9 * 8;

type Action = u32;
#[derive(Clone, Copy)]
struct Actions {
    agent1: Action,
    agent2: Action,
}

#[derive(Clone, Copy, Debug, Default)]
struct DuelFloat {
    agent1: f32,
    agent2: f32,
}

/// Interface used for current AI implementation
pub fn get_agent_action(agent: &Sequential, obs: &Tensor, rng: &mut ThreadRng) -> Result<u32> {
    ppo::get_agent_action(agent, obs, rng)
}

/// Interface used for training
pub fn train(context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) -> Result<()> {
    ppo::train(context, inputs, state)
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

pub fn take_agent_turn(inputs_history: &mut InputHistory, inputs: &mut Inputs, action: u32) {
    let (dir, buttons) = map_ai_action(action);

    inputs_history.skip();
    inputs_history.append_input(0, dir, buttons);

    inputs.update(
        inputs_history.held_buttons(),
        inputs_history.parse_history(),
    );
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
