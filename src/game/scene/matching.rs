use std::net::UdpSocket;

use bincode::{BorrowDecode, config};

use crate::game::{GAME_VERSION, GameContext, GameState, PlayerInputs, scene::Scene};

pub struct Matching {
    socket: UdpSocket,
}

impl Scene for Matching {
    fn enter(
        &mut self,
        _context: &GameContext,
        _inputs: &mut PlayerInputs,
        _state: &mut GameState,
    ) {
        self.socket
            .send_to(
                GAME_VERSION,
                "ec2-3-22-168-249.us-east-2.compute.amazonaws.com:8000",
            )
            .expect("failed to send to matchmaking server");
        println!("{:?}", self.socket.local_addr().unwrap());
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

    fn update(&mut self, _context: &GameContext, state: &mut GameState) -> Option<super::Scenes> {
        // if let Some(connection) = self
        //     .listener
        //     .update(self.current_frame)
        //     .expect("Host listener failed")
        // {
        //     Some(Scenes::OnlinePlay(OnlinePlay::new(
        //         connection,
        //         crate::game::Side::Left,
        //         state,
        //     )))
        // } else {
        //     self.current_frame += 1;
        //     None
        // }
        let mut buf = [0u8; 512];
        match self.socket.recv_from(&mut buf) {
            Ok((len, _)) => {
                let (matchdata, _): (MatchDataJson, usize) =
                    bincode::borrow_decode_from_slice(&buf[..len], config::standard()).unwrap();
                println!("{:?}", matchdata);
            }
            _ => {}
        }

        None
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
        // if context.should_quit() {
        //     _ = self.listener.abort(self.current_frame);
        // }
    }
}

impl Matching {
    pub fn new() -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind");
        socket
            .set_nonblocking(true)
            .expect("Failed to set non blockind");
        Self { socket }
    }
}

#[derive(BorrowDecode, Debug)]
struct MatchDataJson<'a> {
    local_is_host: bool,
    local: &'a str,
    peer: &'a str,
}
