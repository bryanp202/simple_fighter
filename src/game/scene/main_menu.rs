use sdl3::{pixels::Color, render::FRect};

use crate::game::{
    GameContext, GameState, PlayerInputs,
    input::{ButtonFlag, Direction},
    scene::{
        Scene, Scenes, local_play::LocalPlay, matching::Matching, spectate_ai::SpectateAi,
        verses_ai::VersesAi,
    },
};

const MAIN_MENU_OPTIONS: i32 = 4;

pub struct MainMenu {
    l_button_pressed: bool,
    last_dir: Direction,
    scroll_pos: i32,
}

impl Scene for MainMenu {
    fn enter(
        &mut self,
        _context: &GameContext,
        _inputs: &mut PlayerInputs,
        _state: &mut GameState,
    ) {
    }

    fn handle_input(
        &mut self,
        _context: &GameContext,
        inputs: &mut crate::game::PlayerInputs,
        _state: &mut GameState,
    ) -> Result<(), String> {
        inputs.update_player1();
        inputs.skip_player2();
        Ok(())
    }

    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Result<Option<super::Scenes>, String> {
        let just_pressed = state.player1_inputs.just_pressed_buttons();
        let held = state.player1_inputs.active_buttons();

        if self.l_button_pressed && !ButtonFlag::L.intersects(held) {
            return Ok(Some(self.select_scene(context)?));
        }

        let held_dir = state.player1_inputs.dir();

        if held_dir != self.last_dir {
            let scroll_dif = match state.player1_inputs.dir() {
                Direction::Down => 1,
                Direction::Up => -1,
                _ => 0,
            };
            self.scroll_pos =
                (MAIN_MENU_OPTIONS + self.scroll_pos + scroll_dif) % MAIN_MENU_OPTIONS;
            self.last_dir = held_dir;
        }

        self.l_button_pressed = self.l_button_pressed || ButtonFlag::L.intersects(just_pressed);

        Ok(None)
    }

    fn render(
        &self,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &[sdl3::render::Texture],
        context: &GameContext,
        _state: &GameState,
    ) -> Result<(), sdl3::Error> {
        canvas.copy(&global_textures[context.main_menu_texture], None, None)?;
        let (w, h) = canvas.window().size();
        let w = w as f32;
        let h = h as f32;

        let rect_w = w / 30.0;
        let rect_h = h / 16.875;
        let x = w * 3.0 / 10.0;
        let y_start = h * 5.0 / 12.0;
        let y = y_start + (self.scroll_pos * 2) as f32 * rect_h;

        let rect = FRect::new(x, y, rect_w, rect_h);
        canvas.set_draw_color(Color::BLACK);
        canvas.fill_rect(rect)?;

        Ok(())
    }

    fn exit(&mut self, _context: &GameContext, _inputs: &mut PlayerInputs, _state: &mut GameState) {
    }
}

impl MainMenu {
    pub fn new() -> Self {
        Self {
            l_button_pressed: false,
            last_dir: Direction::Neutral,
            scroll_pos: 0,
        }
    }

    fn select_scene(&self, context: &GameContext) -> Result<Scenes, String> {
        let scene = match self.scroll_pos {
            0 => Scenes::LocalPlay(LocalPlay::new()),
            1 => Scenes::VersesAi(VersesAi::new(&context.left_agent_filepath)?),
            2 => Scenes::SpectateAi(SpectateAi::new(
                &context.left_agent_filepath,
                &context.right_agent_filepath,
            )?),
            3 => Scenes::Matching(Matching::new(&context.matchmaking_server)?),
            _ => return Err(String::from("Invalid scene selected")),
        };

        Ok(scene)
    }
}
