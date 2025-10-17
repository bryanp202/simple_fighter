use sdl3::render::{FPoint, FRect};

use crate::game::Side;

#[derive(Clone, Debug)]
pub struct HitBox {
    pos: FRect,
    dmg: f32,
    hit_stun: usize,
    cancel_window: usize,
}

impl HitBox {
    pub fn new(pos: FRect, dmg: f32, hit_stun: usize, cancel_window: usize) -> Self {
        Self { pos, dmg, hit_stun, cancel_window }
    }

    pub fn pos(&self) -> FRect {
        self.pos
    }

    pub fn on_side(&self, side: &Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect { x: self.pos.x + offset.x, y: self.pos.y + offset.y, w: self.pos.w, h: self.pos.h },
            Side::Right =>  FRect { x: -self.pos.x + offset.x - self.pos.w, y: self.pos.y + offset.y, w: self.pos.w, h: self.pos.h },
        }
    }

    pub fn on_side_screen(&self, side: &Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect {
                x: self.pos.x + offset.x,
                y: -self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h
            },
            Side::Right => FRect {
                x: -self.pos.x + offset.x - self.pos.w,
                y: -self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
        }
    }

    pub fn dmg(&self) -> f32 {
        self.dmg
    }

    pub fn hit_stun(&self) -> usize {
        self.hit_stun
    }

    pub fn cancel_window(&self) -> usize {
        self.cancel_window
    }
}

pub struct HurtBox {
    pos: FRect,
}

impl HurtBox {
    pub fn new(pos: FRect) -> Self {
        Self { pos }
    }

    pub fn pos(&self) -> FRect {
        self.pos
    }

    pub fn on_side(&self, side: &Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect { x: self.pos.x + offset.x, y: self.pos.y + offset.y, w: self.pos.w, h: self.pos.h },
            Side::Right =>  FRect { x: -self.pos.x + offset.x - self.pos.w, y: self.pos.y + offset.y, w: self.pos.w, h: self.pos.h },
        }
    }

    pub fn on_side_screen(&self, side: &Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect {
                x: self.pos.x + offset.x,
                y: -self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h
            },
            Side::Right => FRect {
                x: -self.pos.x + offset.x - self.pos.w,
                y: -self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
        }
    }
}

pub struct CollisionBox {
    pos: FRect
}

impl CollisionBox {
    pub fn new(pos: FRect) -> Self {
        Self { pos }
    }

    pub fn pos(&self) -> FRect {
        self.pos
    }

    pub fn on_side(&self, side: &Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect { x: self.pos.x + offset.x, y: self.pos.y + offset.y, w: self.pos.w, h: self.pos.h },
            Side::Right =>  FRect { x: -self.pos.x + offset.x - self.pos.w, y: self.pos.y + offset.y, w: self.pos.w, h: self.pos.h },
        }
    }

    pub fn on_side_screen(&self, side: &Side, offset: FPoint) -> FRect {
        match side {
            Side::Left => FRect {
                x: self.pos.x + offset.x,
                y: -self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h
            },
            Side::Right => FRect {
                x: -self.pos.x + offset.x - self.pos.w,
                y: -self.pos.y + offset.y,
                w: self.pos.w,
                h: self.pos.h,
            },
        }
    }
}