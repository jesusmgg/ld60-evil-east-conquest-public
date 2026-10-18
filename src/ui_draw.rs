use macroquad::prelude::*;
use std::fmt::Write;

use crate::assets::AssetManager;
use crate::camera_manager::CameraManager;
use crate::constants::{VIEW_H, VIEW_W};
use crate::styles::Styles;
use crate::text::draw_scaled_text;
use crate::unit_design::{Resources, RESOURCE_COUNT};

pub const UI_FONT_SIZE: f32 = 16.0;
pub const UI_LINE_HEIGHT: f32 = 11.0;
pub const UI_TEXT_BASELINE: f32 = 8.0;
pub const UI_PADDING: f32 = 4.0;
const UI_ICON_SIZE: f32 = 8.0;
const UI_ICON_ADVANCE: f32 = UI_ICON_SIZE + 2.0;

pub const RESOURCE_ICON_CHARS: [char; RESOURCE_COUNT] =
    ['\u{E000}', '\u{E001}', '\u{E002}', '\u{E003}', '\u{E004}'];

fn resource_icon_index(c: char) -> Option<usize> {
    let index = (c as u32).checked_sub(RESOURCE_ICON_CHARS[0] as u32)? as usize;
    if index < RESOURCE_ICON_CHARS.len() {
        return Some(index);
    }
    None
}

pub fn write_resource_amounts(resources: &Resources, output: &mut String) {
    let start = output.len();
    let amounts = resources.amounts();
    for i in 0..amounts.len() {
        if amounts[i] == 0 {
            continue;
        }
        if output.len() > start {
            output.push(' ');
        }
        let _ = write!(output, "{}{}", amounts[i], RESOURCE_ICON_CHARS[i]);
    }
    if output.len() == start {
        output.push_str("nothing");
    }
}

pub fn virtual_mouse_position(camera_manager: &CameraManager) -> Vec2 {
    if camera_manager.ui_zoom <= 0.0 {
        return Vec2::ZERO;
    }
    let (x, y) = mouse_position();
    Vec2::new(
        (x - camera_manager.ui_offset_x) / camera_manager.ui_zoom,
        (y - camera_manager.ui_offset_y) / camera_manager.ui_zoom,
    )
}

fn plain_text_width(text: &str, assets: &AssetManager) -> f32 {
    measure_text(text, Some(&assets.font), UI_FONT_SIZE as u16, 1.0).width
}

pub fn text_width(text: &str, assets: &AssetManager) -> f32 {
    let mut width = 0.0;
    let mut segment_start = 0;
    for (i, c) in text.char_indices() {
        if resource_icon_index(c).is_some() {
            width += plain_text_width(&text[segment_start..i], assets) + UI_ICON_ADVANCE;
            segment_start = i + c.len_utf8();
        }
    }
    width + plain_text_width(&text[segment_start..], assets)
}

pub fn draw_ui_rect(rect: Rect, color: Color, camera_manager: &CameraManager) {
    let zoom = camera_manager.ui_zoom;
    draw_rectangle(
        camera_manager.ui_offset_x + rect.x * zoom,
        camera_manager.ui_offset_y + rect.y * zoom,
        rect.w * zoom,
        rect.h * zoom,
        color,
    );
}

pub fn draw_ui_rect_lines(rect: Rect, color: Color, camera_manager: &CameraManager) {
    let zoom = camera_manager.ui_zoom;
    draw_rectangle_lines(
        camera_manager.ui_offset_x + rect.x * zoom,
        camera_manager.ui_offset_y + rect.y * zoom,
        rect.w * zoom,
        rect.h * zoom,
        zoom,
        color,
    );
}

pub fn draw_ui_triangle(a: Vec2, b: Vec2, c: Vec2, color: Color, camera_manager: &CameraManager) {
    let zoom = camera_manager.ui_zoom;
    let offset = Vec2::new(camera_manager.ui_offset_x, camera_manager.ui_offset_y);
    draw_triangle(
        offset + a * zoom,
        offset + b * zoom,
        offset + c * zoom,
        color,
    );
}

