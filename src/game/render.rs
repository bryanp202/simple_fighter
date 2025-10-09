use sdl3::{pixels::FColor, render::Canvas, video::Window};

use crate::game::boxes::{CollisionBox, HitBox, HurtBox};

pub mod animation;

pub fn draw_hitbox_system(canvas: &mut Canvas<Window>, hitboxes: &[HitBox]) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(1.0, 0.0, 0.0, 0.5));
    for hitbox in hitboxes {
        canvas.fill_rect(hitbox.pos())?;
    }
    Ok(())
}

pub fn draw_hurtbox_system(canvas: &mut Canvas<Window>, hurtboxes: &[HurtBox]) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(0.0, 1.0, 0.0, 0.5));
    for hurtbox in hurtboxes {
        canvas.fill_rect(hurtbox.pos())?;
    }
    Ok(())
}

pub fn draw_collisionbox_system(canvas: &mut Canvas<Window>, collisionboxes: &[CollisionBox]) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(FColor::RGBA(1.0, 1.0, 1.0, 0.5));
    for collisionbox in collisionboxes {
        canvas.fill_rect(collisionbox.pos())?;
    }
    Ok(())
}