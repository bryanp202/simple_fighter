use sdl3::{
    pixels::Color,
    render::{Canvas, FRect, Texture},
    video::Window,
};

use crate::game::{
    FRAME_RATE, GameContext, SCORE_TO_WIN,
    render::animation::Animation,
    scene::{gameplay::Gameplay, main_menu::MainMenu, round_start::RoundStart},
};

mod gameplay;
mod main_menu;
mod round_start;

pub trait Scene {
    fn enter(&mut self, context: &mut GameContext);
    fn update(&mut self, context: &mut GameContext, dt: f32) -> Option<Scenes>;
    fn render(
        &self,
        context: &GameContext,
        canvas: &mut Canvas<Window>,
        global_textures: &Vec<Texture>,
    ) -> Result<(), sdl3::Error>;
    fn exit(&mut self, context: &mut GameContext);
}

pub enum Scenes {
    MainMenu(MainMenu),
    RoundStart(RoundStart),
    Gameplay(Gameplay),
    //RoundEnd,
    //WinScreen,
    //Settings,
}

impl Scene for Scenes {
    fn enter(&mut self, context: &mut GameContext) -> () {
        match self {
            Self::MainMenu(main_menu) => main_menu.enter(context),
            Self::RoundStart(round_start) => round_start.enter(context),
            Self::Gameplay(gameplay) => gameplay.enter(context),
        }
    }

    fn update(&mut self, context: &mut GameContext, dt: f32) -> Option<Scenes> {
        match self {
            Self::MainMenu(main_menu) => main_menu.update(context, dt),
            Self::RoundStart(round_start) => round_start.update(context, dt),
            Self::Gameplay(gameplay) => gameplay.update(context, dt),
        }
    }

    fn render(
        &self,
        context: &GameContext,
        canvas: &mut Canvas<Window>,
        global_textures: &Vec<Texture>,
    ) -> Result<(), sdl3::Error> {
        match self {
            Self::MainMenu(main_menu) => main_menu.render(context, canvas, global_textures),
            Self::RoundStart(round_start) => round_start.render(context, canvas, global_textures),
            Self::Gameplay(gameplay) => gameplay.render(context, canvas, global_textures),
        }
    }

    fn exit(&mut self, context: &mut GameContext) {
        match self {
            Self::MainMenu(main_menu) => main_menu.exit(context),
            Self::RoundStart(round_start) => round_start.exit(context),
            Self::Gameplay(gameplay) => gameplay.exit(context),
        }
    }
}

impl Scenes {
    pub fn new() -> Self {
        Self::MainMenu(MainMenu::new())
    }
}

fn render_gameplay(
    context: &GameContext,
    canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
    global_textures: &Vec<sdl3::render::Texture>,
    time: usize,
    score: (u32, u32),
) -> Result<(), sdl3::Error> {
    context.stage.render(canvas, global_textures)?;
    context
        .player1
        .render(canvas, &context.camera, global_textures)?;
    context
        .player2
        .render(canvas, &context.camera, global_textures)?;

    let player1_hp_per = context.player1.current_hp() / context.player1.max_hp();
    let player2_hp_per = context.player2.current_hp() / context.player2.max_hp();
    render_health_bars(canvas, player1_hp_per, player2_hp_per)?;
    render_timer(canvas, global_textures, &context.timer_animation, time)?;
    render_scores(canvas, score)?;

    Ok(())
}

fn render_timer(
    canvas: &mut Canvas<Window>,
    global_textures: &Vec<Texture>,
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
    let health_bar = hp_per * bar_width;
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
    canvas.fill_rect(FRect::new(
        screen_w as f32 - bar_width,
        0.0,
        bar_width,
        bar_h,
    ))?;
    canvas.set_draw_color(Color::GREEN);
    let health_bar = hp_per * bar_width;
    canvas.fill_rect(FRect::new(
        screen_w as f32 - bar_width,
        0.0,
        health_bar,
        bar_h,
    ))?;

    Ok(())
}
