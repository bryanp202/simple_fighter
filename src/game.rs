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
    event::{Event, WindowEvent}, keyboard::Keycode, pixels::Color, render::{Canvas, FPoint, FRect, Texture, TextureCreator}, video::{Window, WindowContext}, EventPump
};

use crate::{
    game::{
        input::{Inputs, PLAYER1_BUTTONS, PLAYER1_DIRECTIONS, PLAYER2_BUTTONS, PLAYER2_DIRECTIONS}, physics::{check_hit_collisions, movement_system, side_detection}, render::Camera, stage::Stage
    }
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
    camera: Camera,
    textures: Vec<Texture<'a>>,
    player1_inputs: Inputs,
    player2_inputs: Inputs,

    // Window management
    canvas: Canvas<Window>,
    events: EventPump,
    _texture_creator: &'a TextureCreator<WindowContext>,
    should_quit: bool,
}

impl<'a> Game<'a> {
    pub fn init(
        texture_creator: &'a TextureCreator<WindowContext>,
        canvas: Canvas<Window>,
        events: EventPump,
        screen_dim: (u32, u32),
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

            camera: Camera::new(screen_dim),
            textures,
            player1_inputs: Inputs::new(PLAYER1_BUTTONS, PLAYER1_DIRECTIONS),
            player2_inputs: Inputs::new(PLAYER2_BUTTONS, PLAYER2_DIRECTIONS),

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
                Event::Window { win_event: WindowEvent::Resized(x, y), ..} => {
                    self.camera.resize((x as u32, y as u32));
                },
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
                    self.player1_inputs.handle_keypress(keycode);
                    self.player2_inputs.handle_keypress(keycode);
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    self.player1_inputs.handle_keyrelease(keycode);
                    self.player2_inputs.handle_keyrelease(keycode);
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, dt: f32) {
        self.player1_inputs.update();
        self.player2_inputs.update();
        self.player1.update(&self.player1_inputs);
        self.player2.update(&self.player2_inputs);

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
            .render(&mut self.canvas, &self.camera, &self.textures)
            .unwrap();
        self.player2
            .render(&mut self.canvas, &self.camera, &self.textures)
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
            let blocked = player2.receive_hit(&player1_hit);
            player1.successful_hit(&player1_hit, blocked);
            4
        }
        (None, Some(player2_hit)) => {
            let blocked = player1.receive_hit(&player2_hit);
            player2.successful_hit(&player2_hit, blocked);
            4
        }
        (Some(player1_hit), Some(player2_hit)) => {
            player1.successful_hit(&player1_hit, true);
            player2.successful_hit(&player2_hit, true);
            8
        },
        _ => 0,
    }
}

fn render_player1_health(canvas: &mut Canvas<Window>, hp_per: f32) -> Result<(), sdl3::Error> {
    let (screen_w, screen_h) = canvas.window().size();
    let bar_h = screen_h as f32 / 20.0;
    let bar_width = screen_w as f32 * 0.4;
    canvas.set_draw_color(Color::RED);
    canvas.fill_rect(FRect::new(0.0, 0.0, bar_width, bar_h))?;
    canvas.set_draw_color(Color::GREEN);
    let health_bar = hp_per * bar_width;
    canvas.fill_rect(FRect::new(bar_width - health_bar, 0.0, health_bar, bar_h))?;

    Ok(())
}

fn render_player2_health(canvas: &mut Canvas<Window>, hp_per: f32) -> Result<(), sdl3::Error> {
    let (screen_w, screen_h) = canvas.window().size();
    let bar_h = screen_h as f32 / 20.0;
    let bar_width = screen_w as f32 * 0.4;
    canvas.set_draw_color(Color::RED);
    canvas.fill_rect(FRect::new(
        screen_w as f32 - bar_width,
        0.0,
        bar_width,
        bar_h,
    ))?;
    canvas.set_draw_color(Color::GREEN);
    let health_bar = hp_per * bar_width;
    canvas.fill_rect(FRect::new(
        screen_w as f32 - bar_width,
        0.0,
        health_bar,
        bar_h,
    ))?;

    Ok(())
}
