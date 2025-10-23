use std::cmp::Ordering;

use crate::game::{
    FRAME_RATE, GameContext, SCORE_TO_WIN,
    character::Character,
    physics::{check_hit_collisions, movement_system, side_detection},
    scene::{Scene, Scenes, main_menu::MainMenu, render_gameplay, round_start::RoundStart},
};

const ROUND_LEN: usize = 99;

pub struct Gameplay {
    hit_freeze: usize,
    score: (u32, u32),
    time: usize,
}

impl Gameplay {
    pub fn new(score: (u32, u32)) -> Self {
        Self {
            hit_freeze: 0,
            score,
            time: 0,
        }
    }

    fn check_round_end(&mut self, context: &GameContext) -> Option<Scenes> {
        let player1_hp_ratio = context.player1.current_hp() / context.player1.max_hp();
        let player2_hp_ratio = context.player2.current_hp() / context.player2.max_hp();
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
                Some(Scenes::RoundStart(RoundStart::new(self.score)))
            }
            (SCORE_TO_WIN, _) => {
                if cfg!(feature = "debug") {
                    println!("Player1 wins!");
                }
                Some(Scenes::MainMenu(MainMenu::new()))
            }
            (_, SCORE_TO_WIN) => {
                if cfg!(feature = "debug") {
                    println!("Player2 wins!");
                }
                Some(Scenes::MainMenu(MainMenu::new()))
            }
            _ => Some(Scenes::RoundStart(RoundStart::new(self.score))),
        }
    }
}

impl Scene for Gameplay {
    fn enter(&mut self, _context: &mut crate::game::GameContext) {}

    fn update(
        &mut self,
        context: &mut crate::game::GameContext,
        _dt: f32,
    ) -> Option<super::Scenes> {
        if self.hit_freeze == 0 {
            context.player1.movement_update();
            context.player2.movement_update();

            let (player1_pos, player2_pos) = movement_system(
                context.player1.get_side(),
                &context.player1.pos(),
                &context.player1.get_collision_box(),
                context.player2.get_side(),
                &context.player2.pos(),
                &context.player2.get_collision_box(),
                &context.stage,
            );
            context.player1.set_pos(player1_pos);
            context.player2.set_pos(player2_pos);

            if let Some(player1_side) =
                side_detection(&context.player1.pos(), &context.player2.pos())
            {
                context.player1.set_side(player1_side);
                context.player2.set_side(player1_side.opposite());
            }

            self.hit_freeze = handle_hit_boxes(&mut context.player1, &mut context.player2);

            context.player1.advance_frame();
            context.player2.advance_frame();

            self.time += 1;
        } else {
            self.hit_freeze -= 1;
        }

        context.player1.update(&context.player1_inputs);
        context.player2.update(&context.player2_inputs);

        self.check_round_end(context)
    }

    fn render(
        &self,
        context: &GameContext,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &Vec<sdl3::render::Texture>,
    ) -> Result<(), sdl3::Error> {
        render_gameplay(context, canvas, global_textures, self.time, self.score)
    }

    fn exit(&mut self, _context: &mut GameContext) {}
}

// Returns the amount of frames for hit freeze
fn handle_hit_boxes(player1: &mut Character, player2: &mut Character) -> usize {
    let player1_pos = player1.pos();
    let player1_side = player1.get_side();
    let player2_pos = player2.pos();
    let player2_side = player2.get_side();

    let player1_hit_boxes = player1.get_hit_boxes();
    let player2_hurt_boxes = player2.get_hurt_boxes();
    let player1_hit = check_hit_collisions(
        player1_side,
        player1_pos,
        player1_hit_boxes,
        player2_side,
        player2_pos,
        player2_hurt_boxes,
    );

    let player2_hit_boxes = player2.get_hit_boxes();
    let player1_hurt_boxes = player1.get_hurt_boxes();
    let player2_hit = check_hit_collisions(
        player2_side,
        player2_pos,
        player2_hit_boxes,
        player1_side,
        player1_pos,
        player1_hurt_boxes,
    );

    match (player1_hit, player2_hit) {
        (Some(player1_hit), None) => {
            let blocked = player2.receive_hit(&player1_hit);
            player1.successful_hit(&player1_hit, blocked);
            4
        }
        (None, Some(player2_hit)) => {
            let blocked = player1.receive_hit(&player2_hit);
            player2.successful_hit(&player2_hit, blocked);
            4
        }
        (Some(player1_hit), Some(player2_hit)) => {
            player1.successful_hit(&player1_hit, true);
            player2.successful_hit(&player2_hit, true);
            8
        }
        _ => 0,
    }
}
