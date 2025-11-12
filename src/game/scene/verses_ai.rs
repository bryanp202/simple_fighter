use candle_core::Device;
use candle_nn::VarMap;

use crate::game::{
    GameContext, GameState, PlayerInputs,
    ai::{get_ai_action, make_model, map_ai_action, serialize_observation},
    scene::{
        Scene, Scenes,
        gameplay::{GameplayScene, GameplayScenes},
        main_menu::MainMenu,
    },
};

const GAMEPLAY_EPSILON: f64 = 0.05;

pub struct VersesAi {
    scene: GameplayScenes,
    _var_map: VarMap,
    ai_agent: candle_nn::Sequential,
    device: Device,
}

impl Scene for VersesAi {
    fn enter(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) {
        inputs.local_key_mapping();
        self.scene.enter(context, state);
    }

    fn handle_input(
        &mut self,
        context: &GameContext,
        inputs: &mut crate::game::PlayerInputs,
        state: &mut GameState,
    ) -> std::io::Result<()> {
        inputs.update_player1();

        let timer = match &self.scene {
            GameplayScenes::DuringRound(during_round) => during_round.timer(),
            _ => 0.0,
        };

        let observation = serialize_observation(&self.device, timer, context, state)
            .expect("Model failed to observe environment");

        let ai_action = get_ai_action(&self.ai_agent, &observation, GAMEPLAY_EPSILON)
            .expect("Model failed to exploit");
        let (dir, buttons) = map_ai_action(ai_action);
        inputs.skip_player2();
        inputs.player2.append_input(0, dir, buttons);

        state.player2_inputs.update(
            inputs.player2.held_buttons(),
            inputs.player2.parse_history(),
        );

        Ok(())
    }

    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Option<super::Scenes> {
        if let Some(new_gameplay_scene) = self.scene.update(context, state) {
            self.scene.exit(context, state);
            self.scene = new_gameplay_scene;
            self.scene.enter(context, state);
        }

        match self.scene {
            GameplayScenes::Exit => Some(Scenes::MainMenu(MainMenu::new())),
            _ => None,
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

impl VersesAi {
    pub fn new(model_path: &str) -> Self {
        let mut var_map = VarMap::new();
        var_map.load(model_path).expect("Failed to load agent");
        let agent = make_model(&var_map, &Device::Cpu).expect("Failed to make agent");

        Self {
            scene: GameplayScenes::new_round_start((0, 0)),
            _var_map: var_map,
            ai_agent: agent,
            device: Device::Cpu,
        }
    }
}
