mod state;
mod deserialize;

use crate::{game::{boxes::{CollisionBox, HitBox, HurtBox}, character::{deserialize::deserialize, state::StateData}, input::Inputs, physics::{gravity_system, velocity_system}, projectile::Projectile, render::{animation::Animation, draw_collision_box_system, draw_hit_boxes_system, draw_hurt_boxes_system}}, DEFAULT_SCREEN_HEIGHT, DEFAULT_SCREEN_WIDTH};
use sdl3::{render::{Canvas, FPoint, FRect, Texture, TextureCreator}, video::{Window, WindowContext}};
use state::States;

pub struct Character {
    name: String,
    hp: f32,
    current_hp: f32,
    pos: FPoint,
    facing_right: bool,
    
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
    pub fn from_config<'a>(
        texture_creator: &'a TextureCreator<WindowContext>,
        global_textures: &mut Vec<Texture<'a>>,
        config: &str,
        pos: FPoint,
        facing_right: bool,
    ) -> Result<Self, String> {
        deserialize(texture_creator, global_textures, config, pos, facing_right)
    }

    pub fn absolute_pos(&self) -> FPoint {
        let game_center = (DEFAULT_SCREEN_WIDTH as f32 / 2.0, DEFAULT_SCREEN_HEIGHT as f32);
        FPoint::new(game_center.0 + self.pos.x, game_center.1 - self.pos.y + 32.0)
    }

    pub fn update(&mut self, inputs: &Inputs) {
        if self.facing_right {
            self.state_data.update(
                &self.states,
                inputs.dir().on_left_side(),
                inputs.move_buf().iter().map(|(motion, buttons)| (motion.on_left_side(), *buttons))
            );
            self.pos = velocity_system(&self.pos, &self.state_data.vel());
        } else {
            self.state_data.update(
                &self.states,
                inputs.dir().on_right_side(),
                inputs.move_buf().iter().map(|(motion, buttons)| (motion.on_right_side(), *buttons))
            );
            let mut vel = self.state_data.vel();
            vel.x = -vel.x;
            self.pos = velocity_system(&self.pos, &vel);
        }

        if self.state_data.is_airborne(&self.states) {
            let (new_pos, new_vel, grounded) = gravity_system(
                &self.pos,
                &self.state_data.vel(),
                self.state_data.gravity_mult()
            );
            self.pos = new_pos;
            self.state_data.set_vel(new_vel);
            if grounded {
                self.state_data.ground(&self.states);
            }
        }
    }

    pub fn render(&self, canvas: &mut Canvas<Window>, global_textures: &Vec<Texture>) -> Result<(), sdl3::Error> {
        let current_state = self.state_data.current_state();
        let current_frame = self.state_data.current_frame();

        let shifted_pos = self.absolute_pos();
    
        let (texture, src) = self.animation_data[current_state]
            .get_frame_cycle(current_frame, global_textures);
        canvas.copy_ex(texture, src, FRect::new(shifted_pos.x, shifted_pos.y - src.h, src.w, src.h),
            0.0, None, !self.facing_right, false)?;

        if cfg!(feature = "debug") {
            canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
            let collision_box = self.get_collision_box();
            draw_collision_box_system(canvas, shifted_pos, collision_box)?;

            let hitboxes = self.get_hit_boxes();
            draw_hit_boxes_system(canvas, shifted_pos, hitboxes)?;

            let hurtboxes = self.get_hurt_boxes();
            draw_hurt_boxes_system(canvas, shifted_pos, hurtboxes)?;

            canvas.set_blend_mode(sdl3::render::BlendMode::None);
            println!("{}: hp: {}", self.name, self.hp)
        }

        Ok(())
    }

    pub fn get_hit_boxes(&self) -> &[HitBox] {
        let hit_box_range = self.states.hit_box_range(&self.state_data);
        let hitboxes = &self.hit_box_data[hit_box_range];
        hitboxes
    }

    pub fn get_hurt_boxes(&self) -> &[HurtBox] {
        let hurt_box_range = self.states.hurt_box_range(&self.state_data);
        let hurtboxes = &self.hurt_box_data[hurt_box_range];
        hurtboxes
    }

    pub fn get_collision_box(&self) -> &CollisionBox {
        &self.collision_box_data[self.state_data.current_state()]
    }

    pub fn reset(&mut self, pos: FPoint) {
        self.pos = pos;
        self.current_hp = self.hp; 
        self.state_data = StateData::default();
    }

    pub fn current_hp(&self) -> f32 {
        self.current_hp
    }

    pub fn max_hp(&self) -> f32 {
        self.hp
    }
}

impl Character {
    pub fn successful_hit(&mut self, hit: &HitBox) {
        self.state_data.on_hit_connect();
    } 

    pub fn receive_hit(&mut self, hit: &HitBox) {
        self.current_hp -= hit.dmg();
        self.state_data.set_launch_hit_state(&self.states);
    }
}