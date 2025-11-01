mod boxes;
mod character;
mod input;
mod net;
mod physics;
mod projectile;
mod render;
mod scene;
mod stage;

use std::time::{Duration, Instant};

use sdl3::{
    EventPump,
    event::{Event, WindowEvent},
    pixels::Color,
    render::{Canvas, FPoint, Texture, TextureCreator},
    video::{Window, WindowContext},
};

use crate::game::{
    input::{
        InputHistory, Inputs, PLAYER1_BUTTONS, PLAYER1_DIRECTIONS, PLAYER2_BUTTONS,
        PLAYER2_DIRECTIONS,
    },
    render::{Camera, animation::Animation, load_texture},
    scene::{Scene, Scenes},
    stage::Stage,
};

const FRAME_RATE: usize = 60;
const FRAME_DURATION: f64 = 1.0 / FRAME_RATE as f64;
const SCORE_TO_WIN: u32 = 2;
const MAX_ROLLBACK_FRAMES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

pub struct GameContext {
    should_quit: bool,
    main_menu_texture: usize,
    round_start_animation: Animation,
    timer_animation: Animation,
    stage: Stage,
    player1: character::Context,
    player2: character::Context,

    // Resources
    camera: Camera,
}

impl GameContext {
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
}

#[derive(Clone, PartialEq)]
pub struct GameState {
    player1_inputs: Inputs,
    player2_inputs: Inputs,
    player1: character::State,
    player2: character::State,
}

pub struct PlayerInputs {
    player1: InputHistory,
    player2: InputHistory,
}

impl PlayerInputs {
    pub fn reset_player1(&mut self) {
        self.player1.reset();
    }

    pub fn reset_player2(&mut self) {
        self.player2.reset();
    }

    pub fn update_player1(&mut self) {
        self.player1.update();
    }

    pub fn update_player2(&mut self) {
        self.player2.update();
    }

    pub fn skip_player1(&mut self) {
        self.player1.skip();
    }

    pub fn skip_player2(&mut self) {
        self.player2.skip();
    }

    pub fn online_key_mapping(&mut self) {
        self.player2
            .set_mappings(PLAYER1_BUTTONS, PLAYER1_DIRECTIONS);
    }

    pub fn local_key_mapping(&mut self) {
        self.player2
            .set_mappings(PLAYER2_BUTTONS, PLAYER2_DIRECTIONS);
    }

    pub fn set_delay(&mut self, delay: usize) {
        self.player1.set_delay(delay);
        self.player2.set_delay(delay);
    }
}

pub struct Game<'a> {
    context: GameContext,
    state: GameState,
    scene: Scenes,

    inputs: PlayerInputs,

    // Window management / render
    global_textures: Vec<Texture<'a>>,
    canvas: Canvas<Window>,
    events: EventPump,
    _texture_creator: &'a TextureCreator<WindowContext>,
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
            render::animation::AnimationLayout::Vertical,
        )
        .expect("Invalid round start animation");
        let timer_animation = Animation::load(
            texture_creator,
            &mut global_textures,
            "./resources/scenes/timer_100.png",
            128,
            128,
            100,
            render::animation::AnimationLayout::Vertical,
        )
        .expect("Invalid timer animation");

        let (player1_context, player1_state) = character::from_config(
            texture_creator,
            &mut global_textures,
            "./resources/character1/character1.json",
            FPoint::new(-100.0, 0.0),
            Side::Left,
        )
        .expect("Failed to load player1 config");
        let (player2_context, player2_state) = character::from_config(
            texture_creator,
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

        Self {
            context: GameContext {
                should_quit: false,
                main_menu_texture,
                round_start_animation,
                timer_animation,
                stage: Stage::init(texture_creator, &mut global_textures),
                player1: player1_context,
                player2: player2_context,
                camera: Camera::new(screen_dim),
            },
            state,
            scene: Scenes::new(),

            inputs: PlayerInputs {
                player1: player1_input_history,
                player2: player2_input_history,
            },

            // Window management
            canvas,
            events,
            global_textures,
            _texture_creator: texture_creator,
        }
    }

    pub fn run(mut self) {
        let mut last_frame = Instant::now();
        let mut lag = 0.0;
        while !self.context.should_quit {
            let frame_start = Instant::now();
            lag += frame_start
                .checked_duration_since(last_frame)
                .unwrap_or(Duration::ZERO)
                .as_secs_f64();

            self.input();

            while lag >= FRAME_DURATION {
                self.update();
                lag -= FRAME_DURATION;
            }

            self.render();

            last_frame = frame_start;
            spin_sleep::sleep(
                Duration::from_secs_f64(FRAME_DURATION).saturating_sub(frame_start.elapsed()),
            );
        }

        self.scene.exit(&self.context, &mut self.inputs, &mut self.state);
    }

    fn input(&mut self) {
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => self.context.should_quit = true,
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
                    self.inputs.player1.handle_keypress(keycode);
                    self.inputs.player2.handle_keypress(keycode);
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.inputs.player1.handle_keyrelease(keycode);
                    self.inputs.player2.handle_keyrelease(keycode);
                }
                _ => {}
            }
        }
    }

    fn update(&mut self) {
        // Handle inputs
        self.scene
            .handle_input(&self.context, &mut self.inputs, &mut self.state)
            .expect("Failed to handle user inputs");

        self.state.player1_inputs.update(
            self.inputs.player1.held_buttons(),
            self.inputs.player1.parse_history(),
        );
        self.state.player2_inputs.update(
            self.inputs.player1.held_buttons(),
            self.inputs.player2.parse_history(),
        );

        if let Some(mut new_scene) = self.scene.update(&self.context, &mut self.state) {
            self.scene
                .exit(&self.context, &mut self.inputs, &mut self.state);
            new_scene.enter(&self.context, &mut self.inputs, &mut self.state);
            self.scene = new_scene;
        }
    }

    fn render(&mut self) {
        self.canvas.set_draw_color(Color::BLACK);
        self.canvas.clear();

        self.scene
            .render(
                &mut self.canvas,
                &self.global_textures,
                &self.context,
                &self.state,
            )
            .expect("Failed to render scene");

        self.canvas.present();
    }
}
