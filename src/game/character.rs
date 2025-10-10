mod state;
mod deserialize;

use crate::game::{boxes::{CollisionBox, HitBox, HurtBox}, character::deserialize::deserialize, projectile::Projectile, render::animation::Animation};
use sdl3::{render::{Canvas, FPoint, FRect, Texture, TextureCreator}, video::{Window, WindowContext}};
use state::States;

pub struct Character {
    name: String,
    hp: f32,
    pos: FPoint,
    
    // State Data
    states: States,
    current_frame: usize,
    current_state: usize,
    
    // Other Data
    projectiles: Vec<Projectile>,
    hit_box_data: Vec<HitBox>,
    hurt_box_data: Vec<HurtBox>,
    collision_box_data: Vec<CollisionBox>,

    // Render Data
    animation_data: Vec<Animation>,
}

impl Character {
    pub fn from_config<'a>(texture_creator: &'a TextureCreator<WindowContext>, global_textures: &mut Vec<Texture<'a>>, config: &str) -> Result<Self, String> {
        deserialize(texture_creator, global_textures, config)
    }

    pub fn render(&self, canvas: &mut Canvas<Window>, global_textures: &Vec<Texture>) -> Result<(), sdl3::Error> {
        let (texture, src) = self.animation_data[self.current_state].get_frame(self.current_frame, global_textures);
        canvas.copy(texture, src, FRect::new(self.pos.x, self.pos.y, src.w, src.h))?;
        Ok(())
    }
}