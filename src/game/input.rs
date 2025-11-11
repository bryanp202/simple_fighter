use bincode::{BorrowDecode, Encode};
use bitflags::bitflags;
use sdl3::keyboard::Keycode;

use crate::game::MAX_ROLLBACK_FRAMES;

const DIRECTION_COUNT: usize = 4;
const BUTTON_COUNT: usize = 3;
const INPUT_VARIANTS: usize = 1;
const MOTION_BUF_SIZE: usize = 4;

const HISTORY_FRAME_LEN: usize = MAX_ROLLBACK_FRAMES + 64;
const HISTORY_PARSE_FRAMES: usize = 32;
const DASH_HISTORY_LEN: usize = HISTORY_PARSE_FRAMES / 2;

// Most Valuable
const DP_RIGHT_INVERSE: &[Direction] = &[
    Direction::DownRight,
    Direction::Down,
    Direction::Neutral,
    Direction::Right,
];
const DP_LEFT_INVERSE: &[Direction] = &[
    Direction::DownLeft,
    Direction::Down,
    Direction::Neutral,
    Direction::Left,
];
// Second Most valuable
const QC_RIGHT_INVERSE: &[Direction] = &[Direction::Right, Direction::DownRight, Direction::Down];
const QC_LEFT_INVERSE: &[Direction] = &[Direction::Left, Direction::DownLeft, Direction::Down];
// Least Valuable Motion Input
const RIGHT_RIGHT_INVERSE: &[Direction] = &[Direction::Right, Direction::Neutral, Direction::Right];
const LEFT_LEFT_INVERSE: &[Direction] = &[Direction::Left, Direction::Neutral, Direction::Left];
// Second Least Valuable Motion Input
const DOWN_DOWN_INVERSE: &[Direction] = &[Direction::Down, Direction::Neutral, Direction::Down];

pub const PLAYER1_BUTTONS: KeyToButtons = [
    (Keycode::G, ButtonFlag::L),
    (Keycode::H, ButtonFlag::M),
    (Keycode::J, ButtonFlag::H),
];
pub const PLAYER1_DIRECTIONS: KeyToDirections = [
    (Keycode::W, DirectionFlag::Up),
    (Keycode::S, DirectionFlag::Down),
    (Keycode::A, DirectionFlag::Left),
    (Keycode::D, DirectionFlag::Right),
];
pub const PLAYER2_BUTTONS: KeyToButtons = [
    (Keycode::Kp1, ButtonFlag::L),
    (Keycode::Kp2, ButtonFlag::M),
    (Keycode::Kp3, ButtonFlag::H),
];
pub const PLAYER2_DIRECTIONS: KeyToDirections = [
    (Keycode::Up, DirectionFlag::Up),
    (Keycode::Down, DirectionFlag::Down),
    (Keycode::Left, DirectionFlag::Left),
    (Keycode::Right, DirectionFlag::Right),
];

type MoveBuffer = [(Motion, ButtonFlag); MOTION_BUF_SIZE];

// Returns an input history and state component for a players input
pub fn new_inputs(
    key_to_button: KeyToButtons,
    key_to_direction: KeyToDirections,
) -> (InputHistory, Inputs) {
    let inputs = Inputs::new();
    let input_history = InputHistory::new(key_to_button, key_to_direction, 0);
    (input_history, inputs)
}

#[derive(Clone, PartialEq, Debug)]
pub struct Inputs {
    dir: Direction,
    buttons: ButtonFlag,
    buf: MoveBuffer,
}

impl Inputs {
    fn new() -> Self {
        Self {
            dir: Direction::Neutral,
            buttons: ButtonFlag::NONE,
            buf: std::array::from_fn(|_| (Motion::NONE, ButtonFlag::NONE)),
        }
    }

    pub fn reset(&mut self) {
        *self = Inputs::new();
    }

    pub fn active_buttons(&self) -> ButtonFlag {
        self.buttons
    }

    pub fn dir(&self) -> Direction {
        self.dir
    }

    pub fn move_buf(&self) -> MoveBuffer {
        self.buf
    }

    pub fn update(
        &mut self,
        held_buttons: ButtonFlag,
        parsed_input: (Direction, Motion, ButtonFlag),
    ) {
        let mut new_buf: MoveBuffer = std::array::from_fn(|_| (Motion::NONE, ButtonFlag::NONE));
        new_buf[1..].copy_from_slice(&self.buf[0..MOTION_BUF_SIZE - 1]);

        let (dir, motion, buttons) = parsed_input;
        new_buf[0] = (motion, buttons);
        self.buf = new_buf;
        self.dir = dir;
        self.buttons = held_buttons;
    }
}

