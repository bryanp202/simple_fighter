use std::ops::Range;

use crate::game::input::MoveInput;

pub struct State {
    // Input
    input: MoveInput,

    // Animation data
    frames: usize,
    animation: usize,

    // Cancel data
    cancel_frames: Range<usize>,
    cancel_options: Vec<usize>,

    // Boxes
    hit_boxes: usize,
    hurt_boxes: usize,
    collision_boxes: usize,
}