use crate::game::{
    GameContext, GameState, PlayerInputs,
    input::ButtonFlag,
    scene::{
        Scene, Scenes, local_play::LocalPlay, matching::Matching, spectate_ai::SpectateAi,
        verses_ai::VersesAi,
    },
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

    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Option<super::Scenes> {
        let just_pressed = state.player1_inputs.just_pressed_buttons();
        let held = state.player1_inputs.active_buttons();

        if self.l_button_pressed && !ButtonFlag::L.intersects(held) {
            return Some(Scenes::LocalPlay(LocalPlay::new()));
        }
        if self.m_button_pressed && !ButtonFlag::M.intersects(held) {
            return Some(Scenes::Matching(Matching::new(&context.matchmaking_server)));
        }
        if self.h_button_pressed && !ButtonFlag::H.intersects(held) {
            return Some(Scenes::SpectateAi(SpectateAi::new(
                &context.left_agent_filepath,
                &context.right_agent_filepath,
            )));
            // return Some(Scenes::VersesAi(VersesAi::new(
            //    &context.right_agent_filepath,
            // )));
        }

        self.l_button_pressed = self.l_button_pressed || ButtonFlag::L.intersects(just_pressed);
        self.m_button_pressed = self.m_button_pressed || ButtonFlag::M.intersects(just_pressed);
        self.h_button_pressed = self.h_button_pressed || ButtonFlag::H.intersects(just_pressed);

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
