use sdl3::{
    render::{Canvas, FPoint, Texture, TextureCreator},
    video::{Window, WindowContext},
};

use crate::game::render::load_texture;

const STATIC_LAYERS: &[&str] = &[
    "./resources/stage1/1.png",
    "./resources/stage1/2.png",
    "./resources/stage1/3.png",
    "./resources/stage1/4.png",
    "./resources/stage1/5.png",
    "./resources/stage1/6.png",
    "./resources/stage1/7.png",
    "./resources/stage1/8.png",
];

pub struct Stage {
    layers: Vec<usize>,
    width: f32,
    height: f32,
}

impl Stage {
    pub fn init<'a>(
        texture_creator: &'a TextureCreator<WindowContext>,
        global_textures: &mut Vec<Texture<'a>>,
    ) -> Result<Stage, String> {
        let mut layers = Vec::new();

        for layer in STATIC_LAYERS {
            let texture_index = load_texture(texture_creator, global_textures, layer)?;
            layers.push(texture_index);
        }

        Ok(Self {
            layers,
            width: 420.0,
            height: 600.0,
        })
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn render(
        &self,
        canvas: &mut Canvas<Window>,
        global_textures: &[Texture],
    ) -> Result<(), sdl3::Error> {
        for &layer in &self.layers {
            canvas.copy(&global_textures[layer], None, None)?;
        }

        Ok(())
    }

    pub fn bind_pos(&self, pos: FPoint) -> FPoint {
        FPoint::new(pos.x.clamp(-self.width, self.width), pos.y)
    }
}
