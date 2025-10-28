use std::{
    collections::VecDeque,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
};

use bincode::{BorrowDecode, Encode, config};

use crate::game::input::{ButtonFlag, Direction, InputHistory};

const GAME_VERSION: &[u8] = "0.1.0".as_bytes();
const BUFFER_LEN: usize = 1024;
const PEER_TIME_OUT: usize = 120;

#[derive(Debug, Encode, BorrowDecode)]
struct GameMessage<'a> {
    version: &'a [u8],
    current_frame: usize,
    content: MessageContent<'a>,
}

impl<'a> GameMessage<'a> {
    fn new(current_frame: usize, content: MessageContent<'a>) -> Self {
        Self {
            version: GAME_VERSION,
            current_frame,
            content,
        }
    }
}

#[derive(Debug, BorrowDecode, Encode)]
enum MessageContent<'a> {
    Syn,
    SynAck,
    Connect,
    Inputs((u32, &'a [u8])), // Start seq_num, (frame_num as u32, Direction, ButtonFlags) as bytes
    InputsAck(u32),
}

pub struct UdpListener {
    socket: UdpSocket,
    potential_peer: (usize, Option<SocketAddr>),
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
            potential_peer: (usize::MAX, None),
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        })
    }

    pub fn update(&mut self, current_frame: usize) -> std::io::Result<Option<UdpStream>> {
        let (mut peer_time_out, mut potential_peer) = self.potential_peer;
        if current_frame >= peer_time_out {
            peer_time_out = usize::MAX;
            potential_peer = None;
        }

        while let Some((msg, src_addr)) = self.recv_msg() {
            match msg.content {
                MessageContent::Syn => {
                    if potential_peer.is_none() {
                        peer_time_out = current_frame + PEER_TIME_OUT;
                        potential_peer = Some(src_addr);
                        self.send_msg(src_addr, current_frame, MessageContent::SynAck)?;
                    }
                }
                MessageContent::Connect => {
                    if potential_peer == Some(src_addr) {
                        let peer_frame = msg.current_frame;
                        self.send_msg(src_addr, current_frame, MessageContent::Connect)?;
                        return Ok(Some(self.establish_connection(
                            current_frame,
                            peer_frame,
                            src_addr,
                        )?));
                    }
                }
                _ => {}
            }
        }
        self.potential_peer = (peer_time_out, potential_peer);
        Ok(None)
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

    fn recv_msg(&mut self) -> Option<(GameMessage, SocketAddr)> {
        recv_msg(&self.socket, &mut self.recv_buf)
    }

    fn establish_connection(
        &mut self,
        local_frame: usize,
        peer_frame: usize,
        peer_addr: SocketAddr,
    ) -> std::io::Result<UdpStream> {
        let local_frame_offset = local_frame;
        let peer_frame_offset = peer_frame;

        if cfg!(feature = "debug") {
            println!("Connection established");
            println!("peer_frame_offset: {peer_frame_offset}");
        }

        Ok(UdpStream {
            socket: self.socket.try_clone()?,
            outbound_buf: VecDeque::new(),
            seq_num: 0,
            peer_seq_num: 0,
            peer_addr,
            local_frame_offset,
            peer_frame_offset,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        })
    }
}

pub struct UdpClient {
    socket: UdpSocket,
    target_addr: SocketAddr,
    recv_buf: [u8; BUFFER_LEN],
    send_buf: [u8; BUFFER_LEN],
}

