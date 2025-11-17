use candle_core::Device;
use candle_nn::VarMap;

use crate::game::{
    GameContext, GameState, PlayerInputs,
    ai::{get_agent_action, load_model, observation_with_inv, take_agent_turn},
    scene::{
        Scene, Scenes,
        gameplay::{GameplayScene, GameplayScenes},
        main_menu::MainMenu,
    },
};

pub struct SpectateAi {
    scene: GameplayScenes,
    _var_map1: VarMap,
    _var_map2: VarMap,
    ai_agent1: candle_nn::Sequential,
    ai_agent2: candle_nn::Sequential,
    device: Device,
    rng: rand::rngs::ThreadRng,
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
        if let GameplayScenes::DuringRound(during_round) = &self.scene {
            let timer = during_round.timer();
            let (obs, obs_inv) = observation_with_inv(context, state, timer, &self.device)
                .expect("Model failed to observe environment");

            // Agent1
            let action = get_agent_action(&self.ai_agent1, &obs, &mut self.rng)
                .expect("Failed to get agent action");
            take_agent_turn(&mut inputs.player1, &mut state.player1_inputs, action);
            // Agent2
            let action = get_agent_action(&self.ai_agent2, &obs_inv, &mut self.rng)
                .expect("Failed to get agent action");
            take_agent_turn(&mut inputs.player2, &mut state.player2_inputs, action);
        }

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
        let device = Device::Cpu;
        let (_var_map1, ai_agent1) =
            load_model(left_agent_path, &device).expect("Failed to load model");
        let (_var_map2, ai_agent2) =
            load_model(right_agent_path, &device).expect("Failed to load model");

        Self {
            scene: GameplayScenes::new_round_start((0, 0)),
            _var_map1,
            _var_map2,
            ai_agent1,
            ai_agent2,
            device,
            rng: rand::rng(),
        }
    }
}
