mod deserialize;
mod state;

use crate::game::{
    Side,
    boxes::{CollisionBox, HitBox, HurtBox},
    character::{deserialize::deserialize, state::StateData},
    input::Inputs,
    physics::{gravity_system, velocity_system},
    projectile::Projectile,
    render::{
        animation::Animation, draw_collision_box_system, draw_hit_boxes_system,
        draw_hurt_boxes_system, to_screen_pos,
    },
};
use sdl3::{
    render::{Canvas, FPoint, FRect, Texture, TextureCreator},
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
        self.state_data.update(&self.states, &inputs);
    }

    pub fn movement_update(&mut self) {
        self.state_data.advance_frame();
        self.pos = velocity_system(&self.pos, &self.state_data.vel());

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
        global_textures: &Vec<Texture>,
    ) -> Result<(), sdl3::Error> {
        let current_state = self.state_data.current_state();
        let current_frame = self.state_data.current_frame();

        let screen_pos = to_screen_pos(&self.pos);

        let flip_horz = match self.state_data.get_side() {
            Side::Left => false,
            Side::Right => true,
        };
        let (texture, src) =
            self.animation_data[current_state].get_frame_cycle(current_frame, global_textures);
        // Sprite is rendered with the character pos in the center
        let dst = FRect::new(
            screen_pos.x - src.w / 2.0,
            screen_pos.y - src.h / 2.0,
            src.w,
            src.h,
        );
        canvas.copy_ex(texture, src, dst, 0.0, None, flip_horz, false)?;

        if cfg!(feature = "debug") {
            canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
            let side = self.get_side();
            let collision_box = self.get_collision_box();
            draw_collision_box_system(canvas, side, screen_pos, collision_box)?;

            let hitboxes = self.get_hit_boxes_debug();
            draw_hit_boxes_system(canvas, side, screen_pos, hitboxes)?;

            let hurtboxes = self.get_hurt_boxes();
            draw_hurt_boxes_system(canvas, side, screen_pos, hurtboxes)?;

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
    pub fn successful_hit(&mut self, hit: &HitBox) {
        self.state_data.on_hit_connect();
    }

    pub fn receive_hit(&mut self, hit: &HitBox) {
        self.current_hp -= hit.dmg();
        self.state_data.set_launch_hit_state(&self.states);
    }
}

impl Character {
    fn get_hit_boxes_debug(&self) -> &[HitBox] {
        let hit_box_range = self.states.hit_box_range_no_check(&self.state_data);
        let hurtboxes = &self.hit_box_data[hit_box_range];
        hurtboxes
    }
}