pub fn draw_ui_text(
    text: &str,
    position: Vec2,
    color: Color,
    assets: &AssetManager,
    camera_manager: &CameraManager,
) {
    let mut x = position.x;
    let mut segment_start = 0;
    for (i, c) in text.char_indices() {
        if let Some(icon_i) = resource_icon_index(c) {
            let segment = &text[segment_start..i];
            draw_plain_ui_text(segment, vec2(x, position.y), color, assets, camera_manager);
            x += plain_text_width(segment, assets);
            draw_ui_icon(
                &assets.texture_resource_icons[icon_i],
                vec2(x + 1.0, position.y + UI_TEXT_BASELINE - UI_ICON_SIZE),
                Color::new(1.0, 1.0, 1.0, color.a),
                camera_manager,
            );
            x += UI_ICON_ADVANCE;
            segment_start = i + c.len_utf8();
        }
    }
    draw_plain_ui_text(
        &text[segment_start..],
        vec2(x, position.y),
        color,
        assets,
        camera_manager,
    );
}

fn draw_plain_ui_text(
    text: &str,
    position: Vec2,
    color: Color,
    assets: &AssetManager,
    camera_manager: &CameraManager,
) {
    let zoom = camera_manager.ui_zoom;
    draw_scaled_text(
        text,
        camera_manager.ui_offset_x + position.x * zoom,
        camera_manager.ui_offset_y + (position.y + UI_TEXT_BASELINE) * zoom,
        UI_FONT_SIZE * zoom,
        &color,
        &assets.font,
    );
}

fn draw_ui_icon(texture: &Texture2D, position: Vec2, tint: Color, camera_manager: &CameraManager) {
    let zoom = camera_manager.ui_zoom;
    draw_texture_ex(
        texture,
        camera_manager.ui_offset_x + position.x * zoom,
        camera_manager.ui_offset_y + position.y * zoom,
        tint,
        DrawTextureParams {
            dest_size: Some(Vec2::splat(UI_ICON_SIZE * zoom)),
            ..Default::default()
        },
    );
}

pub fn draw_panel(rect: Rect, styles: &Styles, camera_manager: &CameraManager) {
    let border = Rect::new(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0);
    draw_ui_rect(border, styles.colors.parasite_toxic, camera_manager);
    draw_ui_rect(rect, styles.colors.blue_shadow, camera_manager);
}

pub fn draw_button(
    rect: Rect,
    text: &str,
    is_enabled: bool,
    is_active: bool,
    mouse: Vec2,
    styles: &Styles,
    assets: &AssetManager,
    camera_manager: &CameraManager,
) {
    draw_panel(rect, styles, camera_manager);
    let is_hovered = rect.contains(mouse);
    if is_enabled && (is_active || is_hovered) {
        let mut light = styles.colors.cyan_bright;
        light.a = 0.35;
        draw_ui_rect(rect, light, camera_manager);
    }
    let mut color = styles.colors.cyan_bright;
    if !is_enabled {
        color = styles.colors.gray;
    }
    let text_position = Vec2::new(
        rect.x + (rect.w - text_width(text, assets)) / 2.0,
        rect.y + (rect.h - UI_LINE_HEIGHT) / 2.0,
    );
    draw_ui_text(text, text_position, color, assets, camera_manager);
}

pub fn draw_tooltip(
    text: &str,
    anchor: Vec2,
    styles: &Styles,
    assets: &AssetManager,
    camera_manager: &CameraManager,
) {
    let mut width: f32 = 0.0;
    let mut line_count = 0;
    for line in text.split('\n') {
        width = width.max(text_width(line, assets));
        line_count += 1;
    }
    let rect_w = width + UI_PADDING * 2.0;
    let rect_h = line_count as f32 * UI_LINE_HEIGHT + UI_PADDING * 2.0;
    let x = (anchor.x + 8.0).min(VIEW_W - rect_w - 1.0).max(1.0);
    let y = (anchor.y + 8.0).min(VIEW_H - rect_h - 1.0).max(1.0);
    draw_panel(Rect::new(x, y, rect_w, rect_h), styles, camera_manager);
    let mut line_y = y + UI_PADDING;
    for line in text.split('\n') {
        draw_ui_text(
            line,
            Vec2::new(x + UI_PADDING, line_y),
            styles.colors.cyan_bright,
            assets,
            camera_manager,
        );
        line_y += UI_LINE_HEIGHT;
    }
}
