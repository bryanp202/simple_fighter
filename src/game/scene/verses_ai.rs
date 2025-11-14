use candle_core::Device;
use candle_nn::VarMap;

use crate::game::{
    GameContext, GameState, PlayerInputs,
    ai::{load_model, ppo, serialize_observation},
    scene::{
        Scene, Scenes,
        gameplay::{GameplayScene, GameplayScenes},
        main_menu::MainMenu,
    },
};

pub struct VersesAi {
    scene: GameplayScenes,
    _var_map: VarMap,
    ai_agent: candle_nn::Sequential,
    device: Device,
    rng: rand::rngs::ThreadRng,
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

        if let GameplayScenes::DuringRound(during_round) = &self.scene {
            let timer = during_round.timer();
            let observation = serialize_observation(&self.device, timer, context, state)
            .expect("Model failed to observe environment");

            let action = ppo::get_agent_action(&self.ai_agent, &observation, &mut self.rng)
                .expect("Failed to get agent action");
            ppo::take_agent_turn(&mut inputs.player2, &mut state.player2_inputs, action);
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

impl VersesAi {
    pub fn new(model_path: &str) -> Self {
        let device = Device::Cpu;
        let (_var_map, ai_agent) = load_model(model_path, &device).expect("Failed to load model");

        Self {
            scene: GameplayScenes::new_round_start((0, 0)),
            _var_map,
            ai_agent,
            device,
            rng: rand::rng(),
        }
    }
}
