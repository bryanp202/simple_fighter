use crate::game::{
    GameContext, GameState, PlayerInputs,
    scene::{
        Scene, Scenes,
        gameplay::{GameplayScene, GameplayScenes},
        main_menu::MainMenu,
    },
};

pub struct LocalPlay {
    scene: GameplayScenes,
}

impl Scene for LocalPlay {
    fn enter(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) {
        inputs.local_key_mapping();
        self.scene.enter(context, state);
    }

    fn handle_input(
        &mut self,
        _context: &GameContext,
        inputs: &mut crate::game::PlayerInputs,
        _state: &mut GameState,
    ) -> Result<(), String> {
        inputs.update_player1();
        inputs.update_player2();
        Ok(())
    }

    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Result<Option<Scenes>, String> {
        if let Some(new_gameplay_scene) = self.scene.update(context, state) {
            self.scene.exit(context, state);
            self.scene = new_gameplay_scene;
            self.scene.enter(context, state);
        }

        match self.scene {
            GameplayScenes::Exit => Ok(Some(Scenes::MainMenu(MainMenu::new()))),
            _ => Ok(None),
        }
    }

    fn render(
        &self,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &[sdl3::render::Texture],
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error> {
        self.scene.render(canvas, global_textures, context, state)
    }

    fn exit(&mut self, context: &GameContext, _inputs: &mut PlayerInputs, state: &mut GameState) {
        self.scene.exit(context, state);
    }
}

impl LocalPlay {
    pub fn new() -> Self {
        Self {
            scene: GameplayScenes::new_round_start((0, 0)),
        }
    }
}
