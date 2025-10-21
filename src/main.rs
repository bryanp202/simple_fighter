mod game;
use crate::game::Game;

const DEFAULT_SCREEN_WIDTH: u32 = 960;
const DEFAULT_SCREEN_HEIGHT: u32 = 540;
const SCREEN_SCALE_RATIO: f32 = 1.0;

fn main() {
    let screen_dim = (
        (DEFAULT_SCREEN_WIDTH as f32 * SCREEN_SCALE_RATIO) as u32,
        (DEFAULT_SCREEN_HEIGHT as f32 * SCREEN_SCALE_RATIO) as u32,
    );

    let sdl = sdl3::init().expect("Failed to init sdl");
    let video_subsystem = sdl.video().expect("Failed to init video subsystem");
    let window = video_subsystem
        .window("Fighter", screen_dim.0, screen_dim.1)
        .resizable()
        .build()
        .expect("Failed to make window");
    let canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let events = sdl.event_pump().expect("Failed to make event pump");

    let game = Game::init(&texture_creator, canvas, events, screen_dim);

    if cfg!(feature = "debug") {
        println!("Game initaliazed");
        println!("Screen dim: (w: {}, h: {})", screen_dim.0, screen_dim.1);
        println!("Video driver: {:?}", video_subsystem.current_video_driver());
    }

    game.run()
}