impl UdpClient {
    pub fn bind<A>(local_addr: A, peer_addr: A) -> std::io::Result<Self>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind(local_addr)?;
        socket.connect(peer_addr)?;
        let target_addr = socket.peer_addr()?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            target_addr,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        })
    }

    pub fn update(&mut self, current_frame: usize) -> std::io::Result<Option<UdpStream>> {
        self.send_msg(current_frame, MessageContent::Syn)?;

        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::SynAck => {
                    self.send_msg(current_frame, MessageContent::Connect)?;
                }
                MessageContent::Connect => {
                    let peer_frame = msg.current_frame;
                    self.send_msg(current_frame, MessageContent::Connect)?;
                    return Ok(Some(self.establish_connection(current_frame, peer_frame)?));
                }
                _ => {}
            }
        }

        Ok(None)
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

    fn recv_msg(&mut self) -> Option<GameMessage> {
        let (msg, src_addr) = recv_msg(&self.socket, &mut self.recv_buf)?;

        if src_addr == self.target_addr {
            Some(msg)
        } else {
            None
        }
    }

    fn establish_connection(
        &mut self,
        local_frame: usize,
        peer_frame: usize,
    ) -> std::io::Result<UdpStream> {
        let local_frame_offset = local_frame;
        let peer_frame_offset = peer_frame;

        if cfg!(feature = "debug") {
            println!("Connection established");
            println!("peer_frame_offset: {peer_frame_offset}");
        }

        Ok(UdpStream {
            socket: self.socket.try_clone()?,
            outbound_buf: VecDeque::new(),
            seq_num: 0,
            peer_seq_num: 0,
            peer_addr: self.target_addr,
            local_frame_offset,
            peer_frame_offset,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        })
    }
}

pub struct UdpStream {
    socket: UdpSocket,
    outbound_buf: VecDeque<(u32, (Direction, ButtonFlag))>,
    seq_num: u32,
    peer_seq_num: u32,
    peer_addr: SocketAddr,
    local_frame_offset: usize,
    peer_frame_offset: usize,
    recv_buf: [u8; BUFFER_LEN],
    send_buf: [u8; BUFFER_LEN],
}

