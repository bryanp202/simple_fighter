use sdl3::render::FPoint;

use crate::game::{
    FRAME_RATE, GameContext, GameState, SCORE_TO_WIN,
    scene::{Scene, Scenes, gameplay::Gameplay, render_gameplay},
};

const PAUSE_DURATION: u32 = ROUND_DISPLAY_DURATION + FIGHT_DISPLAY_DURATION;
const ROUND_DISPLAY_DURATION: u32 = (FRAME_RATE as f64 * 2.0) as u32;
const FIGHT_DISPLAY_DURATION: u32 = (FRAME_RATE as f64 * 1.0) as u32;

#[derive(Clone)]
pub struct RoundStart {
    score: (u32, u32),
    round: u32,
    timer: u32,
}

impl Scene for RoundStart {
    fn enter(&mut self, context: &GameContext, state: &mut GameState) {
        state.player1.reset(&context.player1);
        state.player2.reset(&context.player2);
    }

    fn update(
        &mut self,
        _context: &GameContext,
        state: &mut GameState,
        _dt: f32,
    ) -> Option<super::Scenes> {
        state.player1.advance_frame();
        state.player2.advance_frame();

        self.timer += 1;
        if self.timer == PAUSE_DURATION {
            Some(Scenes::Gameplay(Gameplay::new(self.score)))
        } else {
            None
        }
    }

    fn render(
        &self,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &Vec<sdl3::render::Texture>,
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error> {
        render_gameplay(canvas, global_textures, context, state, 0, self.score)?;

        let text_frame = if self.timer < ROUND_DISPLAY_DURATION {
            self.round as usize
        } else {
            context.round_start_animation.get_frame_count() - 1
        };
        context.camera.render_animation(
            canvas,
            global_textures,
            &FPoint::new(0.0, 240.0),
            &context.round_start_animation,
            text_frame,
        )
    }

    fn exit(&mut self, _context: &GameContext, _state: &mut GameState) {}
}

impl RoundStart {
    pub fn new(score: (u32, u32)) -> Self {
        let round = (score.0 + score.1).min(SCORE_TO_WIN);
        Self {
            timer: 0,
            score,
            round,
        }
    }
}
