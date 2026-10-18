use macroquad::color::Color;
use macroquad::input::{get_last_key_pressed, is_mouse_button_pressed, MouseButton};
use macroquad::shapes::draw_rectangle;
use macroquad::window::{screen_height, screen_width};

use crate::assets::{ui_loading_screen, AssetManager, LoadingProgress};
use crate::camera_manager::CameraManager;
use crate::constants::*;
use crate::loading_manager::LoadingManager;
use crate::styles::Styles;
use crate::text::draw_scaled_text;

pub struct UiManager {
    pub message: Option<String>,
    input_dismiss: bool,
}

impl UiManager {
    pub fn new() -> Self {
        Self {
            message: None,
            input_dismiss: false,
        }
    }

    pub fn input(&mut self) {
        self.input_dismiss =
            is_mouse_button_pressed(MouseButton::Left) || get_last_key_pressed().is_some();
    }

    pub fn update(&mut self) {
        if self.message.is_some() && self.input_dismiss {
            self.message = None;
        }
        self.input_dismiss = false;
    }

    pub fn ui_loading(
        &self,
        loading_manager: &LoadingManager,
        styles: &Styles,
        asset_manager: &AssetManager,
    ) {
        let progress = LoadingProgress {
            progress: loading_manager.initialization_progress,
            text: loading_manager.initialization_message.to_string(),
        };

        ui_loading_screen(&progress, styles, &asset_manager.font);
    }

    pub fn ui(
        &self,
        styles: &Styles,
        asset_manager: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        #[cfg(not(debug_assertions))]
        self.ui_version(asset_manager, camera_manager);
        self.ui_message(styles, asset_manager, camera_manager);
    }

    fn ui_message(
        &self,
        styles: &Styles,
        asset_manager: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        if let Some(message) = &self.message {
            let zoom = camera_manager.ui_zoom;
            let x_offset = camera_manager.ui_offset_x;
            let y_offset = camera_manager.ui_offset_y;

            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::new(0.0, 0.0, 0.0, 0.7),
            );

            let lines: Vec<&str> = message.split('\n').collect();
            let line_height = 16.0;
            let empty_line_height = 10.0;

            let mut total_height = 40.0;
            for line in &lines {
                if line.is_empty() {
                    total_height += empty_line_height;
                } else {
                    total_height += line_height;
                }
            }

            let box_width = 280.0;
            let box_height = total_height;
            let box_x = (VIEW_W - box_width) / 2.0;
            let box_y = (VIEW_H - box_height) / 2.0;

            let screen_box_x = x_offset + (box_x * zoom);
            let screen_box_y = y_offset + (box_y * zoom);

            draw_rectangle(
                screen_box_x - 2.0 * zoom,
                screen_box_y - 2.0 * zoom,
                (box_width + 4.0) * zoom,
                (box_height + 4.0) * zoom,
                styles.colors.parasite_toxic,
            );

            draw_rectangle(
                screen_box_x,
                screen_box_y,
                box_width * zoom,
                box_height * zoom,
                styles.colors.blue_shadow,
            );

            let font_size = 16.0;
            let text_x = box_x + 10.0;
            let mut text_y = box_y + 20.0;

            for line in lines {
                let screen_text_x = x_offset + (text_x * zoom);
                let screen_text_y = y_offset + (text_y * zoom);

                draw_scaled_text(
                    line,
                    screen_text_x,
                    screen_text_y,
                    font_size * zoom,
                    &styles.colors.cyan_bright,
                    &asset_manager.font,
                );

                if line.is_empty() {
                    text_y += empty_line_height;
                } else {
                    text_y += line_height;
                }
            }
        }
    }

    #[cfg(not(debug_assertions))]
    fn ui_version(&self, asset_manager: &AssetManager, camera_manager: &CameraManager) {
        const VERSION_TEXT: &str = concat!("v", env!("CARGO_PKG_VERSION"));
        let zoom = camera_manager.ui_zoom;
        let x_offset = camera_manager.ui_offset_x;
        let y_offset = camera_manager.ui_offset_y;
        let text = VERSION_TEXT;
        let font_size = 10.0;
        let margin = 4.0;
        let x = x_offset + margin * zoom;
        let y = y_offset + (VIEW_H - margin) * zoom;
        let color = Color::new(0.5, 0.5, 0.5, 0.15);
        draw_scaled_text(text, x, y, font_size * zoom, &color, &asset_manager.font);
    }
}
