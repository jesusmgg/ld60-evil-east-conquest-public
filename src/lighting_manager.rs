use crate::{
    assets::AssetManager,
    camera_manager::CameraManager,
    constants::{TILE_SIZE_X, TILE_SIZE_Y, VIEW_H, VIEW_W},
    debug_overlay_manager::DebugOverlayManager,
    flame::DamageFlame,
    level::{
        interactable::{Interactable, LightIntensity},
        level_manager::{sector_in_range, sector_range_for_world_rect, LevelManager},
    },
    miniquad::{BlendFactor, BlendState, BlendValue, Equation},
    unit_manager::{snap_to_2px, UnitManager, UnitType},
};
use macroquad::prelude::*;

const LIGHT_ANIMATION_FPS: f32 = 6.0;
const LIGHT_FRAME_COUNT: usize = 8;
const LIGHT_FRAME_SIZE_STRONG: f32 = 256.0;
const LIGHT_FRAME_SIZE_MEDIUM: f32 = 256.0;
const LIGHT_FRAME_SIZE_DIM: f32 = 256.0;
const LIGHT_AMBIENT_WIDTH: f32 = 640.0;
const LIGHT_AMBIENT_HEIGHT: f32 = 368.0;
pub const LIGHT_AMBIENT_LEVEL_DEFAULT: f32 = 0.05;

const LIGHT_CENTER_OFFSET_X: f32 = TILE_SIZE_X / 2.0;
const LIGHT_CENTER_OFFSET_Y: f32 = TILE_SIZE_Y / 2.0;

pub struct LightingManager {
    animation_timer: f32,
    current_frame: usize,
    lightmap_target: RenderTarget,
    multiply_material: Material,
}

impl LightingManager {
    pub fn new() -> Self {
        let lightmap_target = render_target(VIEW_W as u32, VIEW_H as u32);
        lightmap_target.texture.set_filter(FilterMode::Nearest);

        let multiply_material = load_material(
            ShaderSource::Glsl {
                vertex: SIMPLE_VERTEX_SHADER,
                fragment: SIMPLE_FRAGMENT_SHADER,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::DestinationColor),
                        BlendFactor::Zero,
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();

        Self {
            animation_timer: 0.0,
            current_frame: 0,
            lightmap_target,
            multiply_material,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.animation_timer += dt;
        let frame_duration = 1.0 / LIGHT_ANIMATION_FPS;
        if self.animation_timer >= frame_duration {
            self.animation_timer -= frame_duration;
            self.current_frame = (self.current_frame + 1) % LIGHT_FRAME_COUNT;
        }
    }

    pub fn render(
        &self,
        level_manager: &LevelManager,
        unit_manager: &UnitManager,
        assets: &AssetManager,
        camera_manager: &CameraManager,
        debug_overlay_manager: &DebugOverlayManager,
    ) {
        if !debug_overlay_manager.is_lighting_enabled {
            return;
        }

        let camera = &camera_manager.camera;

        let lightmap_camera = Camera2D {
            render_target: Some(self.lightmap_target.clone()),
            zoom: camera.zoom,
            target: camera.target,
            offset: camera.offset,
            rotation: camera.rotation,
            viewport: None,
        };
        set_camera(&lightmap_camera);

        clear_background(BLACK);

        if level_manager.view_sector_pos.is_none() {
            set_camera(camera);
            return;
        }

        let light_reach = LIGHT_FRAME_SIZE_STRONG / 2.0;
        let light_rect = camera_manager.world_view_rect(light_reach);
        let view_rect = camera_manager.world_view_rect(0.0);

        let (sector_min, sector_max) = sector_range_for_world_rect(light_rect);

        for gy in sector_min.y..=sector_max.y {
            for gx in sector_min.x..=sector_max.x {
                let level_pos = IVec2::new(gx, gy);

                let Some(level) = level_manager.get_level_at_pos(level_pos) else {
                    continue;
                };

                let level_size_px = level.grid_size_px();
                let level_origin = level.grid_origin(level_pos);

                let level_bounds = Rect::new(
                    level_origin.x,
                    level_origin.y,
                    level_size_px.x,
                    level_size_px.y,
                );
                if level_bounds.overlaps(&view_rect) {
                    let center_world_pos = level_origin + level_size_px / 2.0;

                    self.draw_ambient_light_cropped(
                        &assets.spritesheet_light_ambient,
                        center_world_pos,
                        level_origin,
                        level_size_px,
                        level.ambient_light_level,
                    );
                }

                for (grid_pos, interactable) in &level.interactables {
                    let Interactable::LightSource(light) = interactable else {
                        continue;
                    };

                    let mut world_pos = level.tile_to_world_pos(level_pos, *grid_pos);
                    world_pos.x += LIGHT_CENTER_OFFSET_X;
                    world_pos.y += LIGHT_CENTER_OFFSET_Y;
                    if !light_rect.contains(world_pos) {
                        continue;
                    }
                    let (spritesheet, frame_size) =
                        self.get_light_for_intensity(light.intensity, assets);
                    self.draw_light_circle(spritesheet, world_pos, frame_size);
                }
            }
        }

        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] {
                continue;
            }
            let mut intensity = LightIntensity::Dim;
            if unit_manager.unit_type[unit_i] != UnitType::Soldier {
                match unit_manager.damage_flame(unit_i) {
                    Some(DamageFlame::Dim) => intensity = LightIntensity::Dim,
                    Some(DamageFlame::Bright) => intensity = LightIntensity::Strong,
                    None => continue,
                }
            }

            let current_level_pos = unit_manager.level_pos[unit_i];
            let previous_level_pos = unit_manager.prev_level_pos[unit_i];
            let current_in = sector_in_range(current_level_pos, sector_min, sector_max);
            let previous_in = sector_in_range(previous_level_pos, sector_min, sector_max);
            if !current_in && !previous_in {
                continue;
            }

            let raw = unit_manager.calculate_world_position(unit_i, level_manager)
                + Vec2::new(LIGHT_CENTER_OFFSET_X, LIGHT_CENTER_OFFSET_Y);
            let unit_center = snap_to_2px(raw);
            if !light_rect.contains(unit_center) {
                continue;
            }
            let (spritesheet, frame_size) = self.get_light_for_intensity(intensity, assets);
            self.draw_light_circle(spritesheet, unit_center, frame_size);
        }

        set_default_camera();

        if let Some((vp_x, vp_y, vp_w, vp_h)) = camera.viewport {
            gl_use_material(&self.multiply_material);

            draw_texture_ex(
                &self.lightmap_target.texture,
                vp_x as f32,
                vp_y as f32,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(vp_w as f32, vp_h as f32)),
                    flip_y: false,
                    ..Default::default()
                },
            );

