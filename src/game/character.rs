use std::ops::Range;

use crate::game::{
    Side,
    boxes::{BlockType, CollisionBox, HitBox, HurtBox},
    input::{ButtonFlag, Inputs, RelativeDirection, RelativeMotion},
    physics::{friction_system, gravity_system, velocity_system},
    render::{
        Camera, animation::Animation, draw_collision_box_system, draw_hit_boxes_system,
        draw_hurt_boxes_system,
    },
    stage::Stage,
};
use bitflags::bitflags;
use sdl3::{
    render::{Canvas, FPoint, Texture},
    video::Window,
};

type StateIndex = usize;
const HIT_GRAVITY_MULT: f32 = 1.2;
const HIT_PUSH_BACK: f32 = -6.0;
const CHIP_DMG_PERCENTAGE: f32 = 0.1;
const COMBO_SCALE_PER_HIT: f32 = 0.1;
const MIN_COMBO_SCALING: f32 = 0.1;

pub struct StateData {
    // Cancel data
    cancel_window: Range<usize>,
    cancel_options: Range<usize>,
    // Boxes
    hit_boxes_start: usize,
    hurt_boxes_start: usize,
    // Behavior
    start_behaviors: StartBehavior,
    flags: StateFlags,
    end_behaviors: EndBehavior,

    // Physics
    collision: CollisionBox,

    // Render
    animation: Animation,
}

impl StateData {
    pub fn new(
        cancel_window: Range<usize>,
        cancel_options: Range<usize>,
        hit_boxes_start: usize,
        hurt_boxes_start: usize,
        start_behaviors: StartBehavior,
        flags: StateFlags,
        end_behaviors: EndBehavior,
        collision: CollisionBox,
        animation: Animation,
    ) -> Self {
        Self {
            cancel_window,
            cancel_options,
            hit_boxes_start,
            hurt_boxes_start,
            start_behaviors,
            flags,
            end_behaviors,
            collision,
            animation,
        }
    }
}

pub struct Context {
    name: String,
    // Init data
    max_hp: f32,
    start_side: Side,
    start_pos: FPoint,

    // Special cached states
    block_stun_state: StateIndex,
    ground_hit_state: StateIndex,
    launch_hit_state: StateIndex,

    // Run length stuff
    run_length_hit_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hitboxes index range
    run_length_hurt_boxes: Vec<(usize, Range<usize>)>, // Frames active, global hurtboxes index range
    run_length_cancel_options: Vec<StateIndex>,

    hit_box_data: Vec<HitBox>,
    hurt_box_data: Vec<HurtBox>,

    // Moves/states
    state_inputs: Vec<MoveInput>,
    states: Vec<StateData>,
}

impl Context {
    pub fn new(
        name: String,
        max_hp: f32,
        start_side: Side,
        start_pos: FPoint,
        block_stun_state: StateIndex,
        ground_hit_state: StateIndex,
        launch_hit_state: StateIndex,
        run_length_hit_boxes: Vec<(usize, Range<usize>)>,
        run_length_hurt_boxes: Vec<(usize, Range<usize>)>,
        run_length_cancel_options: Vec<StateIndex>,
        hit_box_data: Vec<HitBox>,
        hurt_box_data: Vec<HurtBox>,
        state_inputs: Vec<MoveInput>,
        states: Vec<StateData>,
    ) -> Self {
        Self {
            name,
            max_hp,
            start_side,
            start_pos,
            block_stun_state,
            ground_hit_state,
            launch_hit_state,

            run_length_hit_boxes,
            run_length_hurt_boxes,
            run_length_cancel_options,
            hit_box_data,
            hurt_box_data,

            state_inputs,
            states,
        }
    }
}

impl Context {
    fn active_hit_boxes(&self, current_state: StateIndex, mut current_frame: usize) -> &[HitBox] {
        let mut run_start = self.states[current_state].hit_boxes_start;

        loop {
            let (frames, range) = &self.run_length_hit_boxes[run_start];
            if current_frame < *frames {
                return &self.hit_box_data[range.clone()];
            }
            current_frame -= frames;
            run_start += 1;
        }
    }

