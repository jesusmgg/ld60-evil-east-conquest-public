use macroquad::prelude::*;

use crate::constants::{TILE_SIZE_X, TILE_SIZE_Y};
use crate::sprite_sheet::sheet_frame_rect;

pub const FLAME_ANIMATION_FPS: f32 = 6.0;

#[derive(Clone, Copy, PartialEq)]
pub enum DamageFlame {
    Dim,
    Bright,
}

fn draw_flame(texture: &Texture2D, source: Rect, pos: Vec2, tint: Color) {
    draw_texture_ex(
        texture,
        pos.x,
        pos.y,
        tint,
        DrawTextureParams {
            source: Some(source),
            ..Default::default()
        },
    );
}

pub fn draw_flame_on_tile(
    texture: &Texture2D,
    frames_shown: usize,
    tile_world_pos: Vec2,
    tint: Color,
) {
    let source = sheet_frame_rect(texture, frames_shown);
    let centering = (vec2(TILE_SIZE_X, TILE_SIZE_Y) - source.size()) / 2.0;
    draw_flame(texture, source, tile_world_pos + centering, tint);
}

pub fn draw_flame_at_base(texture: &Texture2D, frames_shown: usize, base: Vec2) {
    let source = sheet_frame_rect(texture, frames_shown);
    let pos = base - vec2(source.w / 2.0, source.h);
    draw_flame(texture, source, pos, WHITE);
}
