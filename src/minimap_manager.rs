use macroquad::input::{
    is_key_pressed, is_mouse_button_pressed, mouse_position, KeyCode, MouseButton,
};
use macroquad::math::{IVec2, Vec2};
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::ui::root_ui;

use crate::camera_manager::CameraManager;
use crate::constants::VIEW_W;
use crate::gpu_path_manager::{FIELD_SECTORS_X, FIELD_SECTORS_Y};
use crate::level::level_editor_manager::LevelEditorManager;
use crate::level::level_manager::LevelManager;
use crate::player_manager::{PlayerManager, PANEL_Y};
use crate::styles::Styles;
use crate::ui_draw::virtual_mouse_position;
use crate::unit_manager::{UnitManager, UnitOwner};

const CELL_W: f32 = 10.0;
const CELL_H: f32 = 6.0;
const CELL_GAP: f32 = 1.0;
const MARGIN: f32 = 4.0;
const BORDER: f32 = 1.0;
const CELL_ALPHA: f32 = 0.5;
pub const MINIMAP_BOTTOM: f32 = PANEL_Y + FIELD_SECTORS_Y as f32 * (CELL_H + CELL_GAP);
const MAIN_PATH_ROW: i32 = 4;

pub struct MinimapManager {
    input_clicked_sector: Option<IVec2>,
    input_nav: Option<IVec2>,
    hovered_sector: Option<IVec2>,
}

impl MinimapManager {
    pub fn new() -> Self {
        Self {
            input_clicked_sector: None,
            input_nav: None,
            hovered_sector: None,
        }
    }

    fn layout(level_manager: &LevelManager) -> Option<Vec2> {
        if level_manager.levels.is_empty() {
            return None;
        }
        let width = FIELD_SECTORS_X as f32 * (CELL_W + CELL_GAP) - CELL_GAP;
        Some(Vec2::new(VIEW_W - MARGIN - width, PANEL_Y))
    }

    fn cell_pos(origin: Vec2, sector: IVec2) -> Vec2 {
        Vec2::new(
            origin.x + sector.x as f32 * (CELL_W + CELL_GAP),
            origin.y + sector.y as f32 * (CELL_H + CELL_GAP),
        )
    }

    fn is_sector_shown(
        sector: IVec2,
        player_manager: &PlayerManager,
        level_editor: &LevelEditorManager,
    ) -> bool {
        level_editor.is_enabled
            || sector.y == MAIN_PATH_ROW
            || player_manager.data.visited_levels.contains(&sector)
    }

    pub fn contains_screen_point(&self) -> bool {
        self.hovered_sector.is_some()
    }

    fn sector_at_screen_point(
        level_manager: &LevelManager,
        camera_manager: &CameraManager,
        player_manager: &PlayerManager,
        level_editor: &LevelEditorManager,
    ) -> Option<IVec2> {
        let origin = Self::layout(level_manager)?;
        if camera_manager.ui_zoom <= 0.0 {
            return None;
        }

        let mouse = virtual_mouse_position(camera_manager);
        let virtual_x = mouse.x;
        let virtual_y = mouse.y;

        for sector in level_manager.levels.keys() {
            if !Self::is_sector_shown(*sector, player_manager, level_editor) {
                continue;
            }
            let cell = Self::cell_pos(origin, *sector);
            if virtual_x >= cell.x
                && virtual_x < cell.x + CELL_W
                && virtual_y >= cell.y
                && virtual_y < cell.y + CELL_H
            {
                return Some(*sector);
            }
        }
        None
    }

