use std::ops::Range;

use bitflags::bitflags;
use sdl3::render::FRect;

use crate::game::{boxes::CollisionBox, input::{ButtonFlag, RelativeMotion}};

type StateIndex = usize;

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
}

#[derive(Debug)]
pub struct MoveInput {
    button: ButtonFlag,
    motion: RelativeMotion,
}

impl MoveInput {
    pub fn new(button: ButtonFlag, motion: RelativeMotion) -> Self {
        Self {button, motion}
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