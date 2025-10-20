use std::ops::Range;

use bitflags::bitflags;
use sdl3::render::FPoint;

use crate::game::{
    Side,
    input::{ButtonFlag, RelativeDirection, RelativeMotion},
};

type StateIndex = usize;
const HIT_GRAVITY_MULT: f32 = 1.2;

pub struct StateData {
    current_state: StateIndex,
    current_frame: usize,
    side: Side,
    vel: FPoint,
    gravity_mult: f32,
    hit_connected: bool,
    stun: usize,
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

    pub fn set_block_stun_state(&mut self, states: &States, hit_stun: usize) {
        self.stun = hit_stun;
        self.enter_state(states, states.block_stun_state);
    }

    pub fn set_hit_state(&mut self, states: &States, hit_stun: usize) {
        if self.current_state == states.launch_hit_state ||
        hit_stun == u32::MAX as usize {
            self.enter_state(states, states.launch_hit_state);
            self.gravity_mult *= HIT_GRAVITY_MULT;
        } else {
            self.stun = hit_stun;
            self.enter_state(states, states.ground_hit_state);
        }
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

    pub fn vel_rel(&self) -> FPoint {
        match self.side {
            Side::Left => FPoint::new(self.vel.x, self.vel.y),
            Side::Right => FPoint::new(-self.vel.x, self.vel.y),
        }
    }

    pub fn set_vel(&mut self, new_vel: FPoint) {
        self.vel = new_vel;
    }

    pub fn gravity_mult(&self) -> f32 {
        self.gravity_mult
    }

    pub fn is_blocking_mid(&self, states: &States) -> bool {
        states.flags[self.current_state].intersects(StateFlags::LowBlock | StateFlags::HighBlock)
    }

    pub fn is_blocking_low(&self, states: &States) -> bool {
        states.flags[self.current_state].contains(StateFlags::LowBlock)
    }

    pub fn is_blocking_high(&self, states: &States) -> bool {
        states.flags[self.current_state].contains(StateFlags::HighBlock)
    }

    pub fn is_friction_on(&self, states: &States) -> bool {
        states.flags[self.current_state].contains(StateFlags::FrictionOn)
    }

    pub fn is_airborne(&self, states: &States) -> bool {
        states.flags[self.current_state].contains(StateFlags::Airborne)
    }

    pub fn ground(&mut self, states: &States) {
        if let EndBehavior::OnGroundedToStateY { y } = states.end_behaviors[self.current_state] {
            self.enter_state(states, y);
            self.gravity_mult = 1.0;
        }
    }

    pub fn update<T>(&mut self, states: &States, dir: RelativeDirection, move_iter: T)
    where T: Iterator<Item = (RelativeMotion, ButtonFlag)> + Clone {
        self.check_state_end(states);
        self.check_cancels(states, dir, move_iter);
    }

    fn check_state_end(&mut self, states: &States) {
        match states.end_behaviors[self.current_state] {
            EndBehavior::Endless => {}
            EndBehavior::OnStunEndToStateY { y: transition_state } => {
                if self.current_frame >= self.stun {
                    self.enter_state(states, transition_state);
                }
            }
            EndBehavior::OnFrameXToStateY {
                x: end_frame,
                y: transition_state,
            } => {
                if self.current_frame >= end_frame {
                    self.enter_state(states, transition_state);
                }
            }
            EndBehavior::OnGroundedToStateY { .. } => {}
        }
    }

    fn check_cancels<T>(&mut self, states: &States, dir: RelativeDirection, move_iter: T)
    where T: Iterator<Item = (RelativeMotion, ButtonFlag)> + Clone {
        // Check if not in cancel window
        if !self.in_cancel_window(states) {
            return;
        }

        let cancel_options_range = states.cancel_options[self.current_state].clone();
        let cancel_options = &states.run_length_cancel_options[cancel_options_range];
        for i in cancel_options.iter() {
            let cancel_option = &states.inputs[*i];
            if !cancel_option.dir.matches_or_is_none(&dir) {
                continue;
            }

            let maybe_index = move_iter
                .clone()
                .position(|(buf_motion, buf_buttons)| {
                    cancel_option.motion.matches_or_is_none(&buf_motion)
                        && buf_buttons.contains(cancel_option.button)
                });

            if let Some(_) = maybe_index {
                self.enter_state(states, *i);
                break;
            }
        }
    }

    fn in_cancel_window(&self, states: &States) -> bool {
        states.cancel_windows[self.current_state].contains(&self.current_frame)
            && (self.hit_connected
                || states.flags[self.current_state].contains(StateFlags::CancelOnWhiff))
    }

    fn enter_state(&mut self, states: &States, new_state: StateIndex) {
        self.current_state = new_state;
        self.current_frame = 0;
        self.hit_connected = false;
        match states.start_behaviors[new_state] {
            StartBehavior::None => {}
            StartBehavior::SetVel { x, y } => {
                self.vel = FPoint::new(x, y);
            },
            StartBehavior::AddVel { x, y } => {
                self.vel = FPoint::new(self.vel.x + x, self.vel.y + y);
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
            stun: 0,
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
    block_stun_state: StateIndex,
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
        block_stun_state: StateIndex,
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
            block_stun_state,
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
        Self {
            button,
            motion,
            dir,
        }
    }
}

#[derive(Debug)]
pub enum StartBehavior {
    None,
    SetVel { x: f32, y: f32 },
    AddVel { x: f32, y: f32 },
}

#[derive(Debug)]
pub enum EndBehavior {
    Endless,
    OnStunEndToStateY {y: StateIndex},
    OnFrameXToStateY { x: usize, y: StateIndex },
    OnGroundedToStateY { y: StateIndex },
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct StateFlags: u32 {
        const NONE = 0;
        const Airborne =      0b00000001;
        const CancelOnWhiff = 0b00000010;
        const LockSide =      0b00000100;
        const LowBlock =      0b00001000;
        const HighBlock =     0b00010000;
        const FrictionOn =    0b00100000;
    }
}
