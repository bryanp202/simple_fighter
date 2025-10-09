mod boxes;
mod character;
mod input;
mod projectile;
mod render;

use std::time::{Duration, Instant};

use character::Character;
use sdl3::{event::Event, render::{Canvas, Texture}, video::Window, EventPump};

use crate::game::input::Inputs;

const FRAME_RATE: usize = 60;
const FRAME_DURATION: f32 = 1.0 / FRAME_RATE as f32;

pub struct Game<'a> {
    player1: Character,
    player2: Character,
    timer: f32,
    score: (usize, usize),

    // Resources
    textures: Vec<Texture<'a>>,
    inputs: Inputs,

    // Window management
    canvas: Canvas<Window>,
    events: EventPump,
    should_quit: bool,
}

impl <'a> Game<'a> {
    pub fn init(canvas: Canvas<Window>, events: EventPump) -> Self {
        Self {
            player1: Character::new(),
            player2: Character::new(),
            timer: 0.0,
            score: (0, 0),

            textures: Vec::new(),
            inputs: Inputs::new(),

            canvas,
            events,
            should_quit: false,
        }
    }

    pub fn run(mut self) {
        let mut last_frame = Instant::now();
        while !self.should_quit {
            let frame_start = Instant::now();
            let dt = frame_start.checked_duration_since(last_frame).unwrap_or(Duration::ZERO).as_secs_f32();

            self.input();
            self.update(dt);
            self.render();

            last_frame = frame_start;
            std::thread::sleep(Duration::from_secs_f32(FRAME_DURATION).saturating_sub(frame_start.elapsed()));
        }
    }

    fn input(&mut self) {
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => self.should_quit = true,
                Event::KeyDown { keycode: Some(keycode), repeat: false, .. } => {
                    self.inputs.handle_keypress(keycode);
                },
                Event::KeyUp { keycode: Some(keycode), repeat: false, .. } => {
                    self.inputs.handle_keyrelease(keycode);
                }
                _ => {},
            }
        }
    }

    fn update(&mut self, dt: f32) {
        self.inputs.update();
    }

    fn render(&mut self) {
        //println!("Held Buttons: {:?}, Just Pressed Buttons: {:?}, Dir: {:?}", self.inputs.held_buttons(), self.inputs.just_pressed_buttons(), self.inputs.dir());
        //println!("Move buffer: {:?}", self.inputs.move_buf());
    }
}