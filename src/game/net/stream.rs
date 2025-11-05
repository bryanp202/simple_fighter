use std::{
    collections::VecDeque,
    net::{SocketAddr, UdpSocket},
};

use crate::game::{
    input::{ButtonFlag, Direction, InputHistory},
    net::{BUFFER_LEN, GameMessage, MessageContent, recv_msg, send_msg},
};

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

    pub fn new(socket: UdpSocket, peer_addr: SocketAddr) -> Self {
        UdpStream {
            socket,
            outbound_buf: VecDeque::new(),
            seq_num: 0,
            peer_seq_num: 0,
            peer_addr,
            recv_buf: [0; BUFFER_LEN],
            send_buf: [0; BUFFER_LEN],
            aborted: false,
        }
    }

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
                        println!(
                            "self.seq: {} received at frame: {current_frame}",
                            self.seq_num
                        );
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
        recv_msg(&self.socket, &mut self.recv_buf, self.peer_addr)
    }
}
