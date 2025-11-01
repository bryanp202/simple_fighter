use std::{
    collections::VecDeque,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
};

use bincode::{BorrowDecode, Encode, config};

use crate::game::input::{ButtonFlag, Direction, InputHistory};

const GAME_VERSION: &[u8] = "0.1.0".as_bytes();
const BUFFER_LEN: usize = 1024;
const PEER_TIME_OUT: usize = 120;
const GAME_START_DELAY: usize = 60;

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
    StartAt(usize),
    Inputs((u32, &'a [u8])), // Start seq_num, (frame_num as u32, Direction, ButtonFlags) as bytes
    InputsAck(u32),
    Abort,
}

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
            _ => {},
        }
        Ok(())
    }

    pub fn update(&mut self, current_frame: usize) -> std::io::Result<Option<UdpStream>> {
        loop {
            let Some(new_state) = self.poll(current_frame)? else {
                return Ok(None);
            };
            match new_state {
                UdpListenerState::Connected(peer_addr) => return Ok(Some(self.establish_connection(peer_addr)?)),
                _ => self.state = new_state,
            }
        }
    }

    fn poll(&mut self, current_frame: usize) -> std::io::Result<Option<UdpListenerState>> {
        match self.state {
            UdpListenerState::Listening => self.listen(current_frame),
            UdpListenerState::Syncing(syncing_state) => self.wait_for_connection(current_frame, syncing_state),
            UdpListenerState::Connecting((start_frame, peer_addr)) => self.wait_for_start(current_frame, start_frame, peer_addr),
            _ => Ok(None),
        }
    }

    fn listen(&mut self, current_frame: usize) -> std::io::Result<Option<UdpListenerState>> {
        while let Some((msg, src_addr)) = self.recv_msg() {
            match msg.content {
                MessageContent::Syn => {
                    let peer_frame = msg.current_frame;
                    self.send_msg(src_addr, current_frame, MessageContent::SynAck)?;
                    return Ok(Some(UdpListenerState::Syncing((current_frame, peer_frame, src_addr))));
                }
                _ => {}
            }
        }

        Ok(None)
    }

    fn wait_for_connection(&mut self, current_frame: usize, syncing_state: (usize, usize, SocketAddr)) -> std::io::Result<Option<UdpListenerState>> {
        let (local_offset, peer_offset, peer_addr) = syncing_state;

        while let Some(msg) = self.recv_msg_from(peer_addr) {
            match msg.content {
                MessageContent::Connect => {
                    let peer_start = (current_frame - local_offset) + peer_offset + GAME_START_DELAY;
                    let start_timer = current_frame + GAME_START_DELAY;
                    self.send_msg(
                        peer_addr,
                        current_frame,
                        MessageContent::StartAt(peer_start),
                    )?;
                    return Ok(Some(UdpListenerState::Connecting((start_timer, peer_addr))));
                },
                MessageContent::Abort => return Ok(Some(UdpListenerState::Listening)),
                _ => {},
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
                _ => {},
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
        if addr == src_addr {
            Some(msg)
        } else {
            None
        }
    }

    fn establish_connection(&mut self, peer_addr: SocketAddr) -> std::io::Result<UdpStream> {
        if cfg!(feature = "debug") {
            println!("Connection established");
        }

        Ok(UdpStream {
            socket: self.socket.try_clone()?,
            outbound_buf: VecDeque::new(),
            seq_num: 0,
            peer_seq_num: 0,
            peer_addr,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
            aborted: false,
        })
    }
}

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
            state: UdpClientState::Syncing,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
        })
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
            UdpClientState::WaitingToStart(start_time) => self.wait_to_start(current_frame, start_time),
            _ => Ok(None),
        }
    }

    fn sync(&mut self, current_frame: usize) -> std::io::Result<Option<UdpClientState>> {
        self.send_msg(current_frame, MessageContent::Syn)?;
        
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::SynAck => {
                    self.send_msg(current_frame, MessageContent::Connect)?;
                    return Ok(Some(UdpClientState::Connecting(current_frame)));
                }
                _ => {},
            }
        }

        Ok(None)
    }

    fn connect(&mut self, current_frame: usize, time_out: usize) -> std::io::Result<Option<UdpClientState>> {
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::StartAt(start_timer) => return Ok(Some(UdpClientState::WaitingToStart(start_timer))),
                MessageContent::Abort => return Ok(Some(UdpClientState::Syncing)),
                _ => {},
            }
        }

        if current_frame > time_out + PEER_TIME_OUT {
            Ok(Some(UdpClientState::Syncing))
        } else {
            Ok(None)
        }
    }

    fn wait_to_start(&mut self, current_frame: usize, start_frame: usize) -> std::io::Result<Option<UdpClientState>> {
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::Abort => return Ok(Some(UdpClientState::Syncing)),
                _ => {},
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
        let (msg, src_addr) = recv_msg(&self.socket, &mut self.recv_buf)?;

        if src_addr == self.target_addr {
            Some(msg)
        } else {
            None
        }
    }

    fn establish_connection(&mut self) -> std::io::Result<UdpStream> {
        if cfg!(feature = "debug") {
            println!("Connection established");
        }

        Ok(UdpStream {
            socket: self.socket.try_clone()?,
            outbound_buf: VecDeque::new(),
            seq_num: 0,
            peer_seq_num: 0,
            peer_addr: self.target_addr,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
            aborted: false,
        })
    }
}

