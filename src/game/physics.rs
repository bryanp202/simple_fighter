use sdl3::render::{FPoint, FRect};

use crate::game::boxes::{HitBox, HurtBox};

const GRAVITY_CONSTANT: f32 = 0.4;

pub fn velocity_system(pos: &FPoint, vel: &FPoint) -> FPoint {
    FPoint::new((pos.x + vel.x).clamp(-300.0, 300.0), pos.y + vel.y)
}

/// Returns true if grounded
pub fn gravity_system(pos: &FPoint, vel: &FPoint, gravity_mult: f32) -> (FPoint, FPoint, bool) {
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

pub fn check_hit_collisions(hit_box_offset: FPoint, hit_boxes: &[HitBox], hurt_box_offset: FPoint, hurt_boxes: &[HurtBox]) -> Option<HitBox> {
    for hit_box in hit_boxes.iter() {
        let hit_box_with_offset = hit_box.pos_with_offset(hit_box_offset);
        for hurt_box in hurt_boxes.iter() {
            let hurt_box_with_offset = hurt_box.pos_with_offset(hurt_box_offset);
            if aabb_collision(hit_box_with_offset, hurt_box_with_offset) {
                return Some(hit_box.clone());
            }
        }
    }
    None
}

fn aabb_collision(rect1: FRect, rect2: FRect) -> bool {
    rect1.x < rect2.x + rect2.w &&
    rect1.x + rect1.w > rect2.x &&
    rect1.y < rect2.y + rect2.h &&
    rect1.y + rect1.h > rect2.y
}