type KeyToButtons = [(Keycode, ButtonFlag); BUTTON_COUNT * INPUT_VARIANTS];
type KeyToDirections = [(Keycode, DirectionFlag); DIRECTION_COUNT * INPUT_VARIANTS];
struct InputState {
    active_dir: DirectionFlag,
    release_next_dir: DirectionFlag,

    active_buttons: ButtonFlag,
    release_next_buttons: ButtonFlag,

    key_to_button: KeyToButtons,
    key_to_direction: KeyToDirections,
}

impl InputState {
    pub fn new(key_to_button: KeyToButtons, key_to_direction: KeyToDirections) -> Self {
        Self {
            active_buttons: ButtonFlag::NONE,
            active_dir: DirectionFlag::Neutral,
            release_next_buttons: ButtonFlag::NONE,
            release_next_dir: DirectionFlag::Neutral,
            key_to_button,
            key_to_direction,
        }
    }

    fn handle_keypress(&mut self, keycode: Keycode) {
        let pairing = self.key_to_button.iter().find_map(|pair| {
            if pair.0 == keycode {
                Some(pair.1)
            } else {
                None
            }
        });

        if let Some(pressed_button) = pairing {
            self.active_buttons |= pressed_button;
            self.release_next_buttons &= !pressed_button;
        } else {
            let dir_pairing = self.key_to_direction.iter().find_map(|pair| {
                if pair.0 == keycode {
                    Some(pair.1)
                } else {
                    None
                }
            });

            if let Some(pressed_direction) = dir_pairing {
                self.active_dir |= pressed_direction;
                self.release_next_dir &= !pressed_direction;
            }
        }
    }

    fn handle_keyrelease(&mut self, keycode: Keycode) {
        let pairing = self.key_to_button.iter().find_map(|pair| {
            if pair.0 == keycode {
                Some(pair.1)
            } else {
                None
            }
        });

        if let Some(pressed_button) = pairing {
            self.release_next_buttons |= pressed_button;
        } else {
            let dir_pairing = self.key_to_direction.iter().find_map(|pair| {
                if pair.0 == keycode {
                    Some(pair.1)
                } else {
                    None
                }
            });

            if let Some(pressed_direction) = dir_pairing {
                self.release_next_dir |= pressed_direction;
            }
        }
    }

    fn update(&mut self) -> (Direction, ButtonFlag) {
        let dir = match self.active_dir {
            DirectionFlag::Right | DirectionFlag::_RightAlt => Direction::Right,
            DirectionFlag::Left | DirectionFlag::_LeftAlt => Direction::Left,
            DirectionFlag::Up | DirectionFlag::_UpAlt => Direction::Up,
            DirectionFlag::Down | DirectionFlag::_DownAlt => Direction::Down,
            DirectionFlag::UpLeft => Direction::UpLeft,
            DirectionFlag::UpRight => Direction::UpRight,
            DirectionFlag::DownRight => Direction::DownRight,
            DirectionFlag::DownLeft => Direction::DownLeft,
            _ => Direction::Neutral,
        };
        let buttons = self.active_buttons;

        self.active_buttons ^= self.release_next_buttons;
        self.release_next_buttons = ButtonFlag::NONE;
        self.active_dir ^= self.release_next_dir;
        self.release_next_dir = DirectionFlag::Neutral;

        (dir, buttons)
    }
}

pub struct InputHistory {
    input: InputState,
    buf: [(Direction, ButtonFlag, usize); HISTORY_FRAME_LEN],
    current_index: usize,
    delay: usize,
}

impl InputHistory {
    fn new(key_to_button: KeyToButtons, key_to_direction: KeyToDirections, delay: usize) -> Self {
        Self {
            input: InputState::new(key_to_button, key_to_direction),
            buf: std::array::from_fn(|_| (Direction::Neutral, ButtonFlag::NONE, 1)),
            current_index: 0,
            delay,
        }
    }

    pub fn held_buttons(&self) -> ButtonFlag {
        self.input.active_buttons
    }

    pub fn reset(&mut self) {
        self.buf = std::array::from_fn(|_| (Direction::Neutral, ButtonFlag::NONE, 1));
        self.current_index = 0;
    }

