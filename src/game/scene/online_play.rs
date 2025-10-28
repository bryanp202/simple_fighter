use crate::{
    game::{
        FRAME_DURATION, GameContext, GameState, MAX_ROLLBACK_FRAMES, PlayerInputs, Side,
        net::UdpStream,
        scene::{
            Scene, Scenes,
            gameplay::{GameplayScene, GameplayScenes},
            main_menu::MainMenu,
        },
    },
    ring_buf::RingBuf,
};

pub struct OnlinePlay {
    local_side: Side,
    scene: GameplayScenes,
    game_state_history: RingBuf<(GameplayScenes, GameState), MAX_ROLLBACK_FRAMES>,
    // Net code
    connection: UdpStream,
    current_frame: usize,
    delay: usize,
}

impl Scene for OnlinePlay {
    fn enter(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) {
        inputs.set_delay(self.delay);
        self.scene.enter(context, state);
    }

    fn handle_input(
        &mut self,
        context: &GameContext,
        inputs: &mut crate::game::PlayerInputs,
        state: &mut GameState,
    ) -> std::io::Result<()> {
        let (local_inputs, peer_inputs) = match self.local_side {
            Side::Left => {
                inputs.update_player1();
                inputs.skip_player2();
                (&inputs.player1, &mut inputs.player2)
            }
            Side::Right => {
                inputs.skip_player1();
                inputs.update_player2();
                (&inputs.player2, &mut inputs.player1)
            }
        };
        let (rollback, fastforward) =
            self.connection
                .update(self.current_frame, local_inputs, peer_inputs)?;
        self.rollback(context, inputs, state, rollback);
        self.fastforward(context, inputs, state, fastforward);
        self.current_frame += fastforward;

        Ok(())
    }

    fn update(
        &mut self,
        context: &GameContext,
        state: &mut GameState,
        dt: f32,
    ) -> Option<super::Scenes> {
        if let Some(new_scene) = self.scene.update(context, state, dt) {
            self.scene.exit(context, state);
            self.scene = new_scene;
            self.scene.enter(context, state);
        }

        self.current_frame += 1;

        self.append_game_snapshot(state);

        match self.scene {
            GameplayScenes::Exit => Some(Scenes::MainMenu(MainMenu::new())),
            _ => None,
        }
    }

    fn render(
        &self,
        canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
        global_textures: &Vec<sdl3::render::Texture>,
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error> {
        self.scene.render(canvas, global_textures, context, state)
    }

    fn exit(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) {
        inputs.set_delay(0);
        self.scene.exit(context, state);
    }
}

impl OnlinePlay {
    pub fn new(
        connection: UdpStream,
        local_side: Side,
        state: &GameState,
        current_frame: usize,
    ) -> Self {
        let scene = GameplayScenes::new_round_start((0, 0));
        let initial_state = (scene.clone(), state.clone());
        Self {
            local_side,
            connection,
            scene,
            current_frame,
            game_state_history: RingBuf::new(initial_state),
            delay: 3,
        }
    }

    fn rollback(
        &mut self,
        context: &GameContext,
        inputs: &PlayerInputs,
        state: &mut GameState,
        frames: usize,
    ) {
        if frames < self.delay {
            return;
        }
        let frames = frames - self.delay;
        println!("rolling back: {}", frames);

        let (old_scene, old_state) = self.game_state_history.rewind(frames);
        self.scene = old_scene;
        *state = old_state;

        self.fast_simulate(context, inputs, state, frames);
    }

    fn fast_simulate(
        &mut self,
        context: &GameContext,
        inputs: &PlayerInputs,
        state: &mut GameState,
        frames: usize,
    ) {
        for frame in (1..frames + 1).rev() {
            state
                .player1_inputs
                .update(inputs.player1.parse_history_at(frame));
            state
                .player2_inputs
                .update(inputs.player2.parse_history_at(frame));

            if let Some(mut new_scene) = self.scene.update(context, state, FRAME_DURATION) {
                self.scene.exit(context, state);
                new_scene.enter(context, state);
                self.scene = new_scene;
            }

            self.append_game_snapshot(state);
        }
    }

    fn fastforward(
        &mut self,
        context: &GameContext,
        inputs: &mut PlayerInputs,
        state: &mut GameState,
        frames: usize,
    ) {
        if cfg!(feature = "debug") {
            if frames > 0 {
                println!("fastforwarding: {}", frames);
            }
        }

        for _ in 0..frames {
            state.player1_inputs.update(inputs.player1.parse_history());
            state.player2_inputs.update(inputs.player2.parse_history());

            if let Some(mut new_scene) = self.scene.update(context, state, FRAME_DURATION) {
                self.scene.exit(context, state);
                new_scene.enter(context, state);
                self.scene = new_scene;
            }

            self.append_game_snapshot(state);

            inputs.skip_player1();
            inputs.skip_player2();
        }
    }

    fn append_game_snapshot(&mut self, state: &GameState) {
        self.game_state_history
            .append((self.scene.clone(), state.clone()));
    }
}
