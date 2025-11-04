use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use bincode::{BorrowDecode, config};

use crate::game::{
    GAME_VERSION,
    net::{
        BUFFER_LEN, GameMessage, MessageContent, PEER_TIME_OUT, client::UdpClient,
        listener::UdpListener, recv_msg, send_msg,
    },
};

pub enum PeerConnectionType {
    Hosting(UdpListener),
    Joining(UdpClient),
}

#[derive(BorrowDecode, Debug)]
struct MatchDataJson<'a> {
    local_is_host: bool,
    _local: &'a str,
    peer: &'a str,
}

enum MatchingState {
    RequestPeer,
    WaitForPeer(usize),
    HolePunching((bool, usize, SocketAddr)),
    Hosting(SocketAddr),
    Joining(SocketAddr),
}

pub struct MatchingSocket {
    socket: UdpSocket,
    server_addr: SocketAddr,
    state: MatchingState,
    recv_buf: [u8; BUFFER_LEN],
    send_buf: [u8; BUFFER_LEN],
}

impl MatchingSocket {
    pub fn bind<A>(local_addr: A, server_addr: A) -> std::io::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(local_addr)?;
        let server_addr = server_addr
            .to_socket_addrs()?
            .next()
            .ok_or(std::io::ErrorKind::InvalidData)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            server_addr,
            state: MatchingState::RequestPeer,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        })
    }

    fn host(&mut self, client_addr: SocketAddr) -> std::io::Result<PeerConnectionType> {
        Ok(PeerConnectionType::Hosting(UdpListener::new(
            self.socket.try_clone()?,
            client_addr,
        )))
    }

    fn join(&mut self, host_addr: SocketAddr) -> std::io::Result<PeerConnectionType> {
        Ok(PeerConnectionType::Joining(UdpClient::new(
            self.socket.try_clone()?,
            host_addr,
        )))
    }

    pub fn update(&mut self, current_frame: usize) -> std::io::Result<Option<PeerConnectionType>> {
        loop {
            let Some(new_state) = self.poll(current_frame)? else {
                return Ok(None);
            };
            match new_state {
                MatchingState::Hosting(client_addr) => return Ok(Some(self.host(client_addr)?)),
                MatchingState::Joining(host_addr) => return Ok(Some(self.join(host_addr)?)),
                _ => self.state = new_state,
            }
        }
    }

    fn poll(&mut self, current_frame: usize) -> std::io::Result<Option<MatchingState>> {
        match self.state {
            MatchingState::RequestPeer => self.request_peer(current_frame),
            MatchingState::WaitForPeer(time_out) => self.wait_for_peer(current_frame, time_out),
            MatchingState::HolePunching((is_host, time_out, peer_addr)) => {
                self.hole_punch(is_host, peer_addr, current_frame, time_out)
            }
            _ => Ok(None),
        }
    }

    fn request_peer(&mut self, current_frame: usize) -> std::io::Result<Option<MatchingState>> {
        self.socket
            .send_to(GAME_VERSION, self.server_addr)
            .expect("Failed to contact matchmaking server");

        let time_out = current_frame + PEER_TIME_OUT;

        Ok(Some(MatchingState::WaitForPeer(time_out)))
    }

    fn wait_for_peer(
        &mut self,
        current_frame: usize,
        time_out: usize,
    ) -> std::io::Result<Option<MatchingState>> {
        while let Ok(len) = self.socket.recv(&mut self.recv_buf) {
            let Ok((matchdata, _)): Result<(MatchDataJson, usize), _> =
                bincode::borrow_decode_from_slice(&self.recv_buf[..len], config::standard())
            else {
                continue;
            };

            let time_out = current_frame + PEER_TIME_OUT;
            let peer_addr = matchdata
                .peer
                .to_socket_addrs()?
                .next()
                .ok_or(std::io::ErrorKind::InvalidData)?;

            return Ok(Some(MatchingState::HolePunching((
                matchdata.local_is_host,
                time_out,
                peer_addr,
            ))));
        }

        if current_frame > time_out {
            Ok(Some(MatchingState::RequestPeer))
        } else {
            Ok(None)
        }
    }

    fn hole_punch(
        &mut self,
        is_host: bool,
        peer_addr: SocketAddr,
        current_frame: usize,
        time_out: usize,
    ) -> std::io::Result<Option<MatchingState>> {
        self.send_game_msg(peer_addr, current_frame, MessageContent::HeartBeat)?;

        while let Some(msg) = self.recv_game_msg(peer_addr) {
            match msg.content {
                MessageContent::HeartBeat => {
                    return if is_host {
                        Ok(Some(MatchingState::Hosting(peer_addr)))
                    } else {
                        Ok(Some(MatchingState::Joining(peer_addr)))
                    };
                }
                _ => {}
            }
        }

        if current_frame > time_out {
            Ok(Some(MatchingState::RequestPeer))
        } else {
            Ok(None)
        }
    }

    fn send_game_msg(
        &mut self,
        peer_addr: SocketAddr,
        current_frame: usize,
        content: MessageContent,
    ) -> std::io::Result<usize> {
        send_msg(
            &self.socket,
            &mut self.send_buf,
            peer_addr,
            current_frame,
            content,
        )
    }

    fn recv_game_msg(&mut self, peer_addr: SocketAddr) -> Option<GameMessage<'_>> {
        let (msg, src_addr) = recv_msg(&self.socket, &mut self.recv_buf)?;

        if src_addr == peer_addr {
            Some(msg)
        } else {
            None
        }
    }
}
