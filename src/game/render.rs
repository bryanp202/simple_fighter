use image::DynamicImage;
use sdl3::{pixels::{FColor, PixelFormat}, rect::Rect, render::{Canvas, FPoint, Texture, TextureCreator}, sys::pixels::SDL_PIXELFORMAT_ABGR8888, video::{Window, WindowContext}};

use crate::game::{boxes::{CollisionBox, HitBox, HurtBox}, render::animation::AnimationLayout};

pub mod animation;

pub fn draw_hit_boxes_system(canvas: &mut Canvas<Window>, offset: FPoint, hitboxes: &[HitBox]) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(1.0, 0.0, 0.0, 0.5));
    for hitbox in hitboxes {
        canvas.fill_rect(hitbox.pos_with_offset(offset))?;
    }
    Ok(())
}

pub fn draw_hurt_boxes_system(canvas: &mut Canvas<Window>, offset: FPoint, hurtboxes: &[HurtBox]) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(0.0, 1.0, 0.0, 0.5));
    for hurtbox in hurtboxes {
        canvas.fill_rect(hurtbox.pos_with_offset(offset))?;
    }
    Ok(())
}

pub fn draw_collision_box_system(canvas: &mut Canvas<Window>, offset: FPoint, collision_box: &CollisionBox) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(1.0, 1.0, 1.0, 0.5));
    canvas.fill_rect(collision_box.pos_with_offset(offset))?;
    Ok(())
}

fn open_img(file_path: &str) -> Result<DynamicImage, String> {
    let file = std::fs::File::open(file_path).map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
    let reader = std::io::BufReader::new(file);
    let img = image::ImageReader::new(reader)
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();

    if cfg!(feature = "debug") {
        println!("Loaded image: {}", file_path);
    }

    Ok(img)
}

pub fn load_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    global_textures: &mut Vec<Texture<'a>>,
    file_path: &str,
) -> Result<usize, String> {
    let img = open_img(file_path)?;

    let mut texture = texture_creator.create_texture_streaming(
        unsafe {PixelFormat::from_ll(SDL_PIXELFORMAT_ABGR8888)},
        img.width(),
        img.height(),
    ).map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;

    texture.update(None, &img.to_rgba8(), 4 * img.width() as usize)
        .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;

    global_textures.push(texture);

    Ok(global_textures.len() - 1)
}

pub fn load_animation<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    global_textures: &mut Vec<Texture<'a>>,
    file_path: &str,
    width: u32, height: u32, frames: u32, layout: AnimationLayout,
) -> Result<usize, String> {
    let img = open_img(file_path)?;

    let mut texture = texture_creator.create_texture_streaming(
        unsafe {PixelFormat::from_ll(SDL_PIXELFORMAT_ABGR8888)},
        width,
        height * frames,
    ).map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;

    match layout {
        AnimationLayout::VERTICAL => {
            let frames_rect = Rect::new(0, 0, width, height * frames);
            let frames = img.crop_imm(frames_rect.x as u32, frames_rect.y as u32, frames_rect.width(), frames_rect.height());
            texture.update(frames_rect, &frames.to_rgba8(), 4 * frames.width() as usize)
                .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
        },
        AnimationLayout::HORIZONTAL => {
            for frame in 0..frames {
                let frame_rect = Rect::new((frame * width) as i32, 0, width, height);
                let frame = img.crop_imm(frame_rect.x as u32, frame_rect.y as u32, frame_rect.width(), frame_rect.height());
                let texture_frame = Rect::new(frame_rect.y, frame_rect.x, frame_rect.width(), frame_rect.height());
                texture.update(texture_frame, &frame.to_rgba8(), 4 * frame.width() as usize)
                    .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
            }
        },
    }
    global_textures.push(texture);

    Ok(global_textures.len() - 1)
}