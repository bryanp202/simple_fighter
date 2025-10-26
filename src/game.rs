mod boxes;
mod character;
mod input;
mod physics;
mod projectile;
mod render;
mod scene;
mod stage;

use std::time::{Duration, Instant};

use sdl3::{
    EventPump,
    event::{Event, WindowEvent},
    keyboard::Keycode,
    pixels::Color,
    render::{Canvas, FPoint, Texture, TextureCreator},
    video::{Window, WindowContext},
};

use crate::{
    game::{
        input::{
            InputHistory, Inputs, PLAYER1_BUTTONS, PLAYER1_DIRECTIONS, PLAYER2_BUTTONS,
            PLAYER2_DIRECTIONS,
        },
        render::{Camera, animation::Animation, load_texture},
        scene::{Scene, Scenes},
        stage::Stage,
    },
    ring_buf::RingBuf,
};

const FRAME_RATE: usize = 60;
const FRAME_DURATION: f32 = 1.0 / FRAME_RATE as f32;
const SCORE_TO_WIN: u32 = 2;
const MAX_ROLLBACK_FRAMES: usize = 64;

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
    player1: character::Context,
    player2: character::Context,

    // Resources
    camera: Camera,
}

#[derive(Clone)]
pub struct GameState {
    player1_inputs: Inputs,
    player2_inputs: Inputs,
    player1: character::State,
    player2: character::State,
}

struct GameWorld {
    context: GameContext,
    state: GameState,
    scene: Scenes,

    // Resources
    player1_input_history: InputHistory,
    player2_input_history: InputHistory,
    game_state_history: RingBuf<(Scenes, GameState), MAX_ROLLBACK_FRAMES>,
}

pub struct Game<'a> {
    game_world: GameWorld,

    // Window management / render
    global_textures: Vec<Texture<'a>>,
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

        let (player1_context, player1_state) = character::from_config(
            &texture_creator,
            &mut global_textures,
            "./resources/character1/character1.json",
            FPoint::new(-100.0, 0.0),
            Side::Left,
        )
        .expect("Failed to load player1 config");
        let (player2_context, player2_state) = character::from_config(
            &texture_creator,
            &mut global_textures,
            "./resources/character1/character2.json",
            FPoint::new(100.0, 0.0),
            Side::Right,
        )
        .expect("Failed to load player2 config");

        let (player1_input_history, player1_inputs) =
            input::new_inputs(PLAYER1_BUTTONS, PLAYER1_DIRECTIONS);
        let (player2_input_history, player2_inputs) =
            input::new_inputs(PLAYER2_BUTTONS, PLAYER2_DIRECTIONS);

        let state = GameState {
            player1: player1_state,
            player2: player2_state,
            player1_inputs,
            player2_inputs,
        };
        let scene = Scenes::new();
        let game_state_history = RingBuf::new((scene.clone(), state.clone()));

        Self {
            game_world: GameWorld {
                context: GameContext {
                    main_menu_texture,
                    round_start_animation,
                    timer_animation,
                    stage: Stage::init(texture_creator, &mut global_textures),
                    player1: player1_context,
                    player2: player2_context,
                    camera: Camera::new(screen_dim),
                },
                state,
                scene,

                player1_input_history,
                player2_input_history,
                game_state_history,
            },

            canvas,
            events,
            global_textures,
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
                    self.game_world.context.camera.resize((x as u32, y as u32));
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } if keycode == Keycode::Space => {
                    self.game_world.rollback(3);
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.game_world
                        .player1_input_history
                        .handle_keypress(keycode);
                    self.game_world
                        .player2_input_history
                        .handle_keypress(keycode);
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.game_world
                        .player1_input_history
                        .handle_keyrelease(keycode);
                    self.game_world
                        .player2_input_history
                        .handle_keyrelease(keycode);
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, dt: f32) {
        // Handle inputs
        self.game_world.player1_input_history.update();
        self.game_world.player2_input_history.update();
        self.game_world
            .state
            .player1_inputs
            .update(self.game_world.player1_input_history.parse_history());
        self.game_world
            .state
            .player2_inputs
            .update(self.game_world.player2_input_history.parse_history());

        if let Some(mut new_scene) =
            self.game_world
                .scene
                .update(&self.game_world.context, &mut self.game_world.state, dt)
        {
            self.game_world
                .scene
                .exit(&self.game_world.context, &mut self.game_world.state);
            new_scene.enter(&self.game_world.context, &mut self.game_world.state);
            self.game_world.scene = new_scene;
        }

        self.game_world
            .game_state_history
            .append((self.game_world.scene.clone(), self.game_world.state.clone()));
    }

    fn render(&mut self) {
        self.canvas.set_draw_color(Color::BLACK);
        self.canvas.clear();

        self.game_world
            .scene
            .render(
                &mut self.canvas,
                &self.global_textures,
                &self.game_world.context,
                &self.game_world.state,
            )
            .expect("Failed to render scene");

        self.canvas.present();
    }
}

impl GameWorld {
    fn rollback(&mut self, frames: usize) {
        let (scene, state) = self.game_state_history.rewind(frames);
        self.scene = scene;
        self.state = state;

        self.fast_simulate(frames);
    }

    fn fast_simulate(&mut self, frames: usize) {
        for frame in 0..frames {
            self.state
                .player1_inputs
                .update(self.player1_input_history.parse_history_at(frames - frame));
            self.state
                .player2_inputs
                .update(self.player2_input_history.parse_history_at(frames - frame));

            if let Some(mut new_scene) =
                self.scene
                    .update(&self.context, &mut self.state, FRAME_DURATION)
            {
                self.scene.exit(&self.context, &mut self.state);
                new_scene.enter(&self.context, &mut self.state);
                self.scene = new_scene;
            }

            self.game_state_history
                .append((self.scene.clone(), self.state.clone()));
        }
    }
}
