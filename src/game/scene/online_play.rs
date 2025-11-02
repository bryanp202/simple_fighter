use crate::{
    game::{
        GameContext, GameState, MAX_ROLLBACK_FRAMES, PlayerInputs, Side,
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
        self.rollback(context, inputs, state, rollback, fastforward);
        self.fast_forward(context, inputs, state, fastforward);

        Ok(())
    }

    fn update(
        &mut self,
        context: &GameContext,
        state: &mut GameState,
    ) -> Option<super::Scenes> {
        if self.connection.is_aborted() {
            return Some(Scenes::MainMenu(MainMenu::new()));
        }

        if let Some(new_scene) = self.scene.update(context, state) {
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
        global_textures: &[sdl3::render::Texture],
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error> {
        self.scene.render(canvas, global_textures, context, state)
    }

    fn exit(&mut self, context: &GameContext, inputs: &mut PlayerInputs, state: &mut GameState) {
        _ = self.connection.abort(self.current_frame);
        inputs.set_delay(0);

        match self.local_side {
            Side::Left => inputs.reset_player2(),
            Side::Right => inputs.reset_player1(),
        }

        self.scene.exit(context, state);
    }
}

impl OnlinePlay {
    pub fn new(connection: UdpStream, local_side: Side, state: &GameState) -> Self {
        let scene = GameplayScenes::new_round_start((0, 0));
        let initial_state = (scene.clone(), state.clone());
        Self {
            local_side,
            connection,
            scene,
            current_frame: 0,
            game_state_history: RingBuf::new(initial_state),
            delay: 3,
        }
    }

    fn rollback(
        &mut self,
        context: &GameContext,
        inputs: &PlayerInputs,
        state: &mut GameState,
        rollback_frames: usize,
        fastforward_frames: usize,
    ) {
        if rollback_frames <= self.delay {
            return;
        }
        let frames = rollback_frames - self.delay;

        if cfg!(feature = "debug") {
            println!("rolling back: {frames}");
        }

        let (old_scene, old_state) = self.game_state_history.rewind(frames);
        self.scene = old_scene;
        *state = old_state;

        self.fast_simulate(context, inputs, state, frames, fastforward_frames);
    }

    fn fast_simulate(
        &mut self,
        context: &GameContext,
        inputs: &PlayerInputs,
        state: &mut GameState,
        frames: usize,
        offset: usize,
    ) {
        for frame in (1..frames + 1).rev() {
            state.player1_inputs.update(
                inputs.player1.held_buttons(),
                inputs.player1.parse_history_at(frame + offset),
            );
            state.player2_inputs.update(
                inputs.player2.held_buttons(),
                inputs.player2.parse_history_at(frame + offset),
            );

            if let Some(mut new_scene) = self.scene.update(context, state) {
                self.scene.exit(context, state);
                new_scene.enter(context, state);
                self.scene = new_scene;
            }

            self.append_game_snapshot(state);
        }
    }

    fn fast_forward(
        &mut self,
        context: &GameContext,
        inputs: &mut PlayerInputs,
        state: &mut GameState,
        frames: usize,
    ) {
        if cfg!(feature = "debug") {
            if frames > 0 {
                println!("Fastfowarding: {frames} frames");
            }
        }

        match self.local_side {
            Side::Left => inputs.player1.skip_for(frames),
            Side::Right => inputs.player2.skip_for(frames),
        }

        for frame in (1..frames + 1).rev() {
            state.player1_inputs.update(
                inputs.player1.held_buttons(),
                inputs.player1.parse_history_at(frame),
            );
            state.player2_inputs.update(
                inputs.player2.held_buttons(),
                inputs.player2.parse_history_at(frame),
            );

            if let Some(mut new_scene) = self.scene.update(context, state) {
                self.scene.exit(context, state);
                new_scene.enter(context, state);
                self.scene = new_scene;
            }

            self.append_game_snapshot(state);
        }

        self.current_frame += frames;
    }

    fn append_game_snapshot(&mut self, state: &GameState) {
        self.game_state_history
            .append((self.scene.clone(), state.clone()));
    }
}
