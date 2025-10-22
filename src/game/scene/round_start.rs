use crate::game::{
    FRAME_RATE, GameContext,
    scene::{Scene, Scenes, gameplay::Gameplay, render_gameplay},
};

const PAUSE_DURATION: usize = FRAME_RATE * 3;

pub struct RoundStart {
    score: (u32, u32),
    timer: usize,
}

impl Scene for RoundStart {
    fn enter(&mut self, context: &mut GameContext) {
        context.player1_inputs.reset();
        context.player2_inputs.reset();
        context.player1.reset();
        context.player2.reset();
    }

    fn update(&mut self, context: &mut GameContext, _dt: f32) -> Option<super::Scenes> {
        context.player1.advance_frame();
        context.player2.advance_frame();

        self.timer += 1;
        if self.timer == PAUSE_DURATION {
            Some(Scenes::Gameplay(Gameplay::new(self.score)))
        } else {
            None
        }
    }

    fn render(
        &self,
        context: &GameContext,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &Vec<sdl3::render::Texture>,
    ) -> Result<(), sdl3::Error> {
        render_gameplay(context, canvas, global_textures, 0, self.score)
    }

    fn exit(&mut self, _context: &mut GameContext) {}
}

impl RoundStart {
    pub fn new(score: (u32, u32)) -> Self {
        Self { timer: 0, score }
    }
}
