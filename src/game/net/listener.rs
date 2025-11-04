use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use crate::game::net::{
    BUFFER_LEN, GAME_START_DELAY, GameMessage, MessageContent, PEER_TIME_OUT, recv_msg, send_msg,
    stream::UdpStream,
};

enum UdpListenerState {
    Listening,
    Syncing((usize, usize, SocketAddr)), // local frame offset, peer frame offset, peer addr
    Connecting((usize, SocketAddr)),
    Connected(SocketAddr),
}

pub struct UdpListener {
    socket: UdpSocket,
    state: UdpListenerState,
    recv_buf: [u8; BUFFER_LEN],
    send_buf: [u8; BUFFER_LEN],
}

impl UdpListener {
    pub fn bind<A>(addr: A) -> std::io::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            state: UdpListenerState::Listening,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        })
    }

    pub fn abort(&mut self, current_frame: usize) -> std::io::Result<()> {
        match self.state {
            UdpListenerState::Connected(peer_addr)
            | UdpListenerState::Connecting((_, peer_addr))
            | UdpListenerState::Syncing((_, _, peer_addr)) => {
                self.send_msg(peer_addr, current_frame, MessageContent::Abort)?;
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
                UdpListenerState::Connected(peer_addr) => {
                    return Ok(Some(self.establish_connection(peer_addr)?));
                }
                _ => self.state = new_state,
            }
        }
    }

    fn poll(&mut self, current_frame: usize) -> std::io::Result<Option<UdpListenerState>> {
        match self.state {
            UdpListenerState::Listening => self.listen(current_frame),
            UdpListenerState::Syncing(syncing_state) => {
                self.wait_for_connection(current_frame, syncing_state)
            }
            UdpListenerState::Connecting((start_frame, peer_addr)) => {
                self.wait_for_start(current_frame, start_frame, peer_addr)
            }
            _ => Ok(None),
        }
    }

    fn listen(&mut self, current_frame: usize) -> std::io::Result<Option<UdpListenerState>> {
        while let Some((msg, src_addr)) = self.recv_msg() {
            match msg.content {
                MessageContent::Syn => {
                    let peer_frame = msg.current_frame;
                    self.send_msg(src_addr, current_frame, MessageContent::SynAck)?;
                    return Ok(Some(UdpListenerState::Syncing((
                        current_frame,
                        peer_frame,
                        src_addr,
                    ))));
                }
                _ => {}
            }
        }

        Ok(None)
    }

    fn wait_for_connection(
        &mut self,
        current_frame: usize,
        syncing_state: (usize, usize, SocketAddr),
    ) -> std::io::Result<Option<UdpListenerState>> {
        let (local_offset, peer_offset, peer_addr) = syncing_state;

        while let Some(msg) = self.recv_msg_from(peer_addr) {
            match msg.content {
                MessageContent::Connect => {
                    let peer_start =
                        (current_frame - local_offset) + peer_offset + GAME_START_DELAY;
                    let start_timer = current_frame + GAME_START_DELAY;
                    self.send_msg(
                        peer_addr,
                        current_frame,
                        MessageContent::StartAt(peer_start),
                    )?;
                    return Ok(Some(UdpListenerState::Connecting((start_timer, peer_addr))));
                }
                MessageContent::Abort => return Ok(Some(UdpListenerState::Listening)),
                _ => {}
            }
        }

        if current_frame > local_offset + PEER_TIME_OUT {
            Ok(Some(UdpListenerState::Listening))
        } else {
            Ok(None)
        }
    }

    fn wait_for_start(
        &mut self,
        current_frame: usize,
        start_frame: usize,
        peer_addr: SocketAddr,
    ) -> std::io::Result<Option<UdpListenerState>> {
        while let Some(msg) = self.recv_msg_from(peer_addr) {
            match msg.content {
                MessageContent::Abort => return Ok(Some(UdpListenerState::Listening)),
                _ => {}
            }
        }

        if current_frame >= start_frame {
            Ok(Some(UdpListenerState::Connected(peer_addr)))
        } else {
            Ok(None)
        }
    }

    fn send_msg(
        &mut self,
        dst_addr: SocketAddr,
        current_frame: usize,
        content: MessageContent,
    ) -> std::io::Result<usize> {
        send_msg(
            &self.socket,
            &mut self.send_buf,
            dst_addr,
            current_frame,
            content,
        )
    }

    fn recv_msg(&mut self) -> Option<(GameMessage<'_>, SocketAddr)> {
        recv_msg(&self.socket, &mut self.recv_buf)
    }

    fn recv_msg_from(&mut self, addr: SocketAddr) -> Option<GameMessage<'_>> {
        let (msg, src_addr) = recv_msg(&self.socket, &mut self.recv_buf)?;
        if addr == src_addr { Some(msg) } else { None }
    }

    fn establish_connection(&mut self, peer_addr: SocketAddr) -> std::io::Result<UdpStream> {
        if cfg!(feature = "debug") {
            println!("Connection established");
        }

        Ok(UdpStream::new(self.socket.try_clone()?, peer_addr))
    }
}
