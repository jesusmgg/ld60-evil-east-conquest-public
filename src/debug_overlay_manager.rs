use macroquad::color::{Color, WHITE};
#[cfg(debug_assertions)]
use macroquad::input::{is_key_pressed, KeyCode};
#[cfg(debug_assertions)]
use macroquad::math::IVec2;
use macroquad::prelude::get_fps;
use macroquad::shapes::draw_rectangle;
use macroquad::text::measure_text;

use crate::assets::AssetManager;
use crate::camera_manager::CameraManager;
use crate::constants::*;
use crate::debug::DebugLog;
use crate::level::level_editor_manager::LevelEditorManager;
use crate::level::level_manager::LevelManager;
use crate::minimap_manager::MINIMAP_BOTTOM;
#[cfg(debug_assertions)]
use crate::player_manager::PlayerManager;
use crate::styles::Styles;
use crate::text::draw_scaled_text;
use crate::unit_manager::UnitManager;

#[cfg(debug_assertions)]
use std::fmt::Write;

const DEBUG_BUILD_VERSION: &str = concat!("DEBUG BUILD - v", env!("CARGO_PKG_VERSION"));

pub struct DebugOverlayManager {
    pub is_ui_visible: bool,
    #[cfg(debug_assertions)]
    pub is_log_visible: bool,
    pub is_lighting_enabled: bool,
    pub debug_log: DebugLog,
    pub fps_avg: i32,
    fps_frame_count: i32,
    fps_time_accum: f32,
    input_toggle_ui: bool,
    input_toggle_lighting: bool,
    input_toggle_log: bool,
    #[cfg(debug_assertions)]
    tile_index_text: String,
}

impl DebugOverlayManager {
    pub fn new() -> Self {
        Self {
            is_ui_visible: cfg!(debug_assertions),
            #[cfg(debug_assertions)]
            is_log_visible: cfg!(debug_assertions),
            is_lighting_enabled: true,
            debug_log: DebugLog::new(),
            fps_avg: 0,
            fps_frame_count: 0,
            fps_time_accum: 0.0,
            input_toggle_ui: false,
            input_toggle_lighting: false,
            input_toggle_log: false,
            #[cfg(debug_assertions)]
            tile_index_text: String::new(),
        }
    }

    #[cfg(debug_assertions)]
    pub fn input(&mut self, level_editor: &LevelEditorManager, player_manager: &PlayerManager) {
        if level_editor.is_editing_properties() || player_manager.is_keyboard_captured() {
            return;
        }
        self.input_toggle_ui = is_key_pressed(KeyCode::Tab);
        self.input_toggle_lighting = is_key_pressed(KeyCode::L);
        self.input_toggle_log =
            is_key_pressed(KeyCode::GraveAccent) || is_key_pressed(KeyCode::Apostrophe);
    }

    pub fn update(&mut self, dt: f32) {
        #[cfg(debug_assertions)]
        {
            if self.input_toggle_ui {
                self.is_ui_visible = !self.is_ui_visible;
            }
            if self.input_toggle_lighting {
                self.is_lighting_enabled = !self.is_lighting_enabled;
            }
            if self.input_toggle_log {
                self.is_log_visible = !self.is_log_visible;
            }
        }
        self.input_toggle_ui = false;
        self.input_toggle_lighting = false;
        self.input_toggle_log = false;

        self.fps_frame_count += 1;
        self.fps_time_accum += dt;

        if self.fps_time_accum >= 1.0 {
            self.fps_avg = (self.fps_frame_count as f32 / self.fps_time_accum) as i32;
            self.fps_frame_count = 0;
            self.fps_time_accum = 0.0;
        }
    }

