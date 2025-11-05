use std::net::{SocketAddr, UdpSocket};

use crate::game::net::{
    BUFFER_LEN, GAME_START_DELAY, GameMessage, MessageContent, PEER_TIME_OUT, recv_msg, send_msg,
    stream::UdpStream,
};

enum UdpHostState {
    Listening,
    Syncing((usize, usize)), // local frame offset, peer frame offset, peer addr
    Connecting(usize),
    Connected,
}

pub struct UdpHost {
    socket: UdpSocket,
    client_addr: SocketAddr,
    state: UdpHostState,
    recv_buf: [u8; BUFFER_LEN],
    send_buf: [u8; BUFFER_LEN],
}

impl UdpHost {
    pub fn new(connection: UdpSocket, peer_addr: SocketAddr) -> Self {
        Self {
            socket: connection,
            client_addr: peer_addr,
            state: UdpHostState::Listening,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        }
    }

    pub fn abort(&mut self, current_frame: usize) -> std::io::Result<()> {
        match self.state {
            UdpHostState::Connected | UdpHostState::Connecting(_) | UdpHostState::Syncing(_) => {
                self.send_msg(current_frame, MessageContent::Abort)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn update(&mut self, current_frame: usize) -> std::io::Result<Option<UdpStream>> {
        loop {
            let Some(new_state) = self.poll(current_frame)? else {
                return Ok(None);
            };
            match new_state {
                UdpHostState::Connected => {
                    return Ok(Some(self.establish_connection()?));
                }
                _ => self.state = new_state,
            }
        }
    }

    fn poll(&mut self, current_frame: usize) -> std::io::Result<Option<UdpHostState>> {
        match self.state {
            UdpHostState::Listening => self.listen(current_frame),
            UdpHostState::Syncing((local_offset, peer_offset)) => {
                self.wait_for_connection(current_frame, local_offset, peer_offset)
            }
            UdpHostState::Connecting(start_frame) => {
                self.wait_for_start(current_frame, start_frame)
            }
            _ => Ok(None),
        }
    }

    fn listen(&mut self, current_frame: usize) -> std::io::Result<Option<UdpHostState>> {
        while let Some(msg) = self.recv_msg() {
            if let MessageContent::Syn = msg.content {
                let peer_frame = msg.current_frame;
                self.send_msg(current_frame, MessageContent::SynAck)?;
                return Ok(Some(UdpHostState::Syncing((current_frame, peer_frame))));
            }
        }

        Ok(None)
    }

    fn wait_for_connection(
        &mut self,
        current_frame: usize,
        local_offset: usize,
        peer_offset: usize,
    ) -> std::io::Result<Option<UdpHostState>> {
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::Connect => {
                    let peer_start =
                        (current_frame - local_offset) + peer_offset + GAME_START_DELAY;
                    let start_timer = current_frame + GAME_START_DELAY;
                    self.send_msg(current_frame, MessageContent::StartAt(peer_start))?;
                    return Ok(Some(UdpHostState::Connecting(start_timer)));
                }
                MessageContent::Abort => return Ok(Some(UdpHostState::Listening)),
                _ => {}
            }
        }

        if current_frame > local_offset + PEER_TIME_OUT {
            Ok(Some(UdpHostState::Listening))
        } else {
            Ok(None)
        }
    }

    fn wait_for_start(
        &mut self,
        current_frame: usize,
        start_frame: usize,
    ) -> std::io::Result<Option<UdpHostState>> {
        while let Some(msg) = self.recv_msg() {
            if let MessageContent::Abort = msg.content {
                return Ok(Some(UdpHostState::Listening));
            }
        }

        if current_frame >= start_frame {
            Ok(Some(UdpHostState::Connected))
        } else {
            Ok(None)
        }
    }

    fn send_msg(
        &mut self,
        current_frame: usize,
        content: MessageContent,
    ) -> std::io::Result<usize> {
        send_msg(
            &self.socket,
            &mut self.send_buf,
            self.client_addr,
            current_frame,
            content,
        )
    }

    fn recv_msg(&mut self) -> Option<GameMessage<'_>> {
        recv_msg(&self.socket, &mut self.recv_buf, self.client_addr)
    }

    fn establish_connection(&mut self) -> std::io::Result<UdpStream> {
        if cfg!(feature = "debug") {
            println!("Connection established");
        }

        Ok(UdpStream::new(self.socket.try_clone()?, self.client_addr))
    }
}
