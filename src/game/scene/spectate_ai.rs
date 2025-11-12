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

pub struct SpectateAi {
    scene: GameplayScenes,
    _var_map1: VarMap,
    _var_map2: VarMap,
    ai_agent1: candle_nn::Sequential,
    ai_agent2: candle_nn::Sequential,
    device: Device,
}

impl Scene for SpectateAi {
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
        let timer = match &self.scene {
            GameplayScenes::DuringRound(during_round) => during_round.timer(),
            _ => 1.0,
        };

        let observation = serialize_observation(&self.device, timer, context, state)
            .expect("Model failed to observe environment");

        // Agent1
        let ai_action = get_ai_action(&self.ai_agent1, &observation, GAMEPLAY_EPSILON)
            .expect("Model failed to exploit");
        let (dir, buttons) = map_ai_action(ai_action);
        inputs.skip_player1();
        inputs.player1.append_input(0, dir, buttons);

        state.player1_inputs.update(
            inputs.player1.held_buttons(),
            inputs.player1.parse_history(),
        );

        // Agent2
        let ai_action = get_ai_action(&self.ai_agent2, &observation, GAMEPLAY_EPSILON)
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

impl SpectateAi {
    pub fn new(left_agent_path: &str, right_agent_path: &str) -> Self {
        let mut var_map1 = VarMap::new();
        let mut var_map2 = VarMap::new();
        var_map1
            .load(left_agent_path)
            .expect("Failed to load agent");
        var_map2
            .load(right_agent_path)
            .expect("Failed to load agent");
        let agent1 = make_model(&var_map1, &Device::Cpu).expect("Failed to make agent");
        let agent2 = make_model(&var_map2, &Device::Cpu).expect("Failed to make agent");

        Self {
            scene: GameplayScenes::new_round_start((0, 0)),
            _var_map1: var_map1,
            _var_map2: var_map2,
            ai_agent1: agent1,
            ai_agent2: agent2,
            device: Device::Cpu,
        }
    }
}
