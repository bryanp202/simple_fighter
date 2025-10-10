use sdl3::{pixels::{FColor, PixelFormat}, render::{Canvas, FPoint, Texture, TextureCreator}, sys::pixels::SDL_PIXELFORMAT_ABGR8888, video::{Window, WindowContext}};

use crate::game::boxes::{CollisionBox, HitBox, HurtBox};

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

pub fn load_texture<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    global_textures: &mut Vec<Texture<'a>>,
    file_path: &str, width: u32, height: u32
) -> Result<usize, String> {
    let file = std::fs::File::open(file_path).map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
    let reader = std::io::BufReader::new(file);
    let img = image::ImageReader::new(reader)
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();

    let mut texture = texture_creator.create_texture_streaming(
        unsafe {PixelFormat::from_ll(SDL_PIXELFORMAT_ABGR8888)},
        width,
        height
    ).map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
    texture.update(None, &img.to_rgba8(), 4 * img.width() as usize)
        .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
    global_textures.push(texture);
    Ok(global_textures.len() - 1)
}