use std::{cmp::Ordering, time::Duration};

use candle_core::{Device, Result, Tensor};
use rand::{Rng, rngs::ThreadRng};
use sdl3::render::FPoint;

use crate::game::{
    GameContext, GameState, PlayerInputs, Side,
    ai::{DuelFloat, observation_with_inv, serialize_observation, take_agent_turn},
    scene::gameplay::{GameplayScene, during_round::DuringRound},
};

pub struct Environment<'a> {
    scene: DuringRound,
    context: &'a GameContext,
    inputs: &'a mut PlayerInputs,
    state: &'a mut GameState,

    accumulate_rewards: DuelFloat,
}

// REWARDS
const ROUND_WIN_SCORE: f32 = 25.0;
const ROUND_LOSE_SCORE: f32 = -12.5;
const ROUND_TIE_SCORE: f32 = -50.0;

impl<'a> Environment<'a> {
    pub fn new(
        context: &'a GameContext,
        inputs: &'a mut PlayerInputs,
        state: &'a mut GameState,
    ) -> Self {
        Self {
            scene: DuringRound::new((0, 0)),
            context,
            inputs,
            state,
            accumulate_rewards: DuelFloat::default(),
        }
    }

    pub fn reset(&mut self) {
        self.accumulate_rewards = DuelFloat::default();
        self.scene = DuringRound::new((0, 0));
        self.state.reset(self.context);
        self.inputs.reset_player1();
        self.inputs.reset_player2();
    }

    pub fn reset_rng(&mut self, rng: &mut ThreadRng) {
        self.accumulate_rewards = DuelFloat::default();
        let timer = rng.random_range(0.0..0.4);
        self.scene = DuringRound::new_with_timer((0, 0), timer);

        self.inputs.reset_player1();
        self.inputs.reset_player2();

        self.state.player1_inputs.reset();
        self.state.player2_inputs.reset();

        let stage_bounds = self.context.stage.width() - 200.0;
        let pos_x1 = rng.random_range(-stage_bounds..stage_bounds);
        let pos_x2 = pos_x1 + rng.random_range(60.0..140.0);

        let ((x1, x2), (side1, side2)) = if rng.random_bool(0.5) {
            ((pos_x1, pos_x2), (Side::Left, Side::Right))
        } else {
            ((pos_x2, pos_x1), (Side::Right, Side::Left))
        };

        self.state
            .player1
            .reset_to(&self.context.player1, FPoint::new(x1, 0.0), side1);
        self.state
            .player2
            .reset_to(&self.context.player1, FPoint::new(x2, 0.0), side2);
    }

    pub fn display(&self, epoch: usize, elapsed: Duration) {
        println!("___________________________");
        println!("EPOCH: {epoch}, TIME: {elapsed:?}");
        println!("Accumulate game sum: {:?}", self.accumulate_rewards);
        println!("Round timer: {}", 1.0 - self.scene.timer());
        println!("Agent1: {:?}", self.state.player1);
        println!("~~~~~~~~~~~~~~~~~~~~~~~~~~~");
        println!("Agent2: {:?}", self.state.player2);
        println!("___________________________\n");
    }

    pub fn obs(&self, device: &Device) -> Result<Tensor> {
        let timer = self.scene.timer();
        serialize_observation(self.context, self.state, timer, device)
    }

    /// Returns the state obs tensor with (player1 first in vec, player2 first in vec)
    ///
    /// Used so that the critic can have a common reference point to evaluate states in AC algorithms
    pub fn obs_with_inv(&self, device: &Device) -> Result<(Tensor, Tensor)> {
        let timer = self.scene.timer();
        observation_with_inv(self.context, self.state, timer, device)
    }

