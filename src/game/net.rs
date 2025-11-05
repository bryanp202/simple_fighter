pub mod client;
pub mod listener;
pub mod matching;
pub mod stream;

use std::net::{SocketAddr, UdpSocket};

use bincode::{BorrowDecode, Encode, config};

use crate::game::{FRAME_RATE, GAME_VERSION};

const BUFFER_LEN: usize = 1024;
const PEER_TIME_OUT: usize = FRAME_RATE * 30;
const GAME_START_DELAY: usize = FRAME_RATE;

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
    HeartBeat,
    Inputs((u32, &'a [u8])), // Start seq_num, (frame_num as u32, Direction, ButtonFlags) as bytes
    InputsAck(u32),
    Abort,
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