    pub fn set_mappings(&mut self, key_to_button: KeyToButtons, key_to_direction: KeyToDirections) {
        self.input.key_to_button = key_to_button;
        self.input.key_to_direction = key_to_direction;
    }

    pub fn set_delay(&mut self, delay: usize) {
        self.delay = delay;
    }

    pub fn get_inputs(&self) -> Option<(Direction, ButtonFlag)> {
        let (dir, buttons, frames) = &self.buf[self.current_index];
        if *frames == 1 {
            Some((*dir, *buttons))
        } else {
            None
        }
    }

    pub fn handle_keypress(&mut self, keycode: Keycode) {
        self.input.handle_keypress(keycode);
    }

    pub fn handle_keyrelease(&mut self, keycode: Keycode) {
        self.input.handle_keyrelease(keycode);
    }

    pub fn skip(&mut self) {
        // Increment running frame length
        self.buf[self.current_index].2 += 1;
    }

    pub fn skip_for(&mut self, frames: usize) {
        // Increment running frame length
        self.buf[self.current_index].2 += frames;
    }

    pub fn update(&mut self) {
        let (input_dir, input_buttons) = self.input.update();

        let (dir, buttons, frames) = &mut self.buf[self.current_index];
        if *dir == input_dir && *buttons == input_buttons {
            *frames += 1;
        } else {
            self.current_index = (self.current_index + 1) % HISTORY_FRAME_LEN;
            self.buf[self.current_index] = (input_dir, input_buttons, 1);
        }
    }

    /// Add a new input that may or may not split the current run
    ///
    /// Should never be called out of order
    ///
    /// Ex: [(Neutral, Buttons::L, 200 frames)]: Insert 20 frames back ->
    /// [(Neutral, Buttons::L, 180 frames), (new_dir, new_buttons, 20)]
    pub fn append_input(
        &mut self,
        frames_back: isize,
        input_dir: Direction,
        input_buttons: ButtonFlag,
    ) -> bool {
        let (dir, buttons, frames) = &mut self.buf[self.current_index];

        if frames_back < 0 {
            // Increment running frame length to simulate fastforward
            if *dir == input_dir && *buttons == input_buttons {
                self.buf[self.current_index].2 += (-frames_back) as usize;
                return false;
            }

            self.buf[self.current_index].2 += (-frames_back - 1) as usize;
            self.current_index = (self.current_index + 1) % HISTORY_FRAME_LEN;
            self.buf[self.current_index] = (input_dir, input_buttons, 1);
            return true;
        }

        let new_run_length = frames_back as usize + 1;

        if *dir == input_dir && *buttons == input_buttons {
            return false;
        }

        if *frames != new_run_length {
            *frames -= new_run_length;
            self.current_index = (self.current_index + 1) % HISTORY_FRAME_LEN;
        }
        self.buf[self.current_index] = (input_dir, input_buttons, new_run_length);

        true
    }

    /// Returns the index that the run (index spaces back) is and how much overlap there is
    fn get_index_and_overlap(&self, mut frame: usize) -> (usize, usize) {
        let mut current_index = self.current_index;
        frame += 1;

        loop {
            let (_, _, frames) = &self.buf[current_index];
            if frame <= *frames {
                return (current_index, frame);
            }
            frame -= frames;
            current_index = (HISTORY_FRAME_LEN + current_index - 1) % HISTORY_FRAME_LEN;
        }
    }

    pub fn parse_history(&self) -> (Direction, Motion, ButtonFlag) {
        self.parse_history_at(0)
    }

    /// Expects delay to be <= `HISTORY_FRAME_LEN` + `PARSE_LEN`
    pub fn parse_history_at(&self, rollback: usize) -> (Direction, Motion, ButtonFlag) {
        let mut result = Motion::NONE;

        let target_frame = self.delay + rollback;
        let (overlap_index, overlap) = self.get_index_and_overlap(target_frame);

        let just_pressed_buttons = self.get_buttons_pressed(overlap_index, overlap);

        let mut ordered_frames = [Direction::Neutral; HISTORY_FRAME_LEN];
        let (motion_end, dash_end) = self.order_frames(&mut ordered_frames, overlap_index, overlap);
        let motion_slice = &ordered_frames[0..motion_end];
        let dash_slice = &ordered_frames[0..dash_end];

        result |= Self::find_dir_sequence(motion_slice, DP_RIGHT_INVERSE, Motion::DpRight);
        result |= Self::find_dir_sequence(motion_slice, DP_LEFT_INVERSE, Motion::DpLeft);

        result |= Self::find_dir_sequence(motion_slice, QC_RIGHT_INVERSE, Motion::QcRight);
        result |= Self::find_dir_sequence(motion_slice, QC_LEFT_INVERSE, Motion::QcLeft);

        result |= Self::find_dir_sequence(dash_slice, RIGHT_RIGHT_INVERSE, Motion::RightRight);
        result |= Self::find_dir_sequence(dash_slice, LEFT_LEFT_INVERSE, Motion::LeftLeft);

        result |= Self::find_dir_sequence(motion_slice, DOWN_DOWN_INVERSE, Motion::DownDown);

        let dir = ordered_frames[0];
        (dir, result, just_pressed_buttons)
    }

