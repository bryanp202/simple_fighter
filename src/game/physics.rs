use std::cmp::Ordering;

use sdl3::render::{FPoint, FRect};

use crate::game::{
    Side,
    boxes::{CollisionBox, HitBox, HurtBox},
    stage::Stage,
};

const GRAVITY_CONSTANT: f32 = 0.4;
const FRICTION_COEFFICIENT: f32 = 0.6;

pub fn velocity_system(pos: FPoint, vel: FPoint) -> FPoint {
    FPoint::new(pos.x + vel.x, pos.y + vel.y)
}

pub fn friction_system(vel: FPoint) -> FPoint {
    FPoint::new(vel.x * FRICTION_COEFFICIENT, vel.y * FRICTION_COEFFICIENT)
}

/// Returns true if grounded
/// Returns (pos, vel, grounded)
pub fn gravity_system(pos: FPoint, vel: FPoint, gravity_mult: f32) -> (FPoint, FPoint, bool) {
    if pos.y <= 0.0 {
        let new_pos = FPoint::new(pos.x, 0.0);
        let new_vel = FPoint::new(vel.x, 0.0);
        (new_pos, new_vel, true)
    } else {
        let new_pos = FPoint::new(pos.x, pos.y);
        let new_vel = FPoint::new(vel.x, vel.y - GRAVITY_CONSTANT * gravity_mult);
        (new_pos, new_vel, false)
    }
}

/// Returns true if pos1 is on the left/player1 side
pub fn side_detection(pos1: FPoint, pos2: FPoint) -> Option<Side> {
    match pos1.x.partial_cmp(&pos2.x) {
        Some(Ordering::Less) => Some(Side::Left),
        Some(Ordering::Greater) => Some(Side::Right),
        _ => None,
    }
}

pub fn check_hit_collisions(
    hit_side: Side,
    hit_box_offset: FPoint,
    hit_boxes: &[HitBox],
    hurt_side: Side,
    hurt_box_offset: FPoint,
    hurt_boxes: &[HurtBox],
) -> Option<HitBox> {
    for hit_box in hit_boxes {
        let hit_box_with_offset = hit_box.on_side(hit_side, hit_box_offset);
        for hurt_box in hurt_boxes {
            let hurt_box_with_offset = hurt_box.on_side(hurt_side, hurt_box_offset);
            if aabb_collision(hit_box_with_offset, hurt_box_with_offset) {
                return Some(hit_box.clone());
            }
        }
    }
    None
}

// Returns (player1_pos, player2_pos)
pub fn movement_system(
    pos1_side: Side,
    pos1: FPoint,
    box1: &CollisionBox,
    pos2_side: Side,
    pos2: FPoint,
    box2: &CollisionBox,
    stage: &Stage,
) -> (FPoint, FPoint) {
    let pos1 = stage.bind_pos(pos1);
    let pos2 = stage.bind_pos(pos2);

    let rect1 = box1.on_side(pos1_side, pos1);
    let rect2 = box2.on_side(pos2_side, pos2);
    let x_overlap = aabb_x_overlap(rect1, rect2);

    if x_overlap == 0.0 {
        (pos1, pos2)
    } else {
        let pos1_x_shift = -x_overlap / 2.0;
        let pos2_x_shift = x_overlap / 2.0;

        let new_pos1 = FPoint::new(pos1.x + pos1_x_shift, pos1.y);
        let new_pos2 = FPoint::new(pos2.x + pos2_x_shift, pos2.y);
        (stage.bind_pos(new_pos1), stage.bind_pos(new_pos2))
    }
}

fn aabb_collision(rect1: FRect, rect2: FRect) -> bool {
    rect1.x < rect2.x + rect2.w
        && rect1.x + rect1.w > rect2.x
        && rect1.y >= rect2.y - rect2.h
        && rect1.y - rect1.h <= rect2.y
}

fn aabb_x_overlap(rect1: FRect, rect2: FRect) -> f32 {
    if rect1.y >= rect2.y - rect2.h && rect1.y - rect1.h <= rect2.y {
        match rect1.x.partial_cmp(&rect2.x) {
            Some(Ordering::Less) => (rect1.x + rect1.w - rect2.x).max(0.0),
            Some(Ordering::Greater) => -(rect2.x + rect2.w - rect1.x).max(0.0),
            _ => {
                if rect1.y >= rect2.y {
                    if rect1.x >= 0.0 {
                        (rect1.x + rect1.w - rect2.x).max(0.0)
                    } else {
                        -(rect2.x + rect2.w - rect1.x).max(0.0)
                    }
                } else if rect2.x >= 0.0 {
                    -(rect2.x + rect2.w - rect1.x).max(0.0)
                } else {
                    (rect1.x + rect1.w - rect2.x).max(0.0)
                }
            }
        }
    } else {
        0.0
    }
}
