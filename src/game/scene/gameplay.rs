mod during_round;
mod round_start;

use sdl3::{
    pixels::Color,
    render::{Canvas, FRect, Texture},
    video::Window,
};

use crate::game::{
    FRAME_RATE, GameContext, GameState, SCORE_TO_WIN,
    render::animation::Animation,
    scene::gameplay::{during_round::DuringRound, round_start::RoundStart},
};

const ROUND_LEN: usize = 99;

pub trait GameplayScene {
    fn enter(&mut self, context: &GameContext, state: &mut GameState);
    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Option<GameplayScenes>;
    fn render(
        &self,
        canvas: &mut Canvas<Window>,
        global_textures: &[Texture],
        context: &GameContext,
        state: &GameState,
    ) -> Result<(), sdl3::Error>;
    fn exit(&mut self, context: &GameContext, state: &mut GameState);
}

#[derive(Clone, PartialEq)]
pub enum GameplayScenes {
    RoundStart(RoundStart),
    DuringRound(DuringRound),
    Exit,
}

impl GameplayScenes {
    pub fn new_round_start(score: (u32, u32)) -> GameplayScenes {
        Self::RoundStart(RoundStart::new(score))
    }
}

impl GameplayScene for GameplayScenes {
    fn enter(&mut self, context: &GameContext, state: &mut GameState) {
        match self {
            Self::DuringRound(during_round) => during_round.enter(context, state),
            Self::RoundStart(round_start) => round_start.enter(context, state),
            Self::Exit => {}
        }
    }

    fn update(&mut self, context: &GameContext, state: &mut GameState) -> Option<GameplayScenes> {
        match self {
            Self::DuringRound(during_round) => during_round.update(context, state),
            Self::RoundStart(round_start) => round_start.update(context, state),
            Self::Exit => None,
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
            Self::DuringRound(during_round) => {
                during_round.render(canvas, global_textures, context, state)
            }
            Self::RoundStart(round_start) => {
                round_start.render(canvas, global_textures, context, state)
            }
            Self::Exit => Ok(()),
        }
    }

    fn exit(&mut self, context: &GameContext, state: &mut GameState) {
        match self {
            Self::DuringRound(during_round) => during_round.exit(context, state),
            Self::RoundStart(round_start) => round_start.exit(context, state),
            Self::Exit => {}
        }
    }
}

// impl GameplayScene for GameplayScenes {
//     fn enter(&mut self, context: &GameContext, state: &mut GameState) {
//         match self {
//             Self::RoundStart(round_start) => round_start.enter()
//         }
//     }
// }

fn render_gameplay(
    canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
    global_textures: &[sdl3::render::Texture],
    context: &GameContext,
    state: &GameState,
    time: usize,
    score: (u32, u32),
) -> Result<(), sdl3::Error> {
    context.stage.render(canvas, global_textures)?;
    state
        .player1
        .render(canvas, &context.camera, global_textures, &context.player1)?;
    state
        .player2
        .render(canvas, &context.camera, global_textures, &context.player2)?;

    let player1_hp_per = state.player1.hp_per(&context.player1);
    let player2_hp_per = state.player2.hp_per(&context.player2);
    render_health_bars(canvas, player1_hp_per, player2_hp_per)?;
    render_timer(canvas, global_textures, &context.timer_animation, time)?;
    render_scores(canvas, score)?;

    Ok(())
}

fn render_timer(
    canvas: &mut Canvas<Window>,
    global_textures: &[Texture],
    timer_animation: &Animation,
    time: usize,
) -> Result<(), sdl3::Error> {
    let (screen_w, screen_h) = canvas.window().size();
    let frame = time / FRAME_RATE;
    let (texture, src) = timer_animation.get_frame(frame, global_textures);

    let timer_w = screen_w as f32 / 10.0;
    let timer_h = screen_h as f32 / 5.625;
    let dst = FRect::new(screen_w as f32 * 0.5 - timer_w / 2.0, 0.0, timer_w, timer_h);
    canvas.copy(texture, src, dst)
}

