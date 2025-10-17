use std::ops::Range;

use bitflags::bitflags;
use sdl3::render::FPoint;

use crate::game::{input::{ButtonFlag, Inputs, RelativeDirection, RelativeMotion}, Side};

type StateIndex = usize;

pub struct StateData {
    current_state: StateIndex,
    current_frame: usize,
    vel: FPoint,
    gravity_mult: f32,
    hit_connected: bool,
    side: Side,
}

impl StateData {
    pub fn new(side: Side) -> Self {
        Self {
            side,
            ..Default::default()
        }
    }
    pub fn current_state(&self) -> StateIndex {
        self.current_state
    }

    pub fn current_frame(&self) -> usize {
        self.current_frame
    }
    
    pub fn advance_frame(&mut self) {
        self.current_frame += 1;
    }

    pub fn on_hit_connect(&mut self) {
        self.hit_connected = true;
    }

    pub fn set_launch_hit_state(&mut self, states: &States) {
        match self.side {
            Side::Left => self.enter_state::<true>(states, states.launch_hit_state),
            Side::Right => self.enter_state::<false>(states, states.launch_hit_state),
        }
        
        self.gravity_mult *= 1.2;
    }

    pub fn set_side(&mut self, states: &States, new_side: Side) {
        if !states.flags[self.current_state].contains(StateFlags::LockSide) {
            self.side = new_side;
        }
    }

    pub fn get_side(&self) -> &Side {
        &self.side
    }

    pub fn vel(&self) -> FPoint {
        self.vel
    }

    pub fn set_vel(&mut self, new_vel: FPoint) {
        self.vel = new_vel;
    }

    pub fn gravity_mult(&self) -> f32 {
        self.gravity_mult
    }

    pub fn is_airborne(&self, states: &States) -> bool {
        states.flags[self.current_state].contains(StateFlags::Airborne)
    }

    pub fn ground(&mut self, states: &States) {
        if let EndBehavior::OnGroundedToStateY { y } = states.end_behaviors[self.current_state] {
            match self.side {
                Side::Left => self.enter_state::<true>(states, y),
                Side::Right => self.enter_state::<false>(states, y),
            }
            self.gravity_mult = 1.0;
        }
    }

    pub fn update(&mut self, states: &States, inputs: &Inputs) {
        match self.side {
            Side::Left => {
                self.check_state_end::<true>(states);
                self.check_cancels::<true>(states, inputs);
            },
            Side::Right => {
                self.check_state_end::<false>(states);
                self.check_cancels::<false>(states, inputs);
            }
        }
    }

    fn check_state_end<const LEFT_SIDE: bool>(&mut self, states: &States) {
        match states.end_behaviors[self.current_state] {
            EndBehavior::Endless => {},
            EndBehavior::OnFrameXToStateY { x: end_frame, y: transition_state } => {
                if self.current_frame >= end_frame {
                    self.enter_state::<LEFT_SIDE>(states, transition_state);
                }
            },
            EndBehavior::OnGroundedToStateY { .. } => {},
        }
    }

    fn check_cancels<const LEFT_SIDE: bool>(&mut self, states: &States, inputs: &Inputs) {
        // Check if not in cancel window
        if !self.in_cancel_window(states) {
            return;
        }

        let cancel_options_range = states.cancel_options[self.current_state].clone();
        let cancel_options = &states.run_length_cancel_options[cancel_options_range];
        for i in cancel_options.iter() {
            let cancel_option = &states.inputs[*i];
            // Check direction first
            let relative_dir = if LEFT_SIDE == true {
                &inputs.dir().on_left_side()
            } else {
                &inputs.dir().on_right_side()
            };
            if !cancel_option.dir.matches_or_is_none(relative_dir) {
                continue;
            }

            let maybe_index = if LEFT_SIDE {
                inputs
                .move_buf()
                .iter()
                .map(|(motion, buttons)| (motion.on_left_side(), *buttons))
                .position(|(buf_motion, buf_buttons)| {
                    cancel_option.motion.matches_or_is_none(&buf_motion) &&
                    buf_buttons.contains(cancel_option.button)
                })
            } else {
                inputs
                .move_buf()
                .iter()
                .map(|(motion, buttons)| (motion.on_right_side(), *buttons))
                .position(|(buf_motion, buf_buttons)| {
                    cancel_option.motion.matches_or_is_none(&buf_motion) &&
                    buf_buttons.contains(cancel_option.button)
                })
            };

            if let Some(_) = maybe_index {
                self.enter_state::<LEFT_SIDE>(states, *i);
                break;
            }
        }
    }

