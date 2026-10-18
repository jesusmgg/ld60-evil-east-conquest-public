use macroquad::camera::{set_camera, Camera2D};
use macroquad::input::mouse_position;
use macroquad::math::{f32, Rect};
use macroquad::window::{screen_height, screen_width};

use crate::constants::*;
use crate::loading_manager::LoadingManager;

const VIEW_ZOOM_MIN: f32 = 0.01;

pub struct CameraManager {
    pub camera: Camera2D,
    pub target_pos: f32::Vec2,
    pub is_initialized: bool,
    pub ui_zoom: f32,
    pub ui_offset_x: f32,
    pub ui_offset_y: f32,
    pub mouse_world_pos: f32::Vec2,
}

impl CameraManager {
    pub fn new() -> Self {
        let camera = Camera2D {
            target: f32::vec2(VIEW_W / 2.0, VIEW_H / 2.0),
            ..Default::default()
        };
        let target_pos = camera.target;

        let mut camera_manager = Self {
            camera,
            target_pos,
            is_initialized: false,
            ui_zoom: 1.0,
            ui_offset_x: 0.0,
            ui_offset_y: 0.0,
            mouse_world_pos: f32::Vec2::ZERO,
        };
        camera_manager.apply_view_layout();
        set_camera(&camera_manager.camera);
        camera_manager
    }

    pub fn input(&mut self) {
        self.mouse_world_pos = self
            .camera
            .screen_to_world(f32::Vec2::from(mouse_position()));
    }

    pub fn update(&mut self, dt: f32, loading_manager: &LoadingManager) {
        let game_is_initialized = loading_manager.is_initialized;
        self.apply_view_layout();

        if !self.is_initialized && game_is_initialized {
            self.camera.target = self.target_pos;
            self.is_initialized = true;
            return;
        }

        let diff = self.target_pos - self.camera.target;
        let distance = diff.length();

        if distance > 0.1 {
            let t = (CAMERA_TRANSITION_SPEED * dt).min(1.0);
            self.camera.target = self.camera.target.lerp(self.target_pos, t);
        } else {
            self.camera.target = self.target_pos;
        }
    }

    pub fn world_view_rect(&self, margin: f32) -> Rect {
        let target = self.camera.target;
        Rect::new(
            target.x - VIEW_W / 2.0 - margin,
            target.y - VIEW_H / 2.0 - margin,
            VIEW_W + margin * 2.0,
            VIEW_H + margin * 2.0,
        )
    }

    fn apply_view_layout(&mut self) {
        let (zoom, viewport) = view_layout();

        self.camera.zoom = f32::vec2(2.0 / VIEW_W, 2.0 / VIEW_H);
        self.camera.viewport = Some(viewport);
        self.ui_zoom = zoom;
        self.ui_offset_x = viewport.0 as f32;
        self.ui_offset_y = viewport.1 as f32;
    }
}

pub fn view_layout() -> (f32, (i32, i32, i32, i32)) {
    let window_w = screen_width();
    let window_h = screen_height();
    let zoom = (window_w / VIEW_W)
        .min(window_h / VIEW_H)
        .max(VIEW_ZOOM_MIN);
    let viewport_w = (VIEW_W * zoom).round() as i32;
    let viewport_h = (VIEW_H * zoom).round() as i32;
    let viewport_x = (window_w as i32 - viewport_w) / 2;
    let viewport_y = (window_h as i32 - viewport_h) / 2;
    (zoom, (viewport_x, viewport_y, viewport_w, viewport_h))
}