    fn active_hurt_boxes(&self, current_state: StateIndex, mut current_frame: usize) -> &[HurtBox] {
        let mut run_start = self.states[current_state].hurt_boxes_start;

        loop {
            let (frames, range) = &self.run_length_hurt_boxes[run_start];
            if current_frame < *frames {
                return &self.hurt_box_data[range.clone()];
            }
            current_frame -= frames;
            run_start += 1;
        }
    }

    pub fn start_pos(&self) -> FPoint {
        self.start_pos
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct State {
    current_state: StateIndex,
    current_frame: usize,
    hp: f32,
    side: Side,
    pos: FPoint,
    vel: FPoint,
    friction_vel: FPoint,
    gravity_mult: f32,
    hit_connected: bool,
    stun: usize,
    combo_scaling: f32,
}

impl State {
    pub fn new(hp: f32, pos: FPoint, side: Side) -> Self {
        Self {
            hp,
            pos,
            side,
            current_state: 0,
            current_frame: 0,
            vel: FPoint::new(0.0, 0.0),
            friction_vel: FPoint::new(0.0, 0.0),
            gravity_mult: 1.0,
            hit_connected: false,
            stun: 0,
            combo_scaling: 1.0,
        }
    }

    pub fn serialize(&self, context: &Context, stage: &Stage) -> [f32; 37] {
        let mut data = [0.0; 37];

        // Normal floats
        data[0] = self.hp / context.max_hp;
        data[1] = self.pos.x / stage.width();
        data[2] = self.pos.y / stage.height();
        data[3] = self.vel.x / stage.width();
        data[4] = self.vel.y / stage.height();
        data[5] = self.friction_vel.x / stage.width();
        data[6] = self.friction_vel.y / stage.height();
        data[7] = self.gravity_mult;
        data[8] = self.combo_scaling;
        // Integers
        data[9] = ((self.current_frame + 1) as f32).ln() / 30.0f32.ln();
        data[10] = self.stun as f32 / 60.0;
        // bools / enums
        data[11] = (self.side == Side::Left) as usize as f32;
        data[12 + self.current_state] = 1.0;

        data
    }

    pub fn combo_scaling(&self) -> f32 {
        self.combo_scaling
    }

    pub fn state_update(&mut self, inputs: &Inputs, context: &Context) {
        match self.side {
            Side::Left => {
                self.check_transitions(
                    context,
                    inputs.dir().on_left_side(),
                    &inputs
                        .move_buf()
                        .iter()
                        .map(|(motion, buttons)| (motion.on_left_side(), *buttons)),
                );
            }
            Side::Right => {
                self.check_transitions(
                    context,
                    inputs.dir().on_right_side(),
                    &inputs
                        .move_buf()
                        .iter()
                        .map(|(motion, buttons)| (motion.on_right_side(), *buttons)),
                );
            }
        }
    }

    pub fn movement_update(&mut self, context: &Context) {
        self.pos = velocity_system(self.pos, self.vel_on_side());

        self.friction_vel = friction_system(self.friction_vel);

        if context.states[self.current_state]
            .flags
            .contains(StateFlags::Airborne)
        {
            let (new_pos, new_vel, grounded) =
                gravity_system(self.pos, self.vel, self.gravity_mult);
            self.pos = new_pos;
            self.vel = new_vel;
            if grounded {
                self.friction_vel.y = 0.0;
                self.ground(context);
            }
        }
    }

    pub fn render(
        &self,
        canvas: &mut Canvas<Window>,
        camera: &Camera,
        global_textures: &[Texture],
        context: &Context,
    ) -> Result<(), sdl3::Error> {
        let animation = &context.states[self.current_state].animation;
        camera.render_animation_on_side(
            canvas,
            global_textures,
            self.pos,
            animation,
            self.current_frame,
            self.side,
        )?;

        if cfg!(feature = "debug") {
            canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
            let collision_box = self.get_collision_box(context);
            draw_collision_box_system(canvas, camera, self.side, self.pos, collision_box)?;

            let hitboxes = context.active_hit_boxes(self.current_state, self.current_frame);
            draw_hit_boxes_system(canvas, camera, self.side, self.pos, hitboxes)?;

            let hurtboxes = self.get_hurt_boxes(context);
            draw_hurt_boxes_system(canvas, camera, self.side, self.pos, hurtboxes)?;

            canvas.set_blend_mode(sdl3::render::BlendMode::None);
        }

        Ok(())
    }
}

impl State {
    pub fn reset(&mut self, context: &Context) {
        *self = State::new(context.max_hp, context.start_pos, context.start_side);
    }

    pub fn reset_to(&mut self, context: &Context, pos: FPoint, side: Side) {
        *self = State::new(context.max_hp, pos, side)
    }

    pub fn advance_frame(&mut self) {
        self.current_frame += 1;
    }

    pub fn pos(&self) -> FPoint {
        self.pos
    }

    pub fn set_pos(&mut self, new_pos: FPoint) {
        self.pos = new_pos;
    }

    pub fn side(&self) -> Side {
        self.side
    }

    // Returns the percentage of HP relative to max HP left
    pub fn hp_per(&self, context: &Context) -> f32 {
        self.hp / context.max_hp
    }

    pub fn set_side(&mut self, context: &Context, new_side: Side) {
        if !context.states[self.current_state]
            .flags
            .contains(StateFlags::LockSide)
        {
            self.side = new_side;
        }
    }

    pub fn get_collision_box<'a>(&self, context: &'a Context) -> &'a CollisionBox {
        &context.states[self.current_state].collision
    }

    pub fn get_hit_boxes<'a>(&self, context: &'a Context) -> &'a [HitBox] {
        if self.hit_connected {
            &context.hit_box_data[0..0]
        } else {
            context.active_hit_boxes(self.current_state, self.current_frame)
        }
    }

    pub fn get_hurt_boxes<'a>(&self, context: &'a Context) -> &'a [HurtBox] {
        context.active_hurt_boxes(self.current_state, self.current_frame)
    }

    pub fn receive_hit(&mut self, context: &Context, hit: &HitBox) -> bool {
        let blocking_flag = match hit.block_type() {
            BlockType::Low => StateFlags::LowBlock,
            BlockType::Mid => StateFlags::LowBlock | StateFlags::HighBlock,
            BlockType::High => StateFlags::HighBlock,
        };
        let blocking = context.states[self.current_state]
            .flags
            .intersects(blocking_flag);

        let dmg = if blocking {
            self.set_block_stun_state(context, hit.block_stun());
            hit.dmg() * CHIP_DMG_PERCENTAGE
        } else {
            self.combo_scaling = (self.combo_scaling - COMBO_SCALE_PER_HIT).max(MIN_COMBO_SCALING);
            self.set_hit_state(context, hit.hit_stun());
            hit.dmg() * self.combo_scaling
        };
        self.hp = (self.hp - dmg).max(0.0);

        blocking
    }

    pub fn successful_hit(&mut self, context: &Context, _hit: &HitBox, _blocked: bool) {
        if !context.states[self.current_state]
            .flags
            .contains(StateFlags::Airborne)
        {
            self.friction_vel.x += HIT_PUSH_BACK;
        }
        self.hit_connected = true;
    }
}

impl State {
    fn check_transitions<T>(&mut self, context: &Context, dir: RelativeDirection, move_iter: &T)
    where
        T: Iterator<Item = (RelativeMotion, ButtonFlag)> + Clone,
    {
        self.check_state_end(context);
        self.check_cancels(context, dir, move_iter);
    }

    fn check_state_end(&mut self, context: &Context) {
        match context.states[self.current_state].end_behaviors {
            EndBehavior::Endless => {}
            EndBehavior::OnStunEndToStateY {
                y: transition_state,
            } => {
                if self.current_frame >= self.stun {
                    self.enter_state(context, transition_state);
                }
            }
            EndBehavior::OnFrameXToStateY {
                x: end_frame,
                y: transition_state,
            } => {
                if self.current_frame >= end_frame {
                    self.enter_state(context, transition_state);
                    self.combo_scaling = 1.0;
                }
            }
            EndBehavior::OnGroundedToStateY { .. } => {}
        }
    }

    fn check_cancels<T>(&mut self, context: &Context, dir: RelativeDirection, move_iter: &T)
    where
        T: Iterator<Item = (RelativeMotion, ButtonFlag)> + Clone,
    {
        // Check if not in cancel window
        if !self.in_cancel_window(context) {
            return;
        }

        let cancel_options_range = context.states[self.current_state].cancel_options.clone();
        let cancel_options = &context.run_length_cancel_options[cancel_options_range];
        for i in cancel_options {
            let cancel_option = &context.state_inputs[*i];
            if !cancel_option.dir.matches_or_is_none(dir) {
                continue;
            }

            let maybe_index = move_iter.clone().position(|(buf_motion, buf_buttons)| {
                buf_motion.contains(cancel_option.motion)
                    && buf_buttons.contains(cancel_option.button)
            });

            if maybe_index.is_some() {
                self.enter_state(context, *i);
                break;
            }
        }
    }

    fn in_cancel_window(&self, context: &Context) -> bool {
        context.states[self.current_state]
            .cancel_window
            .contains(&self.current_frame)
            && (self.hit_connected
                || context.states[self.current_state]
                    .flags
                    .contains(StateFlags::CancelOnWhiff))
    }

    fn enter_state(&mut self, context: &Context, new_state: StateIndex) {
        self.current_state = new_state;
        self.current_frame = 0;
        self.hit_connected = false;
        match context.states[new_state].start_behaviors {
            StartBehavior::None => {}
            StartBehavior::SetVel { x, y } => {
                self.vel = FPoint::new(x, y);
            }
            StartBehavior::AddFrictionVel { x, y } => {
                self.vel = FPoint::new(0.0, 0.0);
                self.friction_vel = FPoint::new(self.friction_vel.x + x, self.friction_vel.y + y);
            }
        }
    }

    fn vel_on_side(&self) -> FPoint {
        match self.side {
            Side::Left => FPoint::new(
                self.vel.x + self.friction_vel.x,
                self.vel.y + self.friction_vel.y,
            ),
            Side::Right => FPoint::new(
                -(self.vel.x + self.friction_vel.x),
                self.vel.y + self.friction_vel.y,
            ),
        }
    }

    fn ground(&mut self, context: &Context) {
        if let EndBehavior::OnGroundedToStateY { y } =
            context.states[self.current_state].end_behaviors
        {
            self.enter_state(context, y);
            self.gravity_mult = 1.0;
        }
    }

    fn set_block_stun_state(&mut self, context: &Context, hit_stun: usize) {
        self.stun = hit_stun;
        self.enter_state(context, context.block_stun_state);
    }

    fn set_hit_state(&mut self, context: &Context, hit_stun: usize) {
        let should_launch = self.pos.y != 0.0;
        if should_launch
            || self.current_state == context.launch_hit_state
            || hit_stun == u32::MAX as usize
        {
            self.enter_state(context, context.launch_hit_state);
            self.gravity_mult *= HIT_GRAVITY_MULT;
        } else {
            self.stun = hit_stun;
            self.enter_state(context, context.ground_hit_state);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MoveInput {
    button: ButtonFlag,
    motion: RelativeMotion,
    dir: RelativeDirection,
}

impl MoveInput {
    pub fn new(button: ButtonFlag, motion: RelativeMotion, dir: RelativeDirection) -> Self {
        Self {
            button,
            motion,
            dir,
        }
    }
}

#[derive(Debug)]
pub enum StartBehavior {
    None,
    SetVel { x: f32, y: f32 },
    AddFrictionVel { x: f32, y: f32 },
}

#[derive(Debug)]
pub enum EndBehavior {
    Endless,
    OnStunEndToStateY { y: StateIndex },
    OnFrameXToStateY { x: usize, y: StateIndex },
    OnGroundedToStateY { y: StateIndex },
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct StateFlags: u32 {
        const NONE = 0;
        const Airborne =      0b0000_0001;
        const CancelOnWhiff = 0b0000_0010;
        const LockSide =      0b0000_0100;
        const LowBlock =      0b0000_1000;
        const HighBlock =     0b0001_0000;
    }
}