impl UdpStream {
    const INPUTS_CHUNK_SIZE: usize = size_of::<u32>() + size_of::<u8>() * 2;
    pub fn update(
        &mut self,
        current_frame: usize,
        host_inputs: &InputHistory,
        peer_inputs: &mut InputHistory,
    ) -> std::io::Result<(usize, usize)> {
        let mut rollback = 0;
        let mut fastforward = 0;
        let local_frame_offset = self.local_frame_offset;
        let peer_frame_offset = self.peer_frame_offset;

        let mut peer_seq_num = self.peer_seq_num;
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::Inputs((new_seq_start, raw_inputs)) => {
                    let (new_seq_num, frame_error) = Self::recv_inputs(
                        peer_seq_num,
                        current_frame,
                        local_frame_offset,
                        peer_frame_offset,
                        peer_inputs,
                        new_seq_start,
                        raw_inputs,
                    );
                    peer_seq_num = new_seq_num;

                    self.send_msg(current_frame, MessageContent::InputsAck(peer_seq_num))?;

                    if frame_error >= 0 {
                        rollback = rollback.max(frame_error as usize);
                    } else {
                        fastforward = fastforward.max(-frame_error as usize);
                    }
                }
                MessageContent::InputsAck(ack_seq_num) => {
                    let old_seq_num = self.seq_num;
                    self.seq_num = self.seq_num.max(ack_seq_num);
                    let deque_amt = self.seq_num - old_seq_num;
                    self.outbound_buf.drain(0..deque_amt as usize);

                    if cfg!(feature = "debug") {
                        println!("self.seq: {}", self.seq_num);
                    }
                }
                _ => {}
            }
        }
        self.peer_seq_num = peer_seq_num;

        // Send inputs if needed
        self.send_inputs(current_frame, host_inputs)?;

        Ok((rollback, fastforward))
    }

    fn send_inputs(
        &mut self,
        current_frame: usize,
        local_inputs: &InputHistory,
    ) -> std::io::Result<()> {
        if let Some(local_inputs) = local_inputs.get_inputs() {
            self.outbound_buf
                .push_front((current_frame as u32, local_inputs));
        }

        if !self.outbound_buf.is_empty() {
            let (inputs1, inputs2) = self.outbound_buf.as_slices();
            let mut input_iter =
                inputs1
                    .iter()
                    .chain(inputs2.iter())
                    .flat_map(|&(frame, (dir, buttons))| {
                        let fb = frame.to_ne_bytes();
                        let dir_raw: u8 = dir.into();
                        let button_bits = buttons.bits();
                        [fb[0], fb[1], fb[2], fb[3], dir_raw, button_bits]
                    });
            let input_raw: [u8; BUFFER_LEN] =
                std::array::from_fn(|_| input_iter.next().unwrap_or_default());
            let content = MessageContent::Inputs((
                self.seq_num,
                &input_raw[0..self.outbound_buf.len() * Self::INPUTS_CHUNK_SIZE],
            ));
            self.send_msg(current_frame, content)?;
        }
        Ok(())
    }

    fn recv_inputs(
        peer_seq_num: u32,
        current_frame: usize,
        local_frame_offset: usize,
        peer_frame_offset: usize,
        peer_inputs: &mut InputHistory,
        new_seq_start: u32,
        bytes: &[u8],
    ) -> (u32, isize) {
        let skip_inputs = peer_seq_num.saturating_sub(new_seq_start) as usize;
        let inputs_recv = (bytes.len() / Self::INPUTS_CHUNK_SIZE) as u32;

        if skip_inputs == inputs_recv as usize {
            return (peer_seq_num, 0);
        }

        if cfg!(feature = "debug") {
            println!(
                "Recieved {inputs_recv} new inputs, skipping: {skip_inputs}"
            );
        }

        for chunk in bytes
            .chunks_exact(Self::INPUTS_CHUNK_SIZE)
            .skip(skip_inputs)
        {
            let input_frame = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
            let game_frame = input_frame.saturating_sub(peer_frame_offset);
            let dir = Direction::from(chunk[4]);
            let buttons = ButtonFlag::from_bits_retain(chunk[5]);
            println!("recieved: {dir:?}, {buttons:?} at {game_frame}");
            peer_inputs.insert_input(
                (current_frame - local_frame_offset) - game_frame,
                dir,
                buttons,
            );
        }

        let next_seq_num = peer_seq_num.max(new_seq_start + inputs_recv);

        let offset = Self::INPUTS_CHUNK_SIZE * skip_inputs;
        let oldest_input = u32::from_ne_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);

        (
            next_seq_num,
            (current_frame - local_frame_offset) as isize
                - (oldest_input as usize - peer_frame_offset) as isize,
        )
    }

    fn send_msg(
        &mut self,
        current_frame: usize,
        content: MessageContent,
    ) -> std::io::Result<usize> {
        send_msg(
            &self.socket,
            &mut self.send_buf,
            self.peer_addr,
            current_frame,
            content,
        )
    }

    fn recv_msg(&mut self) -> Option<GameMessage> {
        let (msg, src_addr) = recv_msg(&self.socket, &mut self.recv_buf)?;

        if src_addr == self.peer_addr {
            Some(msg)
        } else {
            None
        }
    }
}

fn send_msg(
    socket: &UdpSocket,
    send_buf: &mut [u8],
    dst_addr: SocketAddr,
    current_frame: usize,
    content: MessageContent,
) -> std::io::Result<usize> {
    let msg = GameMessage::new(current_frame, content);
    let len = bincode::encode_into_slice(msg, send_buf, config::standard())
        .map_err(|_| std::io::ErrorKind::InvalidData)?;
    socket.send_to(&send_buf[0..len], dst_addr)
}

fn recv_msg<'a>(
    socket: &UdpSocket,
    recv_buf: &'a mut [u8],
) -> Option<(GameMessage<'a>, SocketAddr)> {
    if let Ok((packet_len, src_addr)) = socket.recv_from(recv_buf) {
        let (msg, _len): (GameMessage, usize) =
            bincode::borrow_decode_from_slice(&recv_buf[0..packet_len], config::standard()).ok()?;

        if msg.version == GAME_VERSION {
            Some((msg, src_addr))
        } else {
            None
        }
    } else {
        None
    }
}
