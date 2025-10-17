use sdl3::{render::{Canvas, FPoint, Texture, TextureCreator}, video::{Window, WindowContext}};

use crate::{game::render::load_texture, DEFAULT_SCREEN_WIDTH};

const STATIC_LAYERS: &[&str] = &[
    "./resources/stage1/Layer_0011_0.png", "./resources/stage1/Layer_0010_1.png", "./resources/stage1/Layer_0009_2.png",
    "./resources/stage1/Layer_0008_3.png", "./resources/stage1/Layer_0007_Lights.png", "./resources/stage1/Layer_0006_4.png",
    "./resources/stage1/Layer_0004_Lights.png", "./resources/stage1/Layer_0003_6.png", "./resources/stage1/Layer_0002_7.png",
    "./resources/stage1/Layer_0001_8.png", "./resources/stage1/Layer_0000_9.png",
];

pub struct Stage {
    layers: Vec<usize>,
    left_wall: f32,
    right_wall: f32,
}

impl Stage {
    pub fn init<'a>(texture_creator: &'a TextureCreator<WindowContext>, global_textures: &mut Vec<Texture<'a>>) -> Stage {
        let mut layers = Vec::new();

        for layer in STATIC_LAYERS {
            let texture_index = load_texture(texture_creator, global_textures, layer).unwrap();
            layers.push(texture_index);
        }
        
        Self {
            layers,
            left_wall: DEFAULT_SCREEN_WIDTH as f32 / -2.0 + 50.0,
            right_wall: DEFAULT_SCREEN_WIDTH as f32 / 2.0 - 50.0,
        }
    }

    pub fn render(&self, canvas: &mut Canvas<Window>, global_textures: &Vec<Texture>) -> Result<(), sdl3::Error> {
        for &layer in self.layers.iter() {
            canvas.copy(&global_textures[layer], None, None)?;
        }

        Ok(())
    }

    pub fn bind_pos(&self, pos: &FPoint) -> FPoint {
        FPoint::new(pos.x.clamp(self.left_wall, self.right_wall), pos.y)
    }
}