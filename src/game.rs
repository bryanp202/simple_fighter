mod boxes;
mod character;
mod input;
mod physics;
mod projectile;
mod render;
mod stage;

use std::time::{Duration, Instant};

use character::Character;
use sdl3::{
    EventPump,
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    render::{Canvas, FPoint, FRect, Texture, TextureCreator},
    video::{Window, WindowContext},
};

use crate::{
    DEFAULT_SCREEN_WIDTH,
    game::{
        input::Inputs,
        physics::{check_hit_collisions, movement_system, side_detection},
        stage::Stage,
    },
};

const FRAME_RATE: usize = 60;
const FRAME_DURATION: f32 = 1.0 / FRAME_RATE as f32;

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

pub struct Game<'a> {
    stage: Stage,
    player1: Character,
    player2: Character,
    //player2: Character,
    timer: f32,
    score: (usize, usize),
    hit_freeze: usize,

    // Resources
    textures: Vec<Texture<'a>>,
    inputs: Inputs,

    // Window management
    canvas: Canvas<Window>,
    events: EventPump,
    texture_creator: &'a TextureCreator<WindowContext>,
    should_quit: bool,
}

impl<'a> Game<'a> {
    pub fn init(
        texture_creator: &'a TextureCreator<WindowContext>,
        canvas: Canvas<Window>,
        events: EventPump,
    ) -> Self {
        let mut textures = Vec::new();
        Self {
            stage: Stage::init(texture_creator, &mut textures),
            player1: Character::from_config(
                &texture_creator,
                &mut textures,
                "./resources/character1/character1.json",
                FPoint::new(-100.0, 0.0),
                Side::Left,
            )
            .unwrap(),
            player2: Character::from_config(
                &texture_creator,
                &mut textures,
                "./resources/character1/character2.json",
                FPoint::new(100.0, 0.0),
                Side::Right,
            )
            .unwrap(),
            timer: 0.0,
            score: (0, 0),
            hit_freeze: 0,

            textures,
            inputs: Inputs::new(),

            canvas,
            events,
            texture_creator,
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
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    self.player1.reset();
                    self.player2.reset();
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.inputs.handle_keypress(keycode);
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.inputs.handle_keyrelease(keycode);
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, dt: f32) {
        self.inputs.update();
        self.player1.update(&self.inputs);
        self.player2.update(&self.inputs);

        if self.hit_freeze == 0 {
            self.player1.movement_update();
            self.player2.movement_update();

            let (player1_pos, player2_pos) = movement_system(
                self.player1.get_side(),
                &self.player1.pos(),
                &self.player1.get_collision_box(),
                self.player2.get_side(),
                &self.player2.pos(),
                &self.player2.get_collision_box(),
                &self.stage,
            );
            self.player1.set_pos(player1_pos);
            self.player2.set_pos(player2_pos);

            if let Some(player1_side) = side_detection(&self.player1.pos(), &self.player2.pos()) {
                self.player1.set_side(player1_side);
                self.player2.set_side(player1_side.opposite());
            }

            self.hit_freeze = handle_hit_boxes(&mut self.player1, &mut self.player2);
        } else {
            self.hit_freeze -= 1;
        }
    }

    fn render(&mut self) {
        self.canvas.set_draw_color(Color::BLACK);
        self.canvas.clear();

        self.stage.render(&mut self.canvas, &self.textures).unwrap();
        self.player1
            .render(&mut self.canvas, &self.textures)
            .unwrap();
        self.player2
            .render(&mut self.canvas, &self.textures)
            .unwrap();

        let player1_hp_per = self.player1.current_hp() / self.player1.max_hp();
        render_player1_health(&mut self.canvas, player1_hp_per).unwrap();
        let player2_hp_per = self.player2.current_hp() / self.player2.max_hp();
        render_player2_health(&mut self.canvas, player2_hp_per).unwrap();

        self.canvas.present();
    }
}

// Returns the amount of frames for hit freeze
fn handle_hit_boxes(player1: &mut Character, player2: &mut Character) -> usize {
    let player1_pos = player1.pos();
    let player1_side = player1.get_side();
    let player2_pos = player2.pos();
    let player2_side = player2.get_side();

    let player1_hit_boxes = player1.get_hit_boxes();
    let player2_hurt_boxes = player2.get_hurt_boxes();
    let player1_hit = check_hit_collisions(
        player1_side,
        player1_pos,
        player1_hit_boxes,
        player2_side,
        player2_pos,
        player2_hurt_boxes,
    );

    let player2_hit_boxes = player2.get_hit_boxes();
    let player1_hurt_boxes = player1.get_hurt_boxes();
    let player2_hit = check_hit_collisions(
        player2_side,
        player2_pos,
        player2_hit_boxes,
        player1_side,
        player1_pos,
        player1_hurt_boxes,
    );

    match (player1_hit, player2_hit) {
        (Some(player1_hit), None) => {
            player1.successful_hit(&player1_hit);
            player2.receive_hit(&player1_hit);
            4
        }
        (None, Some(player2_hit)) => {
            player2.successful_hit(&player2_hit);
            player1.receive_hit(&player2_hit);
            4
        }
        (Some(_), Some(_)) => 4,
        _ => 0,
    }
}

fn render_player1_health(canvas: &mut Canvas<Window>, hp_per: f32) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(Color::RED);
    canvas.fill_rect(FRect::new(0.0, 0.0, 300.0, 20.0))?;
    canvas.set_draw_color(Color::GREEN);
    let health_bar = hp_per * 300.0;
    canvas.fill_rect(FRect::new(300.0 - health_bar, 0.0, health_bar, 20.0))?;

    Ok(())
}

fn render_player2_health(canvas: &mut Canvas<Window>, hp_per: f32) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(Color::RED);
    canvas.fill_rect(FRect::new(
        DEFAULT_SCREEN_WIDTH as f32 - 300.0,
        0.0,
        300.0,
        20.0,
    ))?;
    canvas.set_draw_color(Color::GREEN);
    let health_bar = hp_per * 300.0;
    canvas.fill_rect(FRect::new(
        DEFAULT_SCREEN_WIDTH as f32 - 300.0,
        0.0,
        health_bar,
        20.0,
    ))?;

    Ok(())
}
