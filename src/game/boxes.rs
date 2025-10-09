use sdl3::render::FRect;

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
}