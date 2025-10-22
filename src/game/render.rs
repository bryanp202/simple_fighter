use image::DynamicImage;
use sdl3::{
    pixels::{FColor, PixelFormat},
    rect::Rect,
    render::{Canvas, FPoint, FRect, Texture, TextureCreator},
    sys::pixels::SDL_PIXELFORMAT_ABGR8888,
    video::{Window, WindowContext},
};

use crate::{
    DEFAULT_SCREEN_HEIGHT, DEFAULT_SCREEN_WIDTH,
    game::{
        Side,
        boxes::{CollisionBox, HitBox, HurtBox},
        render::animation::{Animation, AnimationLayout},
    },
};

pub mod animation;

pub struct Camera {
    screen_w: u32,
    screen_h: u32,
    game_center: FPoint,
    game_to_screen_ratio: FPoint,
    offset: FPoint,
}

impl Camera {
    const SCREEN_WIDTH_RATIO_1: u32 = DEFAULT_SCREEN_WIDTH;
    const SCREEN_HEIGHT_RATIO_1: u32 = DEFAULT_SCREEN_HEIGHT;

    pub fn new(screen_dim: (u32, u32)) -> Self {
        let (w, h) = screen_dim;
        Self {
            screen_w: w,
            screen_h: h,
            game_center: Self::calc_game_center(w, h),
            offset: FPoint { x: 0.0, y: 0.0 },
            game_to_screen_ratio: Self::calc_screen_ratio(w, h),
        }
    }

    pub fn resize(&mut self, screen_dim: (u32, u32)) {
        let (w, h) = screen_dim;
        self.screen_w = w;
        self.screen_h = h;
        self.game_center = Self::calc_game_center(w, h);
        self.game_to_screen_ratio = Self::calc_screen_ratio(w, h);
    }

    pub fn render_animation(
        &self,
        canvas: &mut Canvas<Window>,
        global_textures: &Vec<Texture>,
        pos: &FPoint,
        animation: &Animation,
        frame: usize,
    ) -> Result<(), sdl3::Error> {
        let screen_pos = self.to_screen_pos(pos);

        let (texture, src) = animation.get_frame(frame, global_textures);
        // Animation is rendered with the pos in the center
        let width = src.w * self.game_to_screen_ratio.x;
        let height = src.h * self.game_to_screen_ratio.y;
        let dst = FRect::new(
            screen_pos.x - width / 2.0,
            screen_pos.y - height / 2.0,
            width,
            height,
        );
        canvas.copy(texture, src, dst)
    }

    pub fn render_animation_on_side(
        &self,
        canvas: &mut Canvas<Window>,
        global_textures: &Vec<Texture>,
        pos: &FPoint,
        animation: &Animation,
        frame: usize,
        side: &Side,
    ) -> Result<(), sdl3::Error> {
        let screen_pos = self.to_screen_pos(pos);
        let flip_horz = match side {
            Side::Left => false,
            Side::Right => true,
        };

        let (texture, src) = animation.get_frame_cycle(frame, global_textures);
        // Sprite is rendered with the character pos in the center
        let width = src.w * self.game_to_screen_ratio.x;
        let height = src.h * self.game_to_screen_ratio.y;
        let dst = FRect::new(
            screen_pos.x - width / 2.0,
            screen_pos.y - height / 2.0,
            width,
            height,
        );
        canvas.copy_ex(texture, src, dst, 0.0, None, flip_horz, false)
    }

    fn to_screen_pos(&self, pos: &FPoint) -> FPoint {
        FPoint::new(
            self.game_center.x + (pos.x - self.offset.x) * self.game_to_screen_ratio.x,
            self.game_center.y - (pos.y + self.offset.y) * self.game_to_screen_ratio.y,
        )
    }

    fn to_screen_rect(&self, rect: &FRect) -> FRect {
        FRect::new(
            self.game_center.x + (rect.x - self.offset.x) * self.game_to_screen_ratio.x,
            self.game_center.y - (rect.y + self.offset.y) * self.game_to_screen_ratio.y,
            rect.w * self.game_to_screen_ratio.x,
            rect.h * self.game_to_screen_ratio.y,
        )
    }

