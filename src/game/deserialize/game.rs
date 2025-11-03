use std::error::Error;

use sdl3::{
    EventPump,
    render::{Canvas, TextureCreator},
    video::{Window, WindowContext},
};
use serde::Deserialize;

use crate::game::{
    Game, GameContext, GameState, PlayerInputs,
    deserialize::{AnimationJson, FPointJson, SideJson, TextureJson, character},
    input::{self, PLAYER1_BUTTONS, PLAYER1_DIRECTIONS, PLAYER2_BUTTONS, PLAYER2_DIRECTIONS},
    render::Camera,
    scene::Scenes,
    stage::Stage,
};

pub fn deserialize<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    canvas: Canvas<Window>,
    events: EventPump,
    screen_dim: (u32, u32),
    config: &str,
) -> Result<Game<'a>, Box<dyn Error>> {
    let src = std::fs::read_to_string(config)
        .map_err(|err| format!("Failed to open: '{config}': {err}"))?;
    let game_json: GameJson =
        serde_json::from_str(&src).map_err(|err| format!("Failed to parse: '{config}': {err}"))?;

    let mut global_textures = Vec::new();

    let (player1_context, player1_state) = character::deserialize(
        texture_creator,
        &mut global_textures,
        &game_json.scene_data.gameplay.players.player1,
    )?;
    let (player1_input_history, player1_inputs) =
        input::new_inputs(PLAYER1_BUTTONS, PLAYER1_DIRECTIONS);

    let (player2_context, player2_state) = character::deserialize(
        texture_creator,
        &mut global_textures,
        &game_json.scene_data.gameplay.players.player2,
    )?;
    let (player2_input_history, player2_inputs) =
        input::new_inputs(PLAYER2_BUTTONS, PLAYER2_DIRECTIONS);

    Ok(Game {
        context: GameContext {
            should_quit: false,
            version: game_json.version,
            matchmaking_server: game_json.scene_data.gameplay.matchmaking_server,
            main_menu_texture: game_json
                .scene_data
                .main_menu
                .background
                .make_texture(texture_creator, &mut global_textures)?,
            round_start_animation: game_json
                .scene_data
                .gameplay
                .round_start_animation
                .make_animation(texture_creator, &mut global_textures)?,
            stage: Stage::init(texture_creator, &mut global_textures),
            timer_animation: game_json
                .scene_data
                .gameplay
                .timer_animation
                .make_animation(texture_creator, &mut global_textures)?,
            player1: player1_context,
            player2: player2_context,
            camera: Camera::new(screen_dim),
        },
        state: GameState {
            player1_inputs,
            player2_inputs,
            player1: player1_state,
            player2: player2_state,
        },
        scene: Scenes::new(),
        inputs: PlayerInputs {
            player1: player1_input_history,
            player2: player2_input_history,
        },
        global_textures,
        canvas,
        events,
        _texture_creator: texture_creator,
    })
}

#[derive(Deserialize)]
struct GameJson {
    version: String,
    scene_data: SceneDataJson,
}

#[derive(Deserialize)]
struct SceneDataJson {
    main_menu: MainMenuDataJson,
    gameplay: GameplayDataJson,
}

#[derive(Deserialize)]
struct MainMenuDataJson {
    background: TextureJson,
}

#[derive(Deserialize)]
struct GameplayDataJson {
    matchmaking_server: String,
    round_start_animation: AnimationJson,
    timer_animation: AnimationJson,
    players: PlayersDataJson,
}

#[derive(Deserialize)]
struct PlayersDataJson {
    player1: PlayerJson,
    player2: PlayerJson,
}

#[derive(Deserialize)]
pub struct PlayerJson {
    pub config: String,
    pub start_pos: FPointJson,
    pub start_side: SideJson,
}
