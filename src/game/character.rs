mod deserialize;
mod state;

use crate::game::{
    boxes::{BlockType, CollisionBox, HitBox, HurtBox}, character::{deserialize::deserialize, state::StateData}, input::Inputs, physics::{gravity_system, velocity_system}, projectile::Projectile, render::{
        animation::Animation, draw_collision_box_system, draw_hit_boxes_system,
        draw_hurt_boxes_system, Camera,
    }, Side
};
use sdl3::{
    render::{Canvas, FPoint, Texture, TextureCreator},
    video::{Window, WindowContext},
};
use state::States;

pub struct Character {
    name: String,
    current_hp: f32,
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

    // Init Data
    max_hp: f32,
    start_side: Side,
    start_pos: FPoint,
}

impl Character {
    pub fn from_config<'a>(
        texture_creator: &'a TextureCreator<WindowContext>,
        global_textures: &mut Vec<Texture<'a>>,
        config: &str,
        pos: FPoint,
        side: Side,
    ) -> Result<Self, String> {
        deserialize(texture_creator, global_textures, config, pos, side)
    }

    pub fn pos(&self) -> FPoint {
        self.pos
    }

    pub fn set_pos(&mut self, new_pos: FPoint) {
        self.pos = new_pos;
    }

    pub fn get_side(&self) -> &Side {
        self.state_data.get_side()
    }

    pub fn vel(&self) -> FPoint {
        self.state_data.vel()
    }

    pub fn update(&mut self, inputs: &Inputs) {
        match self.state_data.get_side() {
            Side::Left => {
                self.state_data.update(
                    &self.states,
                    inputs.dir().on_left_side(),
                    inputs.move_buf().iter().map(|(motion, buttons)| (motion.on_left_side(), *buttons))
                );
            },
            Side::Right => {
                self.state_data.update(
                    &self.states,
                    inputs.dir().on_right_side(),
                    inputs.move_buf().iter().map(|(motion, buttons)| (motion.on_right_side(), *buttons))
                );
            },
        }
    }

    pub fn advance_frame(&mut self) {
        self.state_data.advance_frame();
    }

    pub fn movement_update(&mut self) {
        self.pos = velocity_system(&self.pos, &self.state_data.vel_rel());
        
        self.state_data.update_friction();
        
        if self.state_data.is_airborne(&self.states) {
            let (new_pos, new_vel, grounded) = gravity_system(
                &self.pos,
                &self.state_data.vel(),
                self.state_data.gravity_mult(),
            );
            self.pos = new_pos;
            self.state_data.set_vel(new_vel);
            if grounded {
                self.state_data.ground(&self.states);
            }
        }
    }

    pub fn render(
        &self,
        canvas: &mut Canvas<Window>,
        camera: &Camera,
        global_textures: &Vec<Texture>,
    ) -> Result<(), sdl3::Error> {
        let current_state = self.state_data.current_state();
        let frame = self.state_data.current_frame();
        let animation = &self.animation_data[current_state];
        camera.render_animation(canvas, global_textures, &self.pos, animation, frame, self.get_side())?;

        if cfg!(feature = "debug") {
            canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
            let side = self.get_side();
            let collision_box = self.get_collision_box();
            draw_collision_box_system(canvas, camera, side, self.pos, collision_box)?;

            let hitboxes = self.get_hit_boxes_debug();
            draw_hit_boxes_system(canvas, camera, side, self.pos, hitboxes)?;

            let hurtboxes = self.get_hurt_boxes();
            draw_hurt_boxes_system(canvas, camera, side, self.pos, hurtboxes)?;

            canvas.set_blend_mode(sdl3::render::BlendMode::None);
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

    pub fn reset(&mut self) {
        self.pos = self.start_pos;
        self.current_hp = self.max_hp;
        self.state_data = StateData::new(self.start_side);
    }

    pub fn reset_to(&mut self, pos: FPoint) {
        self.pos = pos;
        self.current_hp = self.max_hp;
        self.state_data = StateData::new(self.start_side);
    }

    pub fn current_hp(&self) -> f32 {
        self.current_hp
    }

    pub fn max_hp(&self) -> f32 {
        self.max_hp
    }

    pub fn set_side(&mut self, side: Side) {
        self.state_data.set_side(&self.states, side);
    }
}

impl Character {
    pub fn successful_hit(&mut self, hit: &HitBox, blocked: bool) {
        self.state_data.on_hit_connect(&self.states, blocked);
    }

    /// Returns true if the hit was blocked
    pub fn receive_hit(&mut self, hit: &HitBox) -> bool {
        let blocking = match hit.block_type() {
            BlockType::Low => self.state_data.is_blocking_low(&self.states),
            BlockType::Mid => self.state_data.is_blocking_mid(&self.states),
            BlockType::High => self.state_data.is_blocking_high(&self.states),
        };

        let dmg = self.state_data.on_hit_receive(&self.states, &self.pos, hit, blocking);
        self.current_hp = (self.current_hp - dmg).max(0.0);

        blocking
    }
}

impl Character {
    fn get_hit_boxes_debug(&self) -> &[HitBox] {
        let hit_box_range = self.states.hit_box_range_no_check(&self.state_data);
        let hurtboxes = &self.hit_box_data[hit_box_range];
        hurtboxes
    }
}
