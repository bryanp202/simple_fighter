use sdl3::{
    render::{FPoint, FRect, Texture, TextureCreator},
    video::WindowContext,
};
use serde::Deserialize;

use crate::game::{
    Side,
    character::StateFlags,
    render::{
        animation::{Animation, AnimationLayout},
        load_texture,
    },
};

mod character;
mod game;

pub use game::deserialize;

#[derive(Deserialize, Clone, Copy)]
#[serde(tag = "type")]
enum SideJson {
    Left,
    Right,
}

impl SideJson {
    fn to_side(self) -> Side {
        match self {
            Self::Left => Side::Left,
            Self::Right => Side::Right,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
struct FPointJson {
    x: f32,
    y: f32,
}

impl FPointJson {
    fn to_fpoint(self) -> FPoint {
        FPoint::new(self.x, self.y)
    }
}

#[derive(Deserialize, Clone, Copy)]
struct RectJson {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl RectJson {
    fn to_frect(self) -> FRect {
        FRect::new(self.x - self.w / 2.0, self.y + self.h / 2.0, self.w, self.h)
    }
}

#[derive(Deserialize)]
struct TextureJson {
    texture_path: String,
}

impl TextureJson {
    pub fn make_texture<'a>(
        &self,
        texture_creator: &'a TextureCreator<WindowContext>,
        global_textures: &mut Vec<Texture<'a>>,
    ) -> Result<usize, String> {
        load_texture(texture_creator, global_textures, &self.texture_path)
    }
}

#[derive(Deserialize)]
struct AnimationJson {
    texture_path: String,
    layout: AnimationLayoutJson,
    frames: u32,
    w: u32,
    h: u32,
}

impl AnimationJson {
    pub fn make_animation<'a>(
        &self,
        texture_creator: &'a TextureCreator<WindowContext>,
        global_textures: &mut Vec<Texture<'a>>,
    ) -> Result<Animation, String> {
        Animation::load(
            texture_creator,
            global_textures,
            &self.texture_path,
            self.w,
            self.h,
            self.frames,
            self.layout.to_animation_layout(),
        )
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum AnimationLayoutJson {
    Horz,
    Vert,
}

impl AnimationLayoutJson {
    fn to_animation_layout(&self) -> AnimationLayout {
        match self {
            AnimationLayoutJson::Horz => AnimationLayout::Horizontal,
            AnimationLayoutJson::Vert => AnimationLayout::Vertical,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum FlagsJson {
    Airborne,
    CancelOnWhiff,
    LockSide,
    LowBlock,
    HighBlock,
}

impl FlagsJson {
    fn to_state_json(&self) -> StateFlags {
        match self {
            FlagsJson::Airborne => StateFlags::Airborne,
            FlagsJson::CancelOnWhiff => StateFlags::CancelOnWhiff,
            FlagsJson::LockSide => StateFlags::LockSide,
            FlagsJson::HighBlock => StateFlags::HighBlock,
            FlagsJson::LowBlock => StateFlags::LowBlock,
        }
    }
}