pub struct UdpStream {
    socket: UdpSocket,
    outbound_buf: VecDeque<(u32, (Direction, ButtonFlag))>,
    seq_num: u32,
    peer_seq_num: u32,
    peer_addr: SocketAddr,
    recv_buf: [u8; BUFFER_LEN],
    send_buf: [u8; BUFFER_LEN],
    aborted: bool,
}

impl UdpStream {
    const INPUTS_CHUNK_SIZE: usize = size_of::<u32>() + size_of::<u8>() * 2;

    pub fn abort(&mut self, current_frame: usize) -> std::io::Result<()> {
        self.send_msg(current_frame, MessageContent::Abort)?;
        self.aborted = true;
        Ok(())
    }

    pub fn is_aborted(&self) -> bool {
        self.aborted
    }

    pub fn update(
        &mut self,
        current_frame: usize,
        host_inputs: &InputHistory,
        peer_inputs: &mut InputHistory,
    ) -> std::io::Result<(usize, usize)> {
        let mut rollback = 0;
        let mut fastforward = 0;

        let mut peer_seq_num = self.peer_seq_num;
        while let Some(msg) = self.recv_msg() {
            match msg.content {
                MessageContent::Abort => self.aborted = true,
                MessageContent::Inputs((new_seq_start, raw_inputs)) => {
                    let (new_seq_num, new_rollback, new_fastforward) = Self::recv_inputs(
                        peer_seq_num,
                        current_frame,
                        peer_inputs,
                        new_seq_start,
                        raw_inputs,
                    );
                    peer_seq_num = new_seq_num;

                    self.send_msg(current_frame, MessageContent::InputsAck(peer_seq_num))?;

                    rollback = rollback.max(new_rollback);
                    fastforward = fastforward.max(new_fastforward);
                }
                MessageContent::InputsAck(ack_seq_num) => {
                    let old_seq_num = self.seq_num;
                    self.seq_num = self.seq_num.max(ack_seq_num);
                    let deque_amt = self.seq_num - old_seq_num;
                    self.outbound_buf.drain(0..deque_amt as usize);

                    if cfg!(feature = "debug") {
                        println!("self.seq: {} received at frame: {current_frame}", self.seq_num);
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
                    .rev()
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
        mut current_frame: usize,
        peer_inputs: &mut InputHistory,
        new_seq_start: u32,
        bytes: &[u8],
    ) -> (u32, usize, usize) {
        let skip_inputs = peer_seq_num.saturating_sub(new_seq_start) as usize;
        let inputs_recv = (bytes.len() / Self::INPUTS_CHUNK_SIZE) as u32;

        if cfg!(feature = "debug") {
            println!("Recieved {inputs_recv} new inputs, skipping: {skip_inputs}");
        }

        if skip_inputs == inputs_recv as usize {
            return (peer_seq_num, 0, 0);
        }

        let frame_at_start = current_frame;

        for chunk in bytes
            .chunks_exact(Self::INPUTS_CHUNK_SIZE)
            .skip(skip_inputs)
        {
            let input_frame = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
            let dir = Direction::from(chunk[4]);
            let buttons = ButtonFlag::from_bits_retain(chunk[5]);

            if cfg!(feature = "debug") {
                println!(
                    "recieved: {dir:?}, {buttons:?} for frame: {input_frame} at local frame: {frame_at_start}"
                );
            }

            let relative_frame = current_frame as isize - input_frame as isize;

            peer_inputs.append_input(relative_frame, dir, buttons);

            if relative_frame < 0 {
                current_frame = current_frame + (-relative_frame) as usize;
            }
        }

        let next_seq_num = peer_seq_num.max(new_seq_start + inputs_recv);

        let offset = Self::INPUTS_CHUNK_SIZE * skip_inputs;
        let oldest_input = u32::from_ne_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;

        (
            next_seq_num,
            frame_at_start.saturating_sub(oldest_input),
            current_frame - frame_at_start,
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

    fn recv_msg(&mut self) -> Option<GameMessage<'_>> {
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
