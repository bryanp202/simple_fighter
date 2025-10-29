use crate::game::{
    GameContext, GameState, PlayerInputs,
    net::UdpListener,
    scene::{Scene, Scenes, online_play::OnlinePlay},
};

pub struct Hosting {
    current_frame: usize,
    listener: UdpListener,
}

impl Scene for Hosting {
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
        _dt: f32,
    ) -> Option<super::Scenes> {
        if let Some(connection) = self
            .listener
            .update(self.current_frame)
            .expect("Host listener failed")
        {
            Some(Scenes::OnlinePlay(OnlinePlay::new(
                connection,
                crate::game::Side::Left,
                state,
            )))
        } else {
            self.current_frame += 1;
            None
        }
    }

    fn render(
        &self,
        _canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        _global_textures: &[sdl3::render::Texture],
        _context: &GameContext,
        _state: &GameState,
    ) -> Result<(), sdl3::Error> {
        Ok(())
    }

    fn exit(&mut self, _context: &GameContext, _inputs: &mut PlayerInputs, _state: &mut GameState) {
    }
}

impl Hosting {
    pub fn new() -> Self {
        Self {
            current_frame: 0,
            listener: UdpListener::bind("0.0.0.0:5300").expect("Failed to bind"),
        }
    }
}
