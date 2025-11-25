use crate::game::{
    GameContext, GameState, PlayerInputs,
    net::client::UdpClient,
    scene::{Scene, Scenes, online_play::OnlinePlay},
};

pub struct Connecting {
    current_frame: usize,
    client: UdpClient,
}

impl Scene for Connecting {
    fn enter(&mut self, _context: &GameContext, inputs: &mut PlayerInputs, _state: &mut GameState) {
        inputs.online_key_mapping();
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

    fn update(&mut self, _context: &GameContext, state: &mut GameState) -> Result<Option<Scenes>, String> {
        if let Some(connection) = self
            .client
            .update(self.current_frame)
            .map_err(|err| err.to_string())?
        {
            Ok(Some(Scenes::OnlinePlay(OnlinePlay::new(
                connection,
                crate::game::Side::Right,
                state,
            ))))
        } else {
            self.current_frame += 1;
            Ok(None)
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

    fn exit(&mut self, context: &GameContext, _inputs: &mut PlayerInputs, _state: &mut GameState) {
        if context.should_quit() {
            _ = self.client.abort(self.current_frame);
        }
    }
}

impl Connecting {
    pub fn new(client: UdpClient) -> Self {
        Self {
            current_frame: 0,
            client,
        }
    }
}
