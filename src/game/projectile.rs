use sdl3::render::FRect;

use crate::game::boxes::HitBox;

pub struct Projectile {
    pos: FRect,
    hitbox: HitBox,
    animation: usize,
}