    #[cfg(debug_assertions)]
    pub fn render_world(&self, styles: &Styles, level_manager: &LevelManager) {
        if !self.is_ui_visible {
            return;
        }

        let mut color1 = styles.colors.blue_shadow;
        color1.a = 0.05;
        let mut color2 = styles.colors.cyan_med;
        color2.a = 0.05;

        if let Some(active_pos) = level_manager.view_sector_pos {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let neighbor_pos = IVec2::new(active_pos.x + dx, active_pos.y + dy);

                    if let Some(level) = level_manager.get_level_at_pos(neighbor_pos) {
                        for ty in 0..level.grid_tiles.y {
                            for tx in 0..level.grid_tiles.x {
                                let tile_pos = IVec2::new(tx, ty);
                                let world_pos = level.tile_to_world_pos(neighbor_pos, tile_pos);

                                let color = if (tx + ty) % 2 == 0 { color1 } else { color2 };

                                draw_rectangle(
                                    world_pos.x,
                                    world_pos.y,
                                    TILE_SIZE_X,
                                    TILE_SIZE_Y,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    pub fn ui_tile_indices(
        &mut self,
        asset_manager: &AssetManager,
        camera_manager: &CameraManager,
        level_manager: &LevelManager,
    ) {
        if !self.is_ui_visible {
            return;
        }

        if let Some(level_pos) = level_manager.view_sector_pos {
            if let Some(level) = level_manager.current_level() {
                let font_size = 8.0;

                let zoom = camera_manager.ui_zoom;
                let x_offset = camera_manager.ui_offset_x;
                let y_offset = camera_manager.ui_offset_y;

                let camera = &camera_manager.camera;
                let camera_offset_x = camera.target.x - VIEW_W / 2.0;
                let camera_offset_y = camera.target.y - VIEW_H / 2.0;

                for y in 0..level.grid_tiles.y {
                    for x in 0..level.grid_tiles.x {
                        let tile_pos = IVec2::new(x, y);
                        let world_pos = level.tile_to_world_pos(level_pos, tile_pos);
                        let world_x = world_pos.x;
                        let world_y = world_pos.y;

                        let screen_x = x_offset + ((world_x - camera_offset_x) * zoom);
                        let screen_y = y_offset + ((world_y - camera_offset_y) * zoom);

                        self.tile_index_text.clear();
                        let _ = write!(self.tile_index_text, "{},{}", x, y);
                        let text_offset_x = 0.0;
                        let text_offset_y = font_size / 2.0;
                        let mut color = WHITE;
                        color.a = 0.3;

                        draw_scaled_text(
                            &self.tile_index_text,
                            screen_x + (text_offset_x * zoom),
                            screen_y + (text_offset_y * zoom),
                            font_size * zoom,
                            &color,
                            &asset_manager.font,
                        );
                    }
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    pub fn ui_debug_log(&self, asset_manager: &AssetManager, camera_manager: &CameraManager) {
        self.debug_log.ui(
            self.is_log_visible,
            &asset_manager.font,
            camera_manager.ui_zoom,
            camera_manager.ui_offset_x,
            camera_manager.ui_offset_y,
        );
    }

    pub fn ui_diagnostics(
        &self,
        styles: &Styles,
        asset_manager: &AssetManager,
        camera_manager: &CameraManager,
        level_manager: &LevelManager,
        unit_manager: &UnitManager,
        level_editor: &LevelEditorManager,
    ) {
        if !self.is_ui_visible {
            return;
        }

        self.ui_debug_build_indicator(styles, asset_manager, camera_manager);

        let zoom = camera_manager.ui_zoom;
        let x_offset = camera_manager.ui_offset_x;
        let y_offset = camera_manager.ui_offset_y;

        let font_size = 8.0;
        let line_height = 8.0;
        let padding = 4.0;
        let x = VIEW_W - 4.0;
        let y = MINIMAP_BOTTOM + padding * 2.0;

        let current_level_name = match &level_manager.current_level() {
            Some(level) => level.name.as_str(),
            None => "-",
        };

        let current_level_pos = match level_manager.view_sector_pos {
            Some(pos) => pos,
            None => return,
        };

        let level_text = if level_editor.is_enabled {
            let initial = match &level_manager.level_set {
                Some(ls) => ls.initial_level_name.as_str(),
                None => "-",
            };
            format!(
                "Pos: ({}, {}) | Level: {} | Initial: {}",
                current_level_pos.x, current_level_pos.y, current_level_name, initial
            )
        } else {
            format!(
                "Level: {} ({}, {})",
                current_level_name, current_level_pos.x, current_level_pos.y
            )
        };

        let texts = vec![
            format!("FPS: {} ({})", get_fps(), self.fps_avg),
            level_text,
            format!("Level editor: {}", level_editor.is_enabled),
            format!("Lighting: {}", self.is_lighting_enabled),
            format!("Turn: {}", unit_manager.turn_count),
        ];

        let mut max_width = 0.0;
        for text in &texts {
            let text_dimensions =
                measure_text(text, Some(&asset_manager.font), font_size as u16, 1.0);
            if text_dimensions.width > max_width {
                max_width = text_dimensions.width;
            }
        }

        let bg_width = max_width + padding * 2.0;
        let bg_height = ((texts.len() - 1) as f32 * line_height) + padding * 2.0;
        let bg_x = x - bg_width;
        let bg_y = y - padding;

        let screen_bg_x = x_offset + (bg_x * zoom);
        let screen_bg_y = y_offset + (bg_y * zoom);
        let screen_bg_width = bg_width * zoom;
        let screen_bg_height = bg_height * zoom;

        let bg_color = Color::from_rgba(0, 0, 0, 128);
        draw_rectangle(
            screen_bg_x,
            screen_bg_y,
            screen_bg_width,
            screen_bg_height,
            bg_color,
        );

        let mut text_color = WHITE;
        text_color.a = 1.0;
        let mut text_y = y;

        for text in &texts {
            let text_dimensions =
                measure_text(text, Some(&asset_manager.font), font_size as u16, 1.0);
            let text_x = x - text_dimensions.width - padding;

            let screen_text_x = x_offset + (text_x * zoom);
            let screen_text_y = y_offset + (text_y * zoom);

            draw_scaled_text(
                text,
                screen_text_x,
                screen_text_y,
                font_size * zoom,
                &text_color,
                &asset_manager.font,
            );
            text_y += line_height;
        }
    }

    fn ui_debug_build_indicator(
        &self,
        styles: &Styles,
        asset_manager: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        let zoom = camera_manager.ui_zoom;
        let x_offset = camera_manager.ui_offset_x;
        let y_offset = camera_manager.ui_offset_y;

        let text = DEBUG_BUILD_VERSION;
        let font_size = 12.0;
        let text_x = 4.0;
        let text_y = VIEW_H - 4.0;

        let screen_text_x = x_offset + (text_x * zoom);
        let screen_text_y = y_offset + (text_y * zoom);

        draw_scaled_text(
            text,
            screen_text_x,
            screen_text_y,
            font_size * zoom,
            &styles.colors.cyan_bright,
            &asset_manager.font,
        );
    }
}
