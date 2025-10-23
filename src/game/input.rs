use bitflags::bitflags;
use sdl3::keyboard::Keycode;

const DIRECTION_COUNT: usize = 4;
const BUTTON_COUNT: usize = 3;
const INPUT_VARIANTS: usize = 1;

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

pub struct Inputs {
    state: InputState,
    input_history: InputHistory,
}

impl Inputs {
    pub fn new(key_to_button: KeyToButtons, key_to_direction: KeyToDirections) -> Self {
        Self {
            state: InputState::new(key_to_button, key_to_direction),
            input_history: InputHistory::new(),
        }
    }

    pub fn reset(&mut self) {
        self.input_history = InputHistory::new();
    }

    pub fn held_buttons(&self) -> ButtonFlag {
        self.state.buttons_pressed
    }

    pub fn just_pressed_buttons(&self) -> ButtonFlag {
        self.state.buttons_just_pressed
    }

    pub fn dir(&self) -> Direction {
        self.state.dir
    }

    pub fn move_buf(&self) -> MoveBuffer {
        self.input_history.motion_buf
    }

    pub fn handle_keypress(&mut self, keycode: Keycode) {
        self.state.handle_keypress(keycode);
    }

    pub fn handle_keyrelease(&mut self, keycode: Keycode) {
        self.state.handle_keyrelease(keycode);
    }

    pub fn update(&mut self) {
        self.state.update();
        self.input_history
            .update(self.dir(), self.held_buttons(), self.just_pressed_buttons());
    }
}

type KeyToButtons = [(Keycode, ButtonFlag); BUTTON_COUNT * INPUT_VARIANTS];
type KeyToDirections = [(Keycode, DirectionFlag); DIRECTION_COUNT * INPUT_VARIANTS];
struct InputState {
    dir: Direction,
    held_dir: DirectionFlag,

    buttons_pressed: ButtonFlag,
    buttons_just_pressed_temp: ButtonFlag,
    buttons_just_pressed: ButtonFlag,

    key_to_button: KeyToButtons,
    key_to_direction: KeyToDirections,
}

