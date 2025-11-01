use crate::game::{
    GameContext, GameState, PlayerInputs,
    input::ButtonFlag,
    scene::{Scene, Scenes, hosting::Hosting, local_play::LocalPlay, connecting::Connecting},
};

pub struct MainMenu {
    l_button_pressed: bool,
    m_button_pressed: bool,
    h_button_pressed: bool,
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
    ) -> std::io::Result<()> {
        inputs.update_player1();
        inputs.skip_player2();
        Ok(())
    }

    fn update(
        &mut self,
        _context: &GameContext,
        state: &mut GameState,
    ) -> Option<super::Scenes> {
        let buttons = state.player1_inputs.active_buttons();

        if self.l_button_pressed && !ButtonFlag::L.intersects(buttons) {
            return Some(Scenes::LocalPlay(LocalPlay::new()));
        }
        if self.m_button_pressed && !ButtonFlag::M.intersects(buttons) {
            return Some(Scenes::Hosting(Hosting::new()));
        }
        if self.h_button_pressed && !ButtonFlag::H.intersects(buttons) {
            return Some(Scenes::Connecting(Connecting::new()));
        }

        self.l_button_pressed = ButtonFlag::L.intersects(buttons);
        self.m_button_pressed = ButtonFlag::M.intersects(buttons);
        self.h_button_pressed = ButtonFlag::H.intersects(buttons);

        None
    }

    fn render(
        &self,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &[sdl3::render::Texture],
        context: &GameContext,
        _state: &GameState,
    ) -> Result<(), sdl3::Error> {
        canvas.copy(&global_textures[context.main_menu_texture], None, None)
    }

    fn exit(&mut self, _context: &GameContext, _inputs: &mut PlayerInputs, _state: &mut GameState) {
    }
}

impl MainMenu {
    pub fn new() -> Self {
        Self {
            l_button_pressed: false,
            m_button_pressed: false,
            h_button_pressed: false,
        }
    }
}