    fn calc_screen_ratio(screen_w: u32, screen_h: u32) -> FPoint {
        FPoint::new(
            screen_w as f32 / Self::SCREEN_WIDTH_RATIO_1 as f32,
            screen_h as f32 / Self::SCREEN_HEIGHT_RATIO_1 as f32,
        )
    }

    fn calc_game_center(screen_w: u32, screen_h: u32) -> FPoint {
        FPoint::new(screen_w as f32 / 2.0, screen_h as f32 * 0.9)
    }
}

pub fn draw_hit_boxes_system(
    canvas: &mut Canvas<Window>,
    camera: &Camera,
    side: &Side,
    offset: FPoint,
    hitboxes: &[HitBox],
) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(1.0, 0.0, 0.0, 0.5));
    for hitbox in hitboxes {
        let on_side_hitbox = hitbox.on_side(side, offset);
        let on_screen_rect = camera.to_screen_rect(&on_side_hitbox);
        canvas.fill_rect(on_screen_rect)?;
    }
    Ok(())
}

pub fn draw_hurt_boxes_system(
    canvas: &mut Canvas<Window>,
    camera: &Camera,
    side: &Side,
    offset: FPoint,
    hurtboxes: &[HurtBox],
) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(0.0, 1.0, 0.0, 0.5));
    for hurtbox in hurtboxes {
        let on_side_hitbox = hurtbox.on_side(side, offset);
        let on_screen_rect = camera.to_screen_rect(&on_side_hitbox);
        canvas.fill_rect(on_screen_rect)?;
    }
    Ok(())
}

pub fn draw_collision_box_system(
    canvas: &mut Canvas<Window>,
    camera: &Camera,
    side: &Side,
    offset: FPoint,
    collision_box: &CollisionBox,
) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(1.0, 1.0, 1.0, 0.5));
    let on_side_hitbox = collision_box.on_side(side, offset);
    let on_screen_rect = camera.to_screen_rect(&on_side_hitbox);
    canvas.fill_rect(on_screen_rect)?;
    Ok(())
}

fn open_img(file_path: &str) -> Result<DynamicImage, String> {
    let file = std::fs::File::open(file_path)
        .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
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

    let mut texture = texture_creator
        .create_texture_streaming(
            unsafe { PixelFormat::from_ll(SDL_PIXELFORMAT_ABGR8888) },
            img.width(),
            img.height(),
        )
        .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;

    texture
        .update(None, &img.to_rgba8(), 4 * img.width() as usize)
        .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;

    global_textures.push(texture);

    Ok(global_textures.len() - 1)
}

pub fn load_animation<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    global_textures: &mut Vec<Texture<'a>>,
    file_path: &str,
    width: u32,
    height: u32,
    frames: u32,
    layout: AnimationLayout,
) -> Result<usize, String> {
    let img = open_img(file_path)?;

    let mut texture = texture_creator
        .create_texture_streaming(
            unsafe { PixelFormat::from_ll(SDL_PIXELFORMAT_ABGR8888) },
            width,
            height * frames,
        )
        .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;

    match layout {
        AnimationLayout::VERTICAL => {
            let frames_rect = Rect::new(0, 0, width, height * frames);
            let frames = img.crop_imm(
                frames_rect.x as u32,
                frames_rect.y as u32,
                frames_rect.width(),
                frames_rect.height(),
            );
            texture
                .update(frames_rect, &frames.to_rgba8(), 4 * frames.width() as usize)
                .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
        }
        AnimationLayout::HORIZONTAL => {
            for frame in 0..frames {
                let frame_rect = Rect::new((frame * width) as i32, 0, width, height);
                let frame = img.crop_imm(
                    frame_rect.x as u32,
                    frame_rect.y as u32,
                    frame_rect.width(),
                    frame_rect.height(),
                );
                let texture_frame = Rect::new(
                    frame_rect.y,
                    frame_rect.x,
                    frame_rect.width(),
                    frame_rect.height(),
                );
                texture
                    .update(texture_frame, &frame.to_rgba8(), 4 * frame.width() as usize)
                    .map_err(|err| format!("File: '{}': {}", file_path, err.to_string()))?;
            }
        }
    }
    global_textures.push(texture);

    Ok(global_textures.len() - 1)
}
