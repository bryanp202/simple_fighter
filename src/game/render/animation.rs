use sdl3::{
    render::{FRect, Texture, TextureCreator},
    video::WindowContext,
};

use crate::game::render::load_animation;

pub enum AnimationLayout {
    HORIZONTAL,
    VERTICAL,
}

/// Animation frames are stored vertically
pub struct Animation {
    texture_index: usize,
    frames: usize,
    frame_w: f32,
    frame_h: f32,
}

impl Animation {
    pub fn new(texture_index: usize, frames: usize, frame_w: f32, frame_h: f32) -> Animation {
        Self {
            texture_index,
            frames,
            frame_w,
            frame_h,
        }
    }

    pub fn load<'a>(
        texture_creator: &'a TextureCreator<WindowContext>,
        global_textures: &mut Vec<Texture<'a>>,
        file_path: &str,
        width: u32,
        height: u32,
        frames: u32,
        layout: AnimationLayout,
    ) -> Result<Self, String> {
        let texture_index = load_animation(
            texture_creator,
            global_textures,
            file_path,
            width,
            height,
            frames,
            layout,
        )?;

        Ok(Self::new(
            texture_index,
            frames as usize,
            width as f32,
            height as f32,
        ))
    }

    // pub fn width(&self) -> f32 {
    //     self.frame_w
    // }

    // pub fn height(&self) -> f32 {
    //     self.frame_h
    // }

    pub fn get_frame_count(&self) -> usize {
        self.frames
    }

    pub fn get_frame<'r>(&self, frame: usize, textures: &'r [Texture]) -> (&'r Texture<'r>, FRect) {
        let frame = frame.min(self.frames - 1);
        let src_rect = FRect::new(0.0, frame as f32 * self.frame_h, self.frame_w, self.frame_h);
        (&textures[self.texture_index], src_rect)
    }

    pub fn get_frame_cycle<'r>(
        &self,
        frame: usize,
        textures: &'r [Texture],
    ) -> (&'r Texture<'r>, FRect) {
        let frame = frame % self.frames;
        let src_rect = FRect::new(0.0, frame as f32 * self.frame_h, self.frame_w, self.frame_h);
        (&textures[self.texture_index], src_rect)
    }
}
