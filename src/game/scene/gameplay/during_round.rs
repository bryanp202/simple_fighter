use std::cmp::Ordering;

use crate::game::{
    FRAME_RATE, GameContext, GameState, SCORE_TO_WIN,
    physics::{check_hit_collisions, movement_system, side_detection},
    scene::gameplay::{
        GameplayScene, GameplayScenes, ROUND_LEN, render_gameplay, round_start::RoundStart,
    },
};

#[derive(Clone, PartialEq)]
pub struct DuringRound {
    hit_freeze: usize,
    score: (u32, u32),
    time: usize,
}

impl DuringRound {
    pub fn new(score: (u32, u32)) -> Self {
        Self {
            hit_freeze: 0,
            score,
            time: 0,
        }
    }

    fn check_round_end(
        &mut self,
        context: &GameContext,
        state: &GameState,
    ) -> Option<GameplayScenes> {
        let player1_hp_ratio = state.player1.hp_per(&context.player1);
        let player2_hp_ratio = state.player2.hp_per(&context.player2);
        match (player1_hp_ratio, player2_hp_ratio) {
            (0.0, 0.0) => self.score = (self.score.0 + 1, self.score.1 + 1),
            (0.0, _) => self.score.1 += 1,
            (_, 0.0) => self.score.0 += 1,
            _ => {
                if self.time == ROUND_LEN * FRAME_RATE {
                    match player1_hp_ratio.partial_cmp(&player2_hp_ratio) {
                        Some(Ordering::Less) => self.score.1 += 1,
                        Some(Ordering::Equal) => self.score = (self.score.0 + 1, self.score.1 + 1),
                        Some(Ordering::Greater) => self.score.0 += 1,
                        None => self.score = (self.score.0 + 1, self.score.1 + 1),
                    }
                } else {
                    // Timer not over so should return no scene transition
                    return None;
                }
            }
        }

        match self.score {
            (SCORE_TO_WIN, SCORE_TO_WIN) => {
                self.score = (SCORE_TO_WIN - 1, SCORE_TO_WIN - 1);
                Some(GameplayScenes::RoundStart(RoundStart::new(self.score)))
            }
            (SCORE_TO_WIN, _) => {
                if cfg!(feature = "debug") {
                    println!("Player1 wins!");
                }
                Some(GameplayScenes::Exit)
            }
            (_, SCORE_TO_WIN) => {
                if cfg!(feature = "debug") {
                    println!("Player2 wins!");
                }
                Some(GameplayScenes::Exit)
            }
            _ => Some(GameplayScenes::RoundStart(RoundStart::new(self.score))),
        }
    }
}

impl GameplayScene for DuringRound {
    fn enter(&mut self, _context: &GameContext, _state: &mut GameState) {}

    fn update(
        &mut self,
        context: &GameContext,
        state: &mut GameState,
        _dt: f32,
    ) -> Option<GameplayScenes> {
        // Side check first to prevent flickering
        if let Some(player1_side) = side_detection(&state.player1.pos(), &state.player2.pos()) {
            state.player1.set_side(&context.player1, player1_side);
            state
                .player2
                .set_side(&context.player2, player1_side.opposite());
        }
        state
            .player1
            .state_update(&state.player1_inputs, &context.player1);
        state
            .player2
            .state_update(&state.player2_inputs, &context.player2);

        if self.hit_freeze == 0 {
            state.player1.movement_update(&context.player1);
            state.player2.movement_update(&context.player2);

            let (player1_pos, player2_pos) = movement_system(
                &state.player1.side(),
                &state.player1.pos(),
                &state.player1.get_collision_box(&context.player1),
                &state.player2.side(),
                &state.player2.pos(),
                &state.player2.get_collision_box(&context.player2),
                &context.stage,
            );
            state.player1.set_pos(player1_pos);
            state.player2.set_pos(player2_pos);

            self.hit_freeze = handle_hit_boxes(state, context);

            state.player1.advance_frame();
            state.player2.advance_frame();

            self.time += 1;
        } else {
            self.hit_freeze -= 1;
        }

        self.check_round_end(context, state)
    }

    fn render(
        &self,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &Vec<sdl3::render::Texture>,
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error> {
        render_gameplay(
            canvas,
            global_textures,
            context,
            state,
            self.time,
            self.score,
        )
    }

    fn exit(&mut self, _context: &GameContext, _state: &mut GameState) {}
}

// Returns the amount of frames for hit freeze
fn handle_hit_boxes(state: &mut GameState, context: &GameContext) -> usize {
    let player1_pos = state.player1.pos();
    let player1_side = state.player1.side();
    let player2_pos = state.player2.pos();
    let player2_side = state.player2.side();

    let player1_hit_boxes = state.player1.get_hit_boxes(&context.player1);
    let player2_hurt_boxes = state.player2.get_hurt_boxes(&context.player2);
    let player1_hit = check_hit_collisions(
        &player1_side,
        player1_pos,
        player1_hit_boxes,
        &player2_side,
        player2_pos,
        player2_hurt_boxes,
    );

    let player2_hit_boxes = state.player2.get_hit_boxes(&context.player2);
    let player1_hurt_boxes = state.player1.get_hurt_boxes(&context.player1);
    let player2_hit = check_hit_collisions(
        &player2_side,
        player2_pos,
        player2_hit_boxes,
        &player1_side,
        player1_pos,
        player1_hurt_boxes,
    );

    match (player1_hit, player2_hit) {
        (Some(player1_hit), None) => {
            let blocked = state.player2.receive_hit(&context.player1, &player1_hit);
            state
                .player1
                .successful_hit(&context.player1, &player1_hit, blocked);
            4
        }
        (None, Some(player2_hit)) => {
            let blocked = state.player1.receive_hit(&context.player1, &player2_hit);
            state
                .player2
                .successful_hit(&context.player2, &player2_hit, blocked);
            4
        }
        (Some(player1_hit), Some(player2_hit)) => {
            state
                .player1
                .successful_hit(&context.player1, &player1_hit, true);
            state
                .player2
                .successful_hit(&context.player2, &player2_hit, true);
            8
        }
        _ => 0,
    }
}