    pub fn input(
        &mut self,
        level_manager: &LevelManager,
        level_editor: &LevelEditorManager,
        camera_manager: &CameraManager,
        player_manager: &PlayerManager,
    ) {
        self.hovered_sector = None;
        if !cfg!(debug_assertions) {
            return;
        }
        self.hovered_sector = Self::sector_at_screen_point(
            level_manager,
            camera_manager,
            player_manager,
            level_editor,
        );

        let over_palette = level_editor.palette_contains_screen_point(camera_manager);
        let mouse_over_panel = root_ui().is_mouse_over(Vec2::from(mouse_position()));

        if is_mouse_button_pressed(MouseButton::Left)
            && !mouse_over_panel
            && !over_palette
            && !player_manager.is_mouse_captured
        {
            self.input_clicked_sector = self.hovered_sector;
        }

        if level_editor.is_enabled || player_manager.is_keyboard_captured() {
            return;
        }
        if is_key_pressed(KeyCode::Up) {
            self.input_nav = Some(IVec2::new(0, -1));
        } else if is_key_pressed(KeyCode::Down) {
            self.input_nav = Some(IVec2::new(0, 1));
        } else if is_key_pressed(KeyCode::Left) {
            self.input_nav = Some(IVec2::new(-1, 0));
        } else if is_key_pressed(KeyCode::Right) {
            self.input_nav = Some(IVec2::new(1, 0));
        }
    }

    pub fn update(&mut self, level_manager: &mut LevelManager) {
        if let Some(sector) = self.input_clicked_sector.take() {
            level_manager.view_sector_pos = Some(sector);
        }

        if let Some(step) = self.input_nav.take() {
            if let Some(current) = level_manager.view_sector_pos {
                let target = current + step;
                if level_manager.levels.contains_key(&target) {
                    level_manager.view_sector_pos = Some(target);
                }
            }
        }
    }

    pub fn ui(
        &self,
        level_manager: &LevelManager,
        unit_manager: &UnitManager,
        player_manager: &PlayerManager,
        level_editor: &LevelEditorManager,
        styles: &Styles,
        camera_manager: &CameraManager,
    ) {
        let Some(origin) = Self::layout(level_manager) else {
            return;
        };

        let ui_zoom = camera_manager.ui_zoom;
        let ui_offset_x = camera_manager.ui_offset_x;
        let ui_offset_y = camera_manager.ui_offset_y;

        for sector in level_manager.levels.keys() {
            if !Self::is_sector_shown(*sector, player_manager, level_editor) {
                continue;
            }
            let cell = Self::cell_pos(origin, *sector);
            let x = ui_offset_x + cell.x * ui_zoom;
            let y = ui_offset_y + cell.y * ui_zoom;
            let w = CELL_W * ui_zoom;
            let h = CELL_H * ui_zoom;

            let mut fill = styles.colors.blue_shadow;
            fill.a = CELL_ALPHA;
            draw_rectangle(x, y, w, h, fill);

            if level_manager.view_sector_pos == Some(*sector) {
                draw_rectangle_lines(
                    x,
                    y,
                    w,
                    h,
                    BORDER * 2.0 * ui_zoom,
                    styles.colors.cyan_bright,
                );
            } else {
                let mut edge = styles.colors.gray_metal;
                edge.a = 0.5;
                draw_rectangle_lines(x, y, w, h, BORDER * ui_zoom, edge);
            }
        }

        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] {
                continue;
            }
            let sector = unit_manager.level_pos[unit_i];
            if !Self::is_sector_shown(sector, player_manager, level_editor) {
                continue;
            }
            let Some(level) = level_manager.levels.get(&sector) else {
                continue;
            };
            if level.grid_tiles.x <= 0 || level.grid_tiles.y <= 0 {
                continue;
            }

            let tile = unit_manager.tile_position[unit_i];
            let frac_x = tile.x as f32 / level.grid_tiles.x as f32;
            let frac_y = tile.y as f32 / level.grid_tiles.y as f32;

            let cell = Self::cell_pos(origin, sector);
            let dot_x = ui_offset_x + (cell.x + frac_x * CELL_W) * ui_zoom;
            let dot_y = ui_offset_y + (cell.y + frac_y * CELL_H) * ui_zoom;

            let color = match unit_manager.owner[unit_i] {
                UnitOwner::P0 => styles.colors.cyan_bright,
                UnitOwner::P1 => styles.colors.red_alarm,
            };
            draw_rectangle(dot_x, dot_y, ui_zoom.max(1.0), ui_zoom.max(1.0), color);
        }
    }
}
