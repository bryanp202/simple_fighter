use std::ops::Range;

use sdl3::render::FRect;

use crate::game::{boxes::CollisionBox, input::MoveInput};

#[derive(Default)]
pub struct States {
    // Input
    inputs: Vec<MoveInput>,
    // Animation data: The state number points to the animation in the character
    // Cancel data
    cancel_frames: Vec<Range<usize>>,
    cancel_options: Vec<Range<usize>>,
    // Boxes
    hit_boxes_start: Vec<usize>,
    hurt_boxes_start: Vec<usize>,
    collision_boxes: Vec<CollisionBox>,
    // Behavior
    flags: Vec<usize>,
    start_behaviors: Vec<usize>,
    end_behaviors: Vec<usize>,

    // Run length stuff
    run_length_hit_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hitboxes index range
    run_length_hurt_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hurtboxes index range
    run_length_cancel_options: Vec<usize>,
}

impl States {
    pub fn init(
        inputs: Vec<MoveInput>,
        cancel_frames: Vec<Range<usize>>,
        cancel_options: Vec<Range<usize>>,
        hit_boxes_start: Vec<usize>,
        hurt_boxes_start: Vec<usize>,
        flags: Vec<usize>,
        start_behaviors: Vec<usize>,
        end_behaviors: Vec<usize>,
        run_length_hit_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hitboxes index range
        run_length_hurt_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hurtboxes index range
        run_length_cancel_options: Vec<usize>,
        collision_boxes: Vec<CollisionBox>,
    ) -> Self {
        Self {
            inputs,
            cancel_frames,
            cancel_options,
            hit_boxes_start,
            hurt_boxes_start,
            collision_boxes,
            flags,
            start_behaviors,
            end_behaviors,
            run_length_hit_boxes,
            run_length_hurt_boxes,
            run_length_cancel_options,

        }
    }
}