fn render_scores(canvas: &mut Canvas<Window>, score: (u32, u32)) -> Result<(), sdl3::Error> {
    let (screen_w, screen_h) = canvas.window().size();
    let y = screen_h as f32 / 15.0;
    let score_w = screen_w as f32 / 40.0;
    let score_h = screen_h as f32 / 22.5;

    let player1_offset = screen_w as f32 * 0.5 - score_w * (2 * SCORE_TO_WIN + 3) as f32;
    let player2_offset = screen_w as f32 * 0.5 + score_w * 4.0;
    render_player1_score(canvas, score.0, y, score_w, score_h, player1_offset)?;
    render_player2_score(canvas, score.1, y, score_w, score_h, player2_offset)?;
    Ok(())
}

fn render_player1_score(
    canvas: &mut Canvas<Window>,
    score: u32,
    y: f32,
    w: f32,
    h: f32,
    x: f32,
) -> Result<(), sdl3::Error> {
    for i in 0..SCORE_TO_WIN {
        let i_f32 = i as f32;
        canvas.set_draw_color(Color::BLACK);
        canvas.fill_rect(FRect::new(x + 2.0 * i_f32 * w, y, w, h))?;

        if score > i {
            canvas.set_draw_color(Color::WHITE);
            canvas.fill_rect(FRect::new(
                x + 2.0 * i_f32 * w + w * 0.2,
                y + h * 0.2,
                w * 0.6,
                h * 0.6,
            ))?;
        }
    }

    Ok(())
}

fn render_player2_score(
    canvas: &mut Canvas<Window>,
    score: u32,
    y: f32,
    w: f32,
    h: f32,
    x: f32,
) -> Result<(), sdl3::Error> {
    for i in 0..SCORE_TO_WIN {
        let i_f32 = i as f32;
        canvas.set_draw_color(Color::BLACK);
        canvas.fill_rect(FRect::new(x + 2.0 * i_f32 * w, y, w, h))?;

        if score >= SCORE_TO_WIN - i {
            canvas.set_draw_color(Color::WHITE);
            canvas.fill_rect(FRect::new(
                x + 2.0 * i_f32 * w + w * 0.2,
                y + h * 0.2,
                w * 0.6,
                h * 0.6,
            ))?;
        }
    }

    Ok(())
}

fn render_health_bars(
    canvas: &mut Canvas<Window>,
    player1_hp_per: f32,
    player2_hp_per: f32,
) -> Result<(), sdl3::Error> {
    let (screen_w, screen_h) = canvas.window().size();
    let bar_h = screen_h as f32 / 20.0;
    let bar_width = screen_w as f32 * 0.4;
    render_player1_health(canvas, player1_hp_per, bar_h, bar_width)?;
    render_player2_health(canvas, player2_hp_per, screen_w as f32, bar_h, bar_width)?;
    Ok(())
}

fn render_player1_health(
    canvas: &mut Canvas<Window>,
    hp_per: f32,
    bar_h: f32,
    bar_width: f32,
) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(Color::RED);
    canvas.fill_rect(FRect::new(0.0, 0.0, bar_width, bar_h))?;
    canvas.set_draw_color(Color::GREEN);
    let health_bar = hp_per.powf(1.4) * bar_width;
    canvas.fill_rect(FRect::new(bar_width - health_bar, 0.0, health_bar, bar_h))?;

    Ok(())
}

fn render_player2_health(
    canvas: &mut Canvas<Window>,
    hp_per: f32,
    screen_w: f32,
    bar_h: f32,
    bar_width: f32,
) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(Color::RED);
    canvas.fill_rect(FRect::new(screen_w - bar_width, 0.0, bar_width, bar_h))?;
    canvas.set_draw_color(Color::GREEN);
    let health_bar = hp_per.powf(1.4) * bar_width;
    canvas.fill_rect(FRect::new(screen_w - bar_width, 0.0, health_bar, bar_h))?;

    Ok(())
}
