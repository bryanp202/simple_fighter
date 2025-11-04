use sdl3::{
    render::{Canvas, Texture},
    video::Window,
};

use crate::game::{
    GameContext, GameState, PlayerInputs,
    scene::{
        connecting::Connecting, hosting::Hosting, local_play::LocalPlay, main_menu::MainMenu,
        matching::Matching, online_play::OnlinePlay,
    },
};

mod connecting;
mod gameplay;
mod hosting;
mod local_play;
mod main_menu;
mod matching;
mod online_play;

pub trait Scene {
    fn enter(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState);
    fn handle_input(
        &mut self,
        context: &GameContext,
        inputs: &mut PlayerInputs,
        state: &mut GameState,
    ) -> std::io::Result<()>;
    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Option<Scenes>;
    fn render(
        &self,
        canvas: &mut Canvas<Window>,
        global_textures: &[Texture],
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error>;
    fn exit(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState);
}

pub enum Scenes {
    MainMenu(MainMenu),
    LocalPlay(LocalPlay),
    OnlinePlay(OnlinePlay),
    Hosting(Hosting),
    Connecting(Connecting),
    Matching(Matching),
    //RoundEnd,
    //WinScreen,
    //Settings,
}

impl Scene for Scenes {
    fn enter(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) {
        match self {
            Self::MainMenu(main_menu) => main_menu.enter(context, inputs, state),
            Self::LocalPlay(local_play) => local_play.enter(context, inputs, state),
            Self::OnlinePlay(online_play) => online_play.enter(context, inputs, state),
            Self::Hosting(hosting) => hosting.enter(context, inputs, state),
            Self::Connecting(connecting) => connecting.enter(context, inputs, state),
            Self::Matching(matching) => matching.enter(context, inputs, state),
        }
    }

    /// Returns (rollback, fastforward) frames
    fn handle_input(
        &mut self,
        context: &GameContext,
        inputs: &mut PlayerInputs,
        state: &mut GameState,
    ) -> std::io::Result<()> {
        match self {
            Self::MainMenu(main_menu) => main_menu.handle_input(context, inputs, state),
            Self::LocalPlay(local_play) => local_play.handle_input(context, inputs, state),
            Self::OnlinePlay(online_play) => online_play.handle_input(context, inputs, state),
            Self::Hosting(hosting) => hosting.handle_input(context, inputs, state),
            Self::Connecting(connecting) => connecting.handle_input(context, inputs, state),
            Self::Matching(matching) => matching.handle_input(context, inputs, state),
        }
    }

    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Option<Scenes> {
        match self {
            Self::MainMenu(main_menu) => main_menu.update(context, state),
            Self::LocalPlay(local_play) => local_play.update(context, state),
            Self::OnlinePlay(online_play) => online_play.update(context, state),
            Self::Hosting(hosting) => hosting.update(context, state),
            Self::Connecting(connecting) => connecting.update(context, state),
            Self::Matching(matching) => matching.update(context, state),
        }
    }

    fn render(
        &self,
        canvas: &mut Canvas<Window>,
        global_textures: &[Texture],
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error> {
        match self {
            Self::MainMenu(main_menu) => main_menu.render(canvas, global_textures, context, state),
            Self::LocalPlay(local_play) => {
                local_play.render(canvas, global_textures, context, state)
            }
            Self::OnlinePlay(online_play) => {
                online_play.render(canvas, global_textures, context, state)
            }
            Self::Hosting(hosting) => hosting.render(canvas, global_textures, context, state),
            Self::Connecting(connecting) => {
                connecting.render(canvas, global_textures, context, state)
            }
            Self::Matching(matching) => matching.render(canvas, global_textures, context, state),
        }
    }

    fn exit(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) {
        match self {
            Self::MainMenu(main_menu) => main_menu.exit(context, inputs, state),
            Self::LocalPlay(local_play) => local_play.exit(context, inputs, state),
            Self::OnlinePlay(online_play) => online_play.exit(context, inputs, state),
            Self::Hosting(hosting) => hosting.exit(context, inputs, state),
            Self::Connecting(connecting) => connecting.exit(context, inputs, state),
            Self::Matching(matching) => matching.exit(context, inputs, state),
        }
    }
}

impl Scenes {
    pub fn new() -> Self {
        Self::Matching(Matching::new())
    }
}