impl InputState {
    pub fn new(key_to_button: KeyToButtons, key_to_direction: KeyToDirections) -> Self {
        Self {
            dir: Direction::Neutral,
            held_dir: DirectionFlag::Neutral,
            buttons_pressed: ButtonFlag::NONE,
            buttons_just_pressed_temp: ButtonFlag::NONE,
            buttons_just_pressed: ButtonFlag::NONE,
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
            self.buttons_pressed |= pressed_button;
            self.buttons_just_pressed_temp |= pressed_button;
        } else {
            let dir_pairing = self.key_to_direction.iter().find_map(|pair| {
                if pair.0 == keycode {
                    Some(pair.1)
                } else {
                    None
                }
            });

            if let Some(pressed_direction) = dir_pairing {
                self.held_dir |= pressed_direction;
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
            self.buttons_pressed ^= pressed_button;
        } else {
            let dir_pairing = self.key_to_direction.iter().find_map(|pair| {
                if pair.0 == keycode {
                    Some(pair.1)
                } else {
                    None
                }
            });

            if let Some(pressed_direction) = dir_pairing {
                self.held_dir ^= pressed_direction;
            }
        }
    }

    fn update(&mut self) {
        self.dir = match self.held_dir {
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

        self.buttons_just_pressed = self.buttons_just_pressed_temp;
        self.buttons_just_pressed_temp = ButtonFlag::NONE;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct ButtonFlag: u32 {
        const NONE = 0;
        const L = 0b00000001;
        const M = 0b00000010;
        const H = 0b00000100;
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

#[derive(Clone, Copy, Debug, PartialEq)]
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

impl Direction {
    pub fn on_left_side(&self) -> RelativeDirection {
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

    pub fn on_right_side(&self) -> RelativeDirection {
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
    pub fn matches_or_is_none(&self, other: &Self) -> bool {
        *self == *other || *self == Self::None
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct Motion: u32 {
        const NONE       = 0b00000000;
        const DownDown   = 0b00000001;
        const RightRight = 0b00000010;
        const LeftLeft   = 0b00000100;
        const QcRight    = 0b00001000;
        const QcLeft     = 0b00010000;
        const DpRight    = 0b00100000;
        const DpLeft     = 0b01000000;
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct RelativeMotion: u32 {
        const NONE             = 0b00000000;
        const DownDown         = 0b00000001;
        const ForwardForward   = 0b00000010;
        const BackBack         = 0b00000100;
        const QcForward        = 0b00001000;
        const QcBack           = 0b00010000;
        const DpForward        = 0b00100000;
        const DpBack           = 0b01000000;
        const Inverse          = 0b10000000;
    }
}

impl Motion {
    pub fn on_left_side(&self) -> RelativeMotion {
        RelativeMotion::from_bits_retain(self.bits())
    }

    pub fn on_right_side(&self) -> RelativeMotion {
        RelativeMotion::from_bits_retain(self.bits()) | RelativeMotion::Inverse
    }
}

type MoveBuffer = [(Motion, ButtonFlag); InputHistory::MOTION_BUF_SIZE];

struct InputHistory {
    buf: [(Direction, ButtonFlag, usize); Self::HISTORY_FRAME_LEN],
    motion_buf: MoveBuffer,
    current_index: usize,
}

impl InputHistory {
    const HISTORY_FRAME_LEN: usize = 32;
    const DASH_HISTORY_LEN: usize = Self::HISTORY_FRAME_LEN / 2;
    const MOTION_BUF_SIZE: usize = 4;

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
    const QC_RIGHT_INVERSE: &[Direction] =
        &[Direction::Right, Direction::DownRight, Direction::Down];
    const QC_LEFT_INVERSE: &[Direction] = &[Direction::Left, Direction::DownLeft, Direction::Down];
    // Least Valuable Motion Input
    const RIGHT_RIGHT_INVERSE: &[Direction] =
        &[Direction::Right, Direction::Neutral, Direction::Right];
    const LEFT_LEFT_INVERSE: &[Direction] = &[Direction::Left, Direction::Neutral, Direction::Left];
    // Second Least Valuable Motion Input
    const DOWN_DOWN_INVERSE: &[Direction] = &[Direction::Down, Direction::Neutral, Direction::Down];

    fn new() -> Self {
        Self {
            buf: std::array::from_fn(|_| (Direction::Neutral, ButtonFlag::NONE, 1)),
            motion_buf: std::array::from_fn(|_| (Motion::NONE, ButtonFlag::NONE)),
            current_index: 0,
        }
    }

    fn update(
        &mut self,
        dir: Direction,
        held_buttons: ButtonFlag,
        just_pressed_buttons: ButtonFlag,
    ) {
        self.append_input(dir, held_buttons);
        self.shift_motion_buf(just_pressed_buttons);
    }

    fn shift_motion_buf(&mut self, just_pressed_buttons: ButtonFlag) {
        let mut new_buf: MoveBuffer = std::array::from_fn(|_| (Motion::NONE, ButtonFlag::NONE));
        new_buf[1..].copy_from_slice(&self.motion_buf[0..Self::MOTION_BUF_SIZE - 1]);
        new_buf[0] = (self.parse_motion(), just_pressed_buttons);
        self.motion_buf = new_buf;
    }

    fn append_input(&mut self, input_dir: Direction, input_buttons: ButtonFlag) {
        let (dir, buttons, frames) = &mut self.buf[self.current_index];
        if *dir == input_dir && *buttons == input_buttons {
            *frames += 1;
        } else {
            self.current_index = (self.current_index + 1) % Self::HISTORY_FRAME_LEN;
            self.buf[self.current_index] = (input_dir, input_buttons, 1);
        }
    }

    /// Returns the most recent and most valuable motion stored
    fn parse_motion(&self) -> Motion {
        let mut result = Motion::NONE;

        let mut ordered_frames = [Direction::Neutral; Self::HISTORY_FRAME_LEN];
        let mut frame_count = 0;
        let mut i = 0;
        let mut dash_buffer_end = None;
        while frame_count < Self::HISTORY_FRAME_LEN {
            if dash_buffer_end.is_none() && frame_count >= Self::DASH_HISTORY_LEN {
                dash_buffer_end = Some(i);
            }
            let current_index =
                (Self::HISTORY_FRAME_LEN + self.current_index - i) % Self::HISTORY_FRAME_LEN;
            let (dir, _, frames) = &self.buf[current_index];
            ordered_frames[i] = *dir;
            frame_count += *frames;
            i += 1;
        }
        let motion_slice = &ordered_frames[0..i];
        let dash_slice = &ordered_frames[0..dash_buffer_end.unwrap_or(i)];

        let right_dp = Self::find_dir_sequence(motion_slice, Self::DP_RIGHT_INVERSE);
        let left_dp = Self::find_dir_sequence(motion_slice, Self::DP_LEFT_INVERSE);
        result |= match (right_dp, left_dp) {
            (Some(_), None) => Motion::DpRight,
            (None, Some(_)) => Motion::DpLeft,
            (Some(right), Some(left)) => {
                if right <= left {
                    Motion::DpRight
                } else {
                    Motion::DpLeft
                }
            }
            _ => Motion::NONE,
        };

        let right_qc = Self::find_dir_sequence(motion_slice, Self::QC_RIGHT_INVERSE);
        let left_qc = Self::find_dir_sequence(motion_slice, Self::QC_LEFT_INVERSE);
        result |= match (right_qc, left_qc) {
            (Some(_), None) => Motion::QcRight,
            (None, Some(_)) => Motion::QcLeft,
            (Some(right), Some(left)) => {
                if right <= left {
                    Motion::QcRight
                } else {
                    Motion::QcLeft
                }
            }
            _ => Motion::NONE,
        };

        let right_right = Self::find_dir_sequence(dash_slice, Self::RIGHT_RIGHT_INVERSE);
        let left_left = Self::find_dir_sequence(dash_slice, Self::LEFT_LEFT_INVERSE);
        result |= match (right_right, left_left) {
            (Some(_), None) => Motion::RightRight,
            (None, Some(_)) => Motion::LeftLeft,
            (Some(right), Some(left)) => {
                if right <= left {
                    Motion::RightRight
                } else {
                    Motion::LeftLeft
                }
            }
            _ => Motion::NONE,
        };

        result |= if let Some(_) = Self::find_dir_sequence(motion_slice, Self::DOWN_DOWN_INVERSE) {
            Motion::DownDown
        } else {
            Motion::NONE
        };

        result
    }

    fn find_dir_sequence(haystack: &[Direction], seq: &[Direction]) -> Option<usize> {
        haystack.windows(seq.len()).position(|window| window == seq)
    }
}
