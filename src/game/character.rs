mod state;
mod deserialize;

use crate::{game::{boxes::{CollisionBox, HitBox, HurtBox}, character::{deserialize::deserialize, state::StateData}, input::Inputs, projectile::Projectile, render::{animation::Animation, draw_collision_box_system, draw_hit_boxes_system, draw_hurt_boxes_system}}, DEFAULT_SCREEN_HEIGHT, DEFAULT_SCREEN_WIDTH};
use sdl3::{render::{Canvas, FPoint, FRect, Texture, TextureCreator}, video::{Window, WindowContext}};
use state::States;

pub struct Character {
    name: String,
    hp: f32,
    pos: FPoint,
    
    // State Data
    states: States,
    state_data: StateData,
    
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

    pub fn update(&mut self, inputs: &Inputs) {
        self.states.update(&mut self.state_data, inputs);
        let FPoint { x, y } = self.state_data.vel();

        self.pos.x = (self.pos.x + x).clamp(-(DEFAULT_SCREEN_WIDTH as f32) / 2.0, DEFAULT_SCREEN_WIDTH as f32 / 2.0);
        self.pos.y += y;

        if self.state_data.is_airborne(&self.states) {
            self.state_data.set_vel(FPoint::new(x, y - 0.4));
            if self.pos.y <= 0.0 {
                self.state_data.ground(&self.states);
                self.pos.y = 0.0;
            }
        }
    }

    pub fn render(&self, canvas: &mut Canvas<Window>, global_textures: &Vec<Texture>) -> Result<(), sdl3::Error> {
        let current_state = self.state_data.current_state();
        let current_frame = self.state_data.current_frame();

        let game_center = (DEFAULT_SCREEN_WIDTH as f32 / 2.0, DEFAULT_SCREEN_HEIGHT as f32);
        let shifted_pos = FPoint::new(game_center.0 + self.pos.x, game_center.1 - self.pos.y);
    
        let (texture, src) = self.animation_data[current_state]
            .get_frame_cycle(current_frame, global_textures);
        canvas.copy(texture, src, FRect::new(shifted_pos.x, shifted_pos.y - src.h, src.w, src.h))?;

        canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
        let collision_box = &self.collision_box_data[current_state];
        draw_collision_box_system(canvas, shifted_pos, collision_box)?;

        let hit_box_range = self.states.hit_box_range(current_state, current_frame);
        let hitboxes = &self.hit_box_data[hit_box_range];
        draw_hit_boxes_system(canvas, shifted_pos, hitboxes)?;

        let hurt_box_range = self.states.hurt_box_range(current_state, current_frame);
        let hurtboxes = &self.hurt_box_data[hurt_box_range];
        draw_hurt_boxes_system(canvas, shifted_pos, hurtboxes)?;


        canvas.set_blend_mode(sdl3::render::BlendMode::None);

        Ok(())
    }
}