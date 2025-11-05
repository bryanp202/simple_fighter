use std::net::{SocketAddr, UdpSocket};

use crate::game::net::{
    BUFFER_LEN, GameMessage, MessageContent, PEER_TIME_OUT, recv_msg, send_msg, stream::UdpStream,
};

enum UdpClientState {
    Syncing,
    Connecting(usize),
    WaitingToStart(usize),
    Connected,
}

pub struct UdpClient {
    socket: UdpSocket,
    target_addr: SocketAddr,
    state: UdpClientState,
    recv_buf: [u8; BUFFER_LEN],
    send_buf: [u8; BUFFER_LEN],
}

impl UdpClient {
    pub fn new(connection: UdpSocket, peer_addr: SocketAddr) -> Self {
        Self {
            socket: connection,
            target_addr: peer_addr,
            state: UdpClientState::Syncing,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        }
    }

    pub fn abort(&mut self, current_frame: usize) -> std::io::Result<()> {
        self.send_msg(current_frame, MessageContent::Abort)?;
        Ok(())
    }

    pub fn update(&mut self, current_frame: usize) -> std::io::Result<Option<UdpStream>> {
        loop {
            let Some(new_state) = self.poll(current_frame)? else {
                return Ok(None);
            };
            match new_state {
                UdpClientState::Connected => return Ok(Some(self.establish_connection()?)),
                _ => self.state = new_state,
            }
        }
    }

    fn poll(&mut self, current_frame: usize) -> std::io::Result<Option<UdpClientState>> {
        match self.state {
            UdpClientState::Syncing => self.sync(current_frame),
            UdpClientState::Connecting(time_out) => self.connect(current_frame, time_out),
            UdpClientState::WaitingToStart(start_time) => {
                self.wait_to_start(current_frame, start_time)
            }
            _ => Ok(None),
        }
    }

    fn sync(&mut self, current_frame: usize) -> std::io::Result<Option<UdpClientState>> {
        self.send_msg(current_frame, MessageContent::Syn)?;

        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::SynAck => {
                    self.send_msg(current_frame, MessageContent::Connect)?;
                    let time_out = current_frame + PEER_TIME_OUT;
                    return Ok(Some(UdpClientState::Connecting(time_out)));
                }
                _ => {}
            }
        }

        Ok(None)
    }

    fn connect(
        &mut self,
        current_frame: usize,
        time_out: usize,
    ) -> std::io::Result<Option<UdpClientState>> {
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::StartAt(start_timer) => {
                    return Ok(Some(UdpClientState::WaitingToStart(start_timer)));
                }
                MessageContent::Abort => return Ok(Some(UdpClientState::Syncing)),
                _ => {}
            }
        }

        if current_frame > time_out {
            Ok(Some(UdpClientState::Syncing))
        } else {
            Ok(None)
        }
    }

    fn wait_to_start(
        &mut self,
        current_frame: usize,
        start_frame: usize,
    ) -> std::io::Result<Option<UdpClientState>> {
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::Abort => return Ok(Some(UdpClientState::Syncing)),
                _ => {}
            }
        }

        if current_frame >= start_frame {
            Ok(Some(UdpClientState::Connected))
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
            self.target_addr,
            current_frame,
            content,
        )
    }

    fn recv_msg(&mut self) -> Option<GameMessage<'_>> {
        recv_msg(&self.socket, &mut self.recv_buf, self.target_addr)
    }

    fn establish_connection(&mut self) -> std::io::Result<UdpStream> {
        if cfg!(feature = "debug") {
            println!("Connection established");
        }

        Ok(UdpStream::new(self.socket.try_clone()?, self.target_addr))
    }
}