            gl_use_default_material();
        }

        set_camera(camera);
    }

    fn get_light_for_intensity<'a>(
        &self,
        intensity: LightIntensity,
        assets: &'a AssetManager,
    ) -> (&'a Texture2D, f32) {
        match intensity {
            LightIntensity::Strong => (&assets.spritesheet_light_strong, LIGHT_FRAME_SIZE_STRONG),
            LightIntensity::Medium => (&assets.spritesheet_light_medium, LIGHT_FRAME_SIZE_MEDIUM),
            LightIntensity::Dim => (&assets.spritesheet_light_dim, LIGHT_FRAME_SIZE_DIM),
        }
    }

    fn draw_light_circle(&self, spritesheet: &Texture2D, position: Vec2, frame_size: f32) {
        let source_rect = Rect::new(
            self.current_frame as f32 * frame_size,
            0.0,
            frame_size,
            frame_size,
        );

        let draw_x = position.x - frame_size / 2.0;
        let draw_y = position.y - frame_size / 2.0;

        draw_texture_ex(
            spritesheet,
            draw_x,
            draw_y,
            WHITE,
            DrawTextureParams {
                source: Some(source_rect),
                ..Default::default()
            },
        );
    }

    fn draw_ambient_light_cropped(
        &self,
        spritesheet: &Texture2D,
        center_pos: Vec2,
        level_origin: Vec2,
        level_size: Vec2,
        ambient_level: f32,
    ) {
        let frame_width = LIGHT_AMBIENT_WIDTH;
        let frame_height = LIGHT_AMBIENT_HEIGHT;
        let source_rect = Rect::new(
            self.current_frame as f32 * frame_width,
            0.0,
            frame_width,
            frame_height,
        );

        let light_x = center_pos.x - frame_width / 2.0;
        let light_y = center_pos.y - frame_height / 2.0;

        let clip_x = light_x.max(level_origin.x);
        let clip_y = light_y.max(level_origin.y);
        let clip_right = (light_x + frame_width).min(level_origin.x + level_size.x);
        let clip_bottom = (light_y + frame_height).min(level_origin.y + level_size.y);

        let clip_w = (clip_right - clip_x).max(0.0);
        let clip_h = (clip_bottom - clip_y).max(0.0);

        if clip_w <= 0.0 || clip_h <= 0.0 {
            return;
        }

        let src_offset_x = (clip_x - light_x).max(0.0);
        let src_offset_y = (clip_y - light_y).max(0.0);

        let cropped_source = Rect::new(
            source_rect.x + src_offset_x,
            source_rect.y + src_offset_y,
            clip_w,
            clip_h,
        );

        let ambient_color = Color::new(ambient_level, ambient_level, ambient_level, 1.0);

        draw_texture_ex(
            spritesheet,
            clip_x,
            clip_y,
            ambient_color,
            DrawTextureParams {
                source: Some(cropped_source),
                ..Default::default()
            },
        );
    }
}

const SIMPLE_VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;

varying vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}
"#;

const SIMPLE_FRAGMENT_SHADER: &str = r#"#version 100
precision lowp float;

varying vec2 uv;

uniform sampler2D Texture;

void main() {
    gl_FragColor = texture2D(Texture, uv);
}
"#;
