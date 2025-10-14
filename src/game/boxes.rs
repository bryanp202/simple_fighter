use sdl3::render::{FPoint, FRect};

#[derive(Debug)]
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

    pub fn pos_with_offset(&self, offset: FPoint) -> FRect {
        FRect { x: self.pos.x + offset.x, y: self.pos.y + offset.y - self.pos.h, w: self.pos.w, h: self.pos.h }
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

    pub fn pos_with_offset(&self, offset: FPoint) -> FRect {
        FRect { x: self.pos.x + offset.x, y: self.pos.y + offset.y - self.pos.h, w: self.pos.w, h: self.pos.h }
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

    pub fn pos_with_offset(&self, offset: FPoint) -> FRect {
        FRect { x: self.pos.x + offset.x, y: self.pos.y + offset.y - self.pos.h, w: self.pos.w, h: self.pos.h }
    }
}