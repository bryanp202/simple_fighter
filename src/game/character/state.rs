use std::ops::Range;

use bitflags::bitflags;
use sdl3::render::FPoint;

use crate::game::input::{ButtonFlag, Inputs, RelativeDirection, RelativeMotion};

type StateIndex = usize;

pub struct StateData {
    current_state: StateIndex,
    current_frame: usize,
    vel: FPoint,
}

impl StateData {
    pub fn current_state(&self) -> StateIndex {
        self.current_state
    }

    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    pub fn vel(&self) -> FPoint {
        self.vel
    }
}

impl Default for StateData {
    fn default() -> Self {
        Self {
            current_state: 0,
            current_frame: 0,
            vel: FPoint::new(0.0, 0.0),
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

        }
    }

    pub fn hit_box_range(&self, current_state: StateIndex, mut current_frame: usize) -> Range<usize> {
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

    pub fn hurt_box_range(&self, current_state: StateIndex, mut current_frame: usize) -> Range<usize> {
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

    pub fn update(&mut self, state_data: &mut StateData, inputs: &Inputs) {
        state_data.current_frame += 1;
        self.check_state_end(state_data);
        self.check_cancels(state_data, inputs);
    }

    fn check_state_end(&mut self, state_data: &mut StateData) {
        match self.end_behaviors[state_data.current_state] {
            EndBehavior::Endless => {},
            EndBehavior::OnFrameXToStateY { x: end_frame, y: transition_state } => {
                if state_data.current_frame >= end_frame {
                    self.enter_state(state_data, transition_state);
                }
            },
            EndBehavior::OnGroundedToStateY { y: transition_state } => {

            },
        }
    }

    fn check_cancels(&mut self, state_data: &mut StateData, inputs: &Inputs) {
        // Check if not in cancel window
        if !self.in_cancel_window(state_data) {
            return;
        }

        let cancel_options_range = self.cancel_options[state_data.current_state].clone();
        let cancel_options = &self.run_length_cancel_options[cancel_options_range];
        for i in cancel_options.iter() {
            let cancel_option = &self.inputs[*i];

            // Check direction first
            if !cancel_option.dir.matches_or_is_none(&inputs.dir().on_left_side()) {
                continue;
            }

            let maybe_index = inputs
                .move_buf()
                .iter()
                .position(|(buf_motion, buf_buttons)| {
                    cancel_option.motion.matches_or_is_none(&buf_motion.on_left_side()) &&
                    buf_buttons.contains(cancel_option.button)
                });
            if let Some(_) = maybe_index {
                self.enter_state(state_data, *i);
                break;
            }
        }
    }

    fn in_cancel_window(&self, state_data: &StateData) -> bool {
        self.cancel_windows[state_data.current_state].contains(&state_data.current_frame)
    }

    fn enter_state(&mut self, state_data: &mut StateData, new_state: StateIndex) {
        state_data.current_state = new_state;
        state_data.current_frame = 0;
        match self.start_behaviors[new_state] {
            StartBehavior::SetVel { x, y } => {
                state_data.vel = FPoint::new(x, y);
            }
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
    }
}