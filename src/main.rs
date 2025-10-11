mod game;
use crate::game::Game;

const DEFAULT_SCREEN_WIDTH: u32 = 800;
const DEFAULT_SCREEN_HEIGHT: u32 = 600;

fn main() {
    let sdl = sdl3::init().expect("Failed to init sdl");
    let video_subsystem = sdl.video().expect("Failed to init video subsystem");
    let window = video_subsystem.window("Fighter", DEFAULT_SCREEN_WIDTH, DEFAULT_SCREEN_HEIGHT)
        .build()
        .expect("Failed to make window");
    let canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let events = sdl.event_pump().expect("Failed to make event pump");

    let game = Game::init(&texture_creator, canvas, events);
    println!("Game initaliazed");

    game.run();
}
