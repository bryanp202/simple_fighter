mod boxes;
mod character;
mod input;
mod physics;
mod projectile;
mod render;
mod scene;
mod stage;

use std::time::{Duration, Instant};

use character::Character;
use sdl3::{
    EventPump,
    event::{Event, WindowEvent},
    pixels::Color,
    render::{Canvas, FPoint, Texture, TextureCreator},
    video::{Window, WindowContext},
};

use crate::game::{
    input::{Inputs, PLAYER1_BUTTONS, PLAYER1_DIRECTIONS, PLAYER2_BUTTONS, PLAYER2_DIRECTIONS},
    render::{Camera, animation::Animation, load_texture},
    scene::{Scene, Scenes},
    stage::Stage,
};

const FRAME_RATE: usize = 60;
const FRAME_DURATION: f32 = 1.0 / FRAME_RATE as f32;
const SCORE_TO_WIN: u32 = 2;

#[derive(Clone, Copy, Debug)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn opposite(&self) -> Side {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

pub struct GameContext {
    main_menu_texture: usize,
    round_start_animation: Animation,
    timer_animation: Animation,
    stage: Stage,
    player1: Character,
    player2: Character,

    // Resources
    player1_inputs: Inputs,
    player2_inputs: Inputs,
    camera: Camera,
}

pub struct Game<'a> {
    context: GameContext,
    scene: Scenes,

    // Resources
    global_textures: Vec<Texture<'a>>,

    // Window management
    canvas: Canvas<Window>,
    events: EventPump,
    _texture_creator: &'a TextureCreator<WindowContext>,
    should_quit: bool,
}

impl<'a> Game<'a> {
    /// Maybe this could also be from a config file? ///
    pub fn init(
        texture_creator: &'a TextureCreator<WindowContext>,
        canvas: Canvas<Window>,
        events: EventPump,
        screen_dim: (u32, u32),
    ) -> Self {
        let mut global_textures = Vec::new();

        let main_menu_texture = load_texture(
            texture_creator,
            &mut global_textures,
            "./resources/scenes/main_menu.png",
        )
        .expect("Invalid main menu texture");
        let round_start_animation = Animation::load(
            texture_creator,
            &mut global_textures,
            "./resources/scenes/round_start_text.png",
            512,
            128,
            4,
            render::animation::AnimationLayout::VERTICAL,
        )
        .expect("Invalid round start animation");
        let timer_animation = Animation::load(
            texture_creator,
            &mut global_textures,
            "./resources/scenes/timer_100.png",
            128,
            128,
            100,
            render::animation::AnimationLayout::VERTICAL,
        )
        .expect("Invalid timer animation");

        Self {
            context: GameContext {
                main_menu_texture,
                round_start_animation,
                timer_animation,
                stage: Stage::init(texture_creator, &mut global_textures),
                player1: Character::from_config(
                    &texture_creator,
                    &mut global_textures,
                    "./resources/character1/character1.json",
                    FPoint::new(-100.0, 0.0),
                    Side::Left,
                )
                .expect("Failed to load player1 config"),
                player2: Character::from_config(
                    &texture_creator,
                    &mut global_textures,
                    "./resources/character1/character2.json",
                    FPoint::new(100.0, 0.0),
                    Side::Right,
                )
                .expect("Failed to load player2 config"),

                camera: Camera::new(screen_dim),
                player1_inputs: Inputs::new(PLAYER1_BUTTONS, PLAYER1_DIRECTIONS),
                player2_inputs: Inputs::new(PLAYER2_BUTTONS, PLAYER2_DIRECTIONS),
            },
            scene: Scenes::new(),

            global_textures,

            canvas,
            events,
            _texture_creator: texture_creator,
            should_quit: false,
        }
    }

    pub fn run(mut self) {
        let mut last_frame = Instant::now();
        while !self.should_quit {
            let frame_start = Instant::now();
            let dt = frame_start
                .checked_duration_since(last_frame)
                .unwrap_or(Duration::ZERO)
                .as_secs_f32();

            self.input();
            self.update(dt);
            self.render();

            last_frame = frame_start;
            std::thread::sleep(
                Duration::from_secs_f32(FRAME_DURATION).saturating_sub(frame_start.elapsed()),
            );
        }
    }

    fn input(&mut self) {
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => self.should_quit = true,
                Event::Window {
                    win_event: WindowEvent::Resized(x, y),
                    ..
                } => {
                    self.context.camera.resize((x as u32, y as u32));
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.context.player1_inputs.handle_keypress(keycode);
                    self.context.player2_inputs.handle_keypress(keycode);
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.context.player1_inputs.handle_keyrelease(keycode);
                    self.context.player2_inputs.handle_keyrelease(keycode);
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, dt: f32) {
        self.context.player1_inputs.update();
        self.context.player2_inputs.update();
        if let Some(mut new_scene) = self.scene.update(&mut self.context, dt) {
            self.scene.exit(&mut self.context);
            new_scene.enter(&mut self.context);
            self.scene = new_scene;
        }
    }

    fn render(&mut self) {
        self.canvas.set_draw_color(Color::BLACK);
        self.canvas.clear();

        self.scene
            .render(&self.context, &mut self.canvas, &self.global_textures)
            .expect("Failed to render scene");

        self.canvas.present();
    }
}
