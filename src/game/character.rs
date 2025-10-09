mod state;
mod deserialize;

use crate::game::{boxes::{CollisionBox, HitBox, HurtBox}, character::deserialize::deserialize, projectile::Projectile, render::animation::Animation};
use sdl3::{render::{FRect, Texture, TextureCreator}, video::WindowContext};
use state::States;

pub struct Character {
    hp: f32,
    pos: FRect,
    current_state: usize,
    states: States,
    projectiles: Vec<Projectile>,
    hit_box_data: Vec<HitBox>,
    hurt_box_data: Vec<HurtBox>,
    collision_box_data: Vec<CollisionBox>,
    animation_data: Vec<Animation>,
}

impl Character {
    pub fn from_config<'a>(texture_creator: &'a TextureCreator<WindowContext>, global_textures: &mut Vec<Texture<'a>>, config: &str) -> Result<Self, String> {
        deserialize(texture_creator, global_textures, config)
    }
}