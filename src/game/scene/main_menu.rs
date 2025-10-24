use crate::game::{
    GameContext, GameState,
    input::ButtonFlag,
    scene::{Scene, Scenes, round_start::RoundStart},
};

pub struct MainMenu {
    button_pressed: bool,
}

impl Scene for MainMenu {
    fn enter(&mut self, _context: &GameContext, _state: &mut GameState) {}

    fn update(
        &mut self,
        _context: &GameContext,
        state: &mut GameState,
        _dt: f32,
    ) -> Option<super::Scenes> {
        let light_pressed = state
            .player1_inputs
            .held_buttons()
            .intersects(ButtonFlag::L);
        if self.button_pressed && !light_pressed {
            Some(Scenes::RoundStart(RoundStart::new((0, 0))))
        } else {
            self.button_pressed = light_pressed;
            None
        }
    }

    fn render(
        &self,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &Vec<sdl3::render::Texture>,
        context: &GameContext,
        _state: &GameState,
    ) -> Result<(), sdl3::Error> {
        canvas.copy(&global_textures[context.main_menu_texture], None, None)
    }

    fn exit(&mut self, _context: &GameContext, _state: &mut GameState) {}
}

impl MainMenu {
    pub fn new() -> Self {
        Self {
            button_pressed: false,
        }
    }
}
