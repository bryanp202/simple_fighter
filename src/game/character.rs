mod state;

use crate::game::{boxes::{CollisionBox, HitBox, HurtBox}, projectile::Projectile, render::animation::Animation};
use sdl3::render::FRect;
use state::State;

pub struct Character {
    hp: f32,
    pos: FRect,
    current_state: usize,
    states: Vec<State>,
    projectiles: Vec<Projectile>,
    hit_box_data: Vec<HitBox>,
    hurt_box_data: Vec<HurtBox>,
    collision_box_data: Vec<CollisionBox>,
    animation_data: Vec<Animation>,
}

impl Character {
    pub fn new() -> Self {
        Self {
            hp: 0.0,
            pos: FRect::new(0.0, 0.0, 0.0, 0.0),
            current_state: 0,
            states: Vec::new(),
            projectiles: Vec::new(),
            hit_box_data: Vec::new(),
            hurt_box_data: Vec::new(),
            collision_box_data: Vec::new(),
            animation_data: Vec::new(),
        }
    }
}