    fn in_cancel_window(&self, states: &States) -> bool {
        states.cancel_windows[self.current_state].contains(&self.current_frame) &&
        (self.hit_connected ||
            states.flags[self.current_state].contains(StateFlags::CancelOnWhiff))
    }

    fn enter_state<const LEFT_SIDE: bool>(&mut self, states: &States, new_state: StateIndex) {
        self.current_state = new_state;
        self.current_frame = 0;
        self.hit_connected = false;
        match states.start_behaviors[new_state] {
            StartBehavior::None => {},
            StartBehavior::SetVel { x, y } => {
                let x = if LEFT_SIDE { x } else { -x };
                self.vel = FPoint::new(x, y);
            }
        }
    }
}

impl Default for StateData {
    fn default() -> Self {
        Self {
            current_state: 0,
            current_frame: 0,
            vel: FPoint::new(0.0, 0.0),
            gravity_mult: 1.0,
            hit_connected: false,
            side: Side::Left,
        }
    }
}

#[derive(Default, Debug)]
pub struct States {
    // Input
    inputs: Vec<MoveInput>,
    // Animation data: The state number points to the animation in the character
    // Cancel data
    cancel_windows: Vec<Range<usize>>,
    cancel_options: Vec<Range<usize>>,
    // Boxes
    hit_boxes_start: Vec<usize>,
    hurt_boxes_start: Vec<usize>,
    // Behavior
    start_behaviors: Vec<StartBehavior>,
    flags: Vec<StateFlags>,
    end_behaviors: Vec<EndBehavior>,

    // Run length stuff
    run_length_hit_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hitboxes index range
    run_length_hurt_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hurtboxes index range
    run_length_cancel_options: Vec<StateIndex>,

    // Special cached states
    ground_hit_state: StateIndex,
    launch_hit_state: StateIndex,
}

impl States {
    pub fn init(
        inputs: Vec<MoveInput>,
        cancel_windows: Vec<Range<usize>>,
        cancel_options: Vec<Range<usize>>,
        hit_boxes_start: Vec<usize>,
        hurt_boxes_start: Vec<usize>,
        flags: Vec<StateFlags>,
        start_behaviors: Vec<StartBehavior>,
        end_behaviors: Vec<EndBehavior>,
        run_length_hit_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hitboxes index range
        run_length_hurt_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hurtboxes index range
        run_length_cancel_options: Vec<StateIndex>,
        ground_hit_state: StateIndex,
        launch_hit_state: StateIndex,
    ) -> Self {
        Self {
            inputs,
            cancel_windows,
            cancel_options,
            hit_boxes_start,
            hurt_boxes_start,
            flags,
            start_behaviors,
            end_behaviors,
            run_length_hit_boxes,
            run_length_hurt_boxes,
            run_length_cancel_options,
            ground_hit_state,
            launch_hit_state,
        }
    }

    pub fn hit_box_range(&self, state_data: &StateData) -> Range<usize> {
        if state_data.hit_connected {
            return 0..0;
        }

        self.hit_box_range_no_check(state_data)
    }

    pub fn hit_box_range_no_check(&self, state_data: &StateData) -> Range<usize> {
        let current_state = state_data.current_state;
        let mut current_frame = state_data.current_frame;
        let mut run_start = self.hit_boxes_start[current_state];
        
        loop {
            let (frames, range) = &self.run_length_hit_boxes[run_start];
            if current_frame < *frames {
                return range.clone();
            }
            current_frame -= frames;
            run_start += 1;
        }
    }

    pub fn hurt_box_range(&self, state_data: &StateData) -> Range<usize> {
        let current_state = state_data.current_state;
        let mut current_frame = state_data.current_frame;
        let mut run_start = self.hurt_boxes_start[current_state];
        
        loop {
            let (frames, range) = &self.run_length_hurt_boxes[run_start];
            if current_frame < *frames {
                return range.clone();
            }
            current_frame -= frames;
            run_start += 1;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MoveInput {
    button: ButtonFlag,
    motion: RelativeMotion,
    dir: RelativeDirection,
}

impl MoveInput {
    pub fn new(button: ButtonFlag, motion: RelativeMotion, dir: RelativeDirection) -> Self {
        Self {button, motion, dir}
    }
}

#[derive(Debug)]
pub enum StartBehavior {
    None,
    SetVel {x: f32, y: f32}
}

#[derive(Debug)]
pub enum EndBehavior {
    Endless,
    OnFrameXToStateY {x: usize, y: StateIndex},
    OnGroundedToStateY { y: StateIndex },
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct StateFlags: u32 {
        const NONE = 0;
        const Airborne =      0b00000001;
        const CancelOnWhiff = 0b00000010;
        const LockSide =      0b00000100;
    }
}