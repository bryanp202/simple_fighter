use sdl3::render::{FPoint, FRect};

use crate::game::Side;

#[derive(Clone, Copy, Debug)]
pub enum BlockType {
    Low,
    Mid,
    High,
}

#[derive(Clone, Debug)]
pub struct HitBox {
    pos: FRect,
    dmg: f32,
    hit_stun: u32,
    block_stun: u32,
    cancel_window: usize,
    block_type: BlockType,
}

impl HitBox {
    pub fn new(
        pos: FRect,
        dmg: f32,
        block_stun: u32,
        hit_stun: u32,
        cancel_window: usize,
        block_type: BlockType,
    ) -> Self {
        Self {
            pos,
            dmg,
            block_stun,
            hit_stun,
            cancel_window,
            block_type,
        }
    }

    pub fn on_side(&self, side: Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect {
                x: self.pos.x + offset.x,
                y: self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
            Side::Right => FRect {
                x: -self.pos.x + offset.x - self.pos.w,
                y: self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
        }
    }

    pub fn dmg(&self) -> f32 {
        self.dmg
    }

    pub fn block_stun(&self) -> usize {
        self.block_stun as usize
    }

    pub fn hit_stun(&self) -> usize {
        self.hit_stun as usize
    }

    pub fn cancel_window(&self) -> usize {
        self.cancel_window
    }

    pub fn block_type(&self) -> BlockType {
        self.block_type
    }
}

pub struct HurtBox {
    pos: FRect,
}

impl HurtBox {
    pub fn new(pos: FRect) -> Self {
        Self { pos }
    }

    pub fn on_side(&self, side: Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect {
                x: self.pos.x + offset.x,
                y: self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
            Side::Right => FRect {
                x: -self.pos.x + offset.x - self.pos.w,
                y: self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
        }
    }
}

pub struct CollisionBox {
    pos: FRect,
}

impl CollisionBox {
    pub fn new(pos: FRect) -> Self {
        Self { pos }
    }

    pub fn on_side(&self, side: Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect {
                x: self.pos.x + offset.x,
                y: self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
            Side::Right => FRect {
                x: -self.pos.x + offset.x - self.pos.w,
                y: self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
        }
    }
}
