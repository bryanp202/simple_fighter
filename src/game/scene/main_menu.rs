use crate::game::{input::ButtonFlag, scene::{round_start::RoundStart, Scene, Scenes}, GameContext};

pub struct MainMenu {
    button_pressed: bool,
}

impl Scene for MainMenu {
    fn enter(&mut self, _context: &mut GameContext) {}
    
    fn update(&mut self, context: &mut GameContext, _dt: f32) -> Option<super::Scenes> {
        let light_pressed = context.player1_inputs.held_buttons().intersects(ButtonFlag::L);
        if self.button_pressed && !light_pressed {
            Some(Scenes::RoundStart(RoundStart::new((0, 0))))
        } else {
            self.button_pressed = light_pressed;
            None
        }
    }

    fn render(
        &self,
        context: &GameContext,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &Vec<sdl3::render::Texture>
    ) -> Result<(), sdl3::Error> {
        canvas.copy(&global_textures[context.main_menu_texture], None, None)
    }

    fn exit(&mut self, _context: &mut GameContext) {
        
    }
}

impl MainMenu {
    pub fn new() -> Self {
        Self { button_pressed: false }
    }
}