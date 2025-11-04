use crate::game::{
    GameContext, GameState, PlayerInputs,
    net::matching::{MatchingSocket, PeerConnectionType},
    scene::{Scene, Scenes, connecting::Connecting, hosting::Hosting},
};

pub struct Matching {
    socket: MatchingSocket,
    current_frame: usize,
}

impl Scene for Matching {
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

    fn update(&mut self, _context: &GameContext, _state: &mut GameState) -> Option<Scenes> {
        if let Some(connection) = self
            .socket
            .update(self.current_frame)
            .expect("Match socket failed")
        {
            match connection {
                PeerConnectionType::Hosting(listener) => {
                    Some(Scenes::Hosting(Hosting::new(listener)))
                }
                PeerConnectionType::Joining(client) => {
                    Some(Scenes::Connecting(Connecting::new(client)))
                }
            }
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

impl Matching {
    pub fn new(server_addr: &str) -> Self {
        Self {
            socket: MatchingSocket::bind("0.0.0.0:0", server_addr)
                .expect("Failed to bind matching socket"),
            current_frame: 0,
        }
    }
}