    fn order_frames(
        &self,
        buf: &mut [Direction; HISTORY_FRAME_LEN],
        overlap_index: usize,
        overlap: usize,
    ) -> (usize, usize) {
        let mut dash_buffer_end = None;
        let mut frame_count = self.buf[overlap_index].2 - overlap;
        buf[0] = self.buf[overlap_index].0;
        let mut write_i = 1;
        let mut read_i = 1;

        while frame_count < HISTORY_PARSE_FRAMES {
            if dash_buffer_end.is_none() && frame_count >= DASH_HISTORY_LEN {
                dash_buffer_end = Some(write_i);
            }
            let current_index = (HISTORY_FRAME_LEN + overlap_index - read_i) % HISTORY_FRAME_LEN;
            let (dir, _, frames) = &self.buf[current_index];
            if buf[write_i - 1] != *dir {
                buf[write_i] = *dir;
                write_i += 1;
            }
            read_i += 1;
            frame_count += *frames;
        }

        (write_i, dash_buffer_end.unwrap_or(write_i))
    }

    fn get_buttons_pressed(&self, overlap_index: usize, overlap: usize) -> ButtonFlag {
        if self.buf[overlap_index].2 == overlap {
            let index_before = (HISTORY_FRAME_LEN + overlap_index - 1) % HISTORY_FRAME_LEN;
            (self.buf[index_before].1 ^ self.buf[overlap_index].1) & !self.buf[index_before].1
        } else {
            ButtonFlag::NONE
        }
    }

    fn find_dir_sequence(haystack: &[Direction], seq: &[Direction], motion: Motion) -> Motion {
        if haystack.windows(seq.len()).any(|window| window == seq) {
            motion
        } else {
            Motion::NONE
        }
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct ButtonFlag: u8 {
        const NONE = 0;
        const L = 0b0000_0001;
        const M = 0b0000_0010;
        const H = 0b0000_0100;
    }
}

const UP_DIR: u32 = 0b0001;
const DOWN_DIR: u32 = 0b0010;
const LEFT_DIR: u32 = 0b0100;
const RIGHT_DIR: u32 = 0b1000;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct DirectionFlag: u32 {
        const Neutral = 0;
        const Up = UP_DIR;
        const Down = DOWN_DIR;
        const Left = LEFT_DIR;
        const Right = RIGHT_DIR;

        const _UpAlt = UP_DIR | RIGHT_DIR | LEFT_DIR;
        const _DownAlt = DOWN_DIR | RIGHT_DIR| LEFT_DIR;
        const _LeftAlt = LEFT_DIR | UP_DIR | DOWN_DIR;
        const _RightAlt = RIGHT_DIR | UP_DIR | DOWN_DIR;
        const UpRight = UP_DIR | RIGHT_DIR;
        const UpLeft = UP_DIR | LEFT_DIR;
        const DownRight = DOWN_DIR | RIGHT_DIR;
        const DownLeft = DOWN_DIR | LEFT_DIR;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, BorrowDecode, Encode)]
pub enum Direction {
    Neutral,
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    DownLeft,
    UpRight,
    DownRight,
}

impl From<u8> for Direction {
    fn from(value: u8) -> Self {
        match value {
            1 => Direction::Up,
            2 => Direction::Down,
            3 => Direction::Left,
            4 => Direction::Right,
            5 => Direction::UpLeft,
            6 => Direction::DownLeft,
            7 => Direction::UpRight,
            8 => Direction::DownRight,
            _ => Direction::Neutral,
        }
    }
}