    pub fn step(&mut self, actions: (u32, u32)) -> (bool, DuelFloat) {
        take_agent_turn(
            &mut self.inputs.player1,
            &mut self.state.player1_inputs,
            actions.0,
        );
        take_agent_turn(
            &mut self.inputs.player2,
            &mut self.state.player2_inputs,
            actions.1,
        );

        let old_pos = (self.state.player1.pos(), self.state.player2.pos());
        let old_hp = (
            self.state.player1.hp_per(&self.context.player1),
            self.state.player2.hp_per(&self.context.player2),
        );
        let old_combo = (
            self.state.player1.combo_scaling(),
            self.state.player2.combo_scaling(),
        );
        let old_score = self.scene.score();

        let terminal = self.scene.update(self.context, self.state).is_some();

        let rewards = self.reward(old_pos, old_hp, old_combo, old_score);
        self.accumulate_rewards.agent1 += rewards.agent1;
        self.accumulate_rewards.agent2 += rewards.agent2;

        (terminal, rewards)
    }

    /// Not a zero sum game
    ///
    /// Return value as is represents the reward for agent1, and the negation is the reward for agent2
    fn reward(
        &self,
        old_pos: (FPoint, FPoint),
        old_hp: (f32, f32),
        old_combo: (f32, f32),
        old_score: (u32, u32),
    ) -> DuelFloat {
        let timer = 1.0 - self.scene.timer();
        let new_score = self.scene.score();
        let new_combo = (
            self.state.player1.combo_scaling(),
            self.state.player2.combo_scaling(),
        );
        let new_pos = (self.state.player1.pos(), self.state.player2.pos());
        let new_hp = (
            self.state.player1.hp_per(&self.context.player1),
            self.state.player2.hp_per(&self.context.player2),
        );

        let (round_rwd1, round_rwd2) = match new_score.0.cmp(&new_score.1) {
            Ordering::Less => {
                // Player 2 wins
                if new_hp.0 <= 0.0 {
                    // Gets a higher score for winning with more hp
                    (
                        ROUND_LOSE_SCORE,
                        ROUND_WIN_SCORE * (1.0 + new_hp.1 - new_hp.0 + timer) / 3.0,
                    )
                } else {
                    (ROUND_LOSE_SCORE * 2.0, ROUND_WIN_SCORE / 4.0)
                }
            }
            Ordering::Equal => {
                // Tie, figure out if game still going
                if new_score.0 > old_score.0 {
                    (ROUND_TIE_SCORE, ROUND_TIE_SCORE)
                } else {
                    (-0.002, -0.002)
                }
            }
            Ordering::Greater => {
                // Player 1 wins
                if new_hp.1 <= 0.0 {
                    // Gets a higher score for winning with more hp
                    (
                        ROUND_WIN_SCORE * (1.0 + new_hp.0 - new_hp.1 + timer) / 3.0,
                        ROUND_LOSE_SCORE,
                    )
                } else {
                    (ROUND_WIN_SCORE / 4.0, ROUND_LOSE_SCORE * 2.0)
                }
            }
        };

        let dmg_rwd1 = (old_hp.1 - new_hp.1) * 10.0;
        let dmg_rwd2 = (old_hp.0 - new_hp.0) * 10.0;

        let combo_rwd1 = (old_combo.1 - new_combo.1).max(0.0) * 10.0;
        let combo_rwd2 = (old_combo.0 - new_combo.0).max(0.0) * 10.0;

        // If agent made an action to get closer then reward it
        let approached_1 =
            (old_pos.1.x - new_pos.0.x).abs().max(60.0) < (old_pos.1.x - old_pos.0.x).abs();
        let approach_rwd1 = approached_1 as u8 as f32 * 0.02;
        let approached_2 =
            (old_pos.0.x - new_pos.1.x).abs().max(60.0) < (old_pos.0.x - old_pos.1.x).abs();
        let approach_rwd2 = approached_2 as u8 as f32 * 0.02;

        let dmg_penalty1 = dmg_rwd2 * 0.8;
        let dmg_penalty2 = dmg_rwd1 * 0.8;

        let agent1 = round_rwd1 + dmg_rwd1 + combo_rwd1 + approach_rwd1 - dmg_penalty1;
        let agent2 = round_rwd2 + dmg_rwd2 + combo_rwd2 + approach_rwd2 - dmg_penalty2;

        DuelFloat { agent1, agent2 }
    }
}
