use sdl3::render::FRect;

use crate::game::boxes::HitBox;

#[allow(dead_code)]
pub struct Projectile {
    pos: FRect,
    hitbox: HitBox,
    animation: usize,
}