impl From<Direction> for u8 {
    fn from(val: Direction) -> Self {
        match val {
            Direction::Neutral => 0,
            Direction::Up => 1,
            Direction::Down => 2,
            Direction::Left => 3,
            Direction::Right => 4,
            Direction::UpLeft => 5,
            Direction::DownLeft => 6,
            Direction::UpRight => 7,
            Direction::DownRight => 8,
        }
    }
}

impl Direction {
    pub fn on_left_side(self) -> RelativeDirection {
        match self {
            Direction::Down => RelativeDirection::Down,
            Direction::DownLeft => RelativeDirection::DownBack,
            Direction::UpLeft => RelativeDirection::UpBack,
            Direction::Left => RelativeDirection::Back,
            Direction::Right => RelativeDirection::Forward,
            Direction::Neutral => RelativeDirection::Neutral,
            Direction::UpRight => RelativeDirection::UpForward,
            Direction::DownRight => RelativeDirection::DownForward,
            Direction::Up => RelativeDirection::Up,
        }
    }

    pub fn on_right_side(self) -> RelativeDirection {
        match self {
            Direction::Down => RelativeDirection::Down,
            Direction::DownLeft => RelativeDirection::DownForward,
            Direction::UpLeft => RelativeDirection::UpForward,
            Direction::Left => RelativeDirection::Forward,
            Direction::Right => RelativeDirection::Back,
            Direction::Neutral => RelativeDirection::Neutral,
            Direction::UpRight => RelativeDirection::UpBack,
            Direction::DownRight => RelativeDirection::DownBack,
            Direction::Up => RelativeDirection::Up,
        }
    }

    #[allow(dead_code)]
    pub fn inverse(self) -> Direction {
        match self {
            Direction::Down => Direction::Down,
            Direction::DownLeft => Direction::DownRight,
            Direction::UpLeft => Direction::UpRight,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Neutral => Direction::Neutral,
            Direction::UpRight => Direction::UpLeft,
            Direction::DownRight => Direction::DownLeft,
            Direction::Up => Direction::Up,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RelativeDirection {
    None,
    Neutral,
    Up,
    Down,
    Back,
    Forward,
    UpBack,
    DownBack,
    UpForward,
    DownForward,
}

impl RelativeDirection {
    /// Returns true if self and other match, or if self is none
    pub fn matches_or_is_none(self, other: Self) -> bool {
        self == other || self == Self::None
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct Motion: u32 {
        const NONE       = 0b0000_0000;
        const DownDown   = 0b0000_0001;
        const RightRight = 0b0000_0010;
        const LeftLeft   = 0b0000_0100;
        const QcRight    = 0b0000_1000;
        const QcLeft     = 0b0001_0000;
        const DpRight    = 0b0010_0000;
        const DpLeft     = 0b0100_0000;

        const LEFTS      = 0b0101_0100;
        const RIGHTS     = 0b0010_1010;
        const NEUTRALS   = 0b0000_0001;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct RelativeMotion: u32 {
        const NONE             = 0b0000_0000;
        const DownDown         = 0b0000_0001;
        const ForwardForward   = 0b0000_0010;
        const BackBack         = 0b0000_0100;
        const QcForward        = 0b0000_1000;
        const QcBack           = 0b0001_0000;
        const DpForward        = 0b0010_0000;
        const DpBack           = 0b0100_0000;
    }
}

impl Motion {
    pub fn on_left_side(self) -> RelativeMotion {
        RelativeMotion::from_bits_retain(self.bits())
    }

    pub fn on_right_side(self) -> RelativeMotion {
        let bits = self.bits();
        let shifted = (bits & Motion::LEFTS.bits()) >> 1
            | (bits & Motion::RIGHTS.bits()) << 1
            | (bits & Motion::NEUTRALS.bits());
        RelativeMotion::from_bits_retain(shifted)
    }
}

#[test]
fn test_insert_input() {
    let mut history = InputHistory::new(PLAYER1_BUTTONS, PLAYER1_DIRECTIONS, 0);

    history.handle_keypress(Keycode::W);
    history.update();
    history.update();
    history.handle_keyrelease(Keycode::W);
    history.update();
    history.update();

    history.append_input(0, Direction::Down, ButtonFlag::L);

    assert_eq!(
        [
            (Direction::Up, ButtonFlag::NONE, 3),
            (Direction::Down, ButtonFlag::L, 1)
        ],
        history.buf[1..3]
    );
}
