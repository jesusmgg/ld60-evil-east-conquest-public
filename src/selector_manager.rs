use std::f32::consts::PI;

use crate::camera_manager::CameraManager;
use crate::constants::*;
use crate::level::level_editor_manager::LevelEditorManager;
use crate::level::level_manager::LevelManager;
use crate::minimap_manager::MinimapManager;
use crate::player_manager::PlayerManager;
use crate::styles::Styles;
use crate::unit_manager::UnitManager;
use macroquad::prelude::*;
use macroquad::ui::root_ui;

const INTERACTABLE_HIGHLIGHT_ENABLED: bool = false;
const INTERACTABLE_HIGHLIGHT_PULSE_SPEED: f32 = 7.0;
const INTERACTABLE_HIGHLIGHT_PULSE_OFFSET: f32 = -PI / 2.0;
const SELECTION_CLICK_RADIUS: f32 = TILE_SIZE_X;

pub struct SelectorManager {
    pub position: Vec2,
    pub selected_unit_i: Option<usize>,
    pub hovered_unit_i: Option<usize>,
    #[cfg(debug_assertions)]
    prev_editor_enabled: bool,
    highlighted_interactable: Option<(IVec2, IVec2)>,
    pub input_selection: Option<Option<usize>>,
    pulse_timer: f32,
}

impl SelectorManager {
    pub fn new() -> Self {
        Self {
            position: Vec2::ZERO,
            selected_unit_i: None,
            hovered_unit_i: None,
            #[cfg(debug_assertions)]
            prev_editor_enabled: false,
            highlighted_interactable: None,
            input_selection: None,
            pulse_timer: 0.0,
        }
    }

    pub fn input(
        &mut self,
        camera_manager: &CameraManager,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        level_editor: &LevelEditorManager,
        minimap: &MinimapManager,
        player_manager: &PlayerManager,
    ) {
        self.hovered_unit_i = None;
        if level_editor.is_editing_properties() {
            return;
        }

        let unit_positions = unit_manager.get_all_positions(level_manager);
        let is_world_under_mouse = is_world_under_mouse(
            camera_manager,
            level_manager,
            level_editor,
            minimap,
            player_manager,
        );
        if is_world_under_mouse {
            self.hovered_unit_i =
                find_hovered_unit(camera_manager.mouse_world_pos, &unit_positions);
        }

        const SELECT_KEYS: [KeyCode; 5] = [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
        ];
        for n in 0..SELECT_KEYS.len() {
            if !player_manager.is_keyboard_captured() && is_key_pressed(SELECT_KEYS[n]) {
                if let Some(&(unit_i, _)) = unit_positions.get(n) {
                    self.input_selection = Some(Some(unit_i));
                }
                break;
            }
        }

        if is_mouse_button_pressed(MouseButton::Left) && is_world_under_mouse {
            self.input_selection =
                Some(self.find_clicked_unit(camera_manager.mouse_world_pos, &unit_positions));
        }
    }

    pub fn select_first_player(&mut self, unit_manager: &UnitManager) {
        self.selected_unit_i = unit_manager.find_first_player_unit();
    }

    pub fn update(
        &mut self,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        _level_editor: &LevelEditorManager,
        dt: f32,
    ) {
        if let Some(new_selection) = self.input_selection.take() {
            let prev_selection = self.selected_unit_i;
            self.selected_unit_i = new_selection;

            if self.selected_unit_i != prev_selection {
                let new_highlight = if let Some(unit_i) = self.selected_unit_i {
                    unit_manager.get_facing_interactable(unit_i, level_manager)
                } else {
                    None
                };
                self.set_highlighted_interactable(new_highlight);
            }
        }

        if let Some(unit_i) = self.selected_unit_i {
            if unit_i >= unit_manager.len() || unit_manager.is_dead[unit_i] {
                self.selected_unit_i = None;
            }
        }

        if let Some(unit_i) = self.selected_unit_i {
            if let Some(target_pos) = unit_manager.get_world_position(unit_i, level_manager) {
                self.position = target_pos;
            }
        }

        self.pulse_timer += dt * INTERACTABLE_HIGHLIGHT_PULSE_SPEED;

        #[cfg(debug_assertions)]
        {
            let editor_enabled = _level_editor.is_enabled;
            if editor_enabled != self.prev_editor_enabled {
                if editor_enabled {
                    self.selected_unit_i = None;
                } else {
                    self.select_first_player(unit_manager);
                }
                self.prev_editor_enabled = editor_enabled;
            }
        }
    }

    pub fn render(&self, styles: &Styles) {
        if self.selected_unit_i.is_none() {
            return;
        }

        let padding = 2.0;
        let x = self.position.x - padding;
        let y = self.position.y - padding;
        let size = TILE_SIZE_X + padding * 2.0;

        let thickness = 2.0;
        let color = styles.colors.parasite_spread;

        draw_rectangle(x, y, size, thickness, color);
        draw_rectangle(x, y + size - thickness, size, thickness, color);
        draw_rectangle(x, y, thickness, size, color);
        draw_rectangle(x + size - thickness, y, thickness, size, color);
    }

    pub fn render_interactable_highlight(&self, level_manager: &LevelManager, styles: &Styles) {
        if !INTERACTABLE_HIGHLIGHT_ENABLED {
            return;
        }

        if !level_manager.is_initialized {
            return;
        }

        let Some((level_pos, tile_pos)) = self.highlighted_interactable else {
            return;
        };

        let world_pos = level_manager.tile_to_world(level_pos, tile_pos);

        let pulse = 0.5 + 0.5 * self.pulse_timer.sin();

        let padding = 2.0;
        let x = world_pos.x - padding;
        let y = world_pos.y - padding;
        let size = TILE_SIZE_X + padding * 2.0;
        let thickness = 2.0;

        let mut color = styles.colors.cyan_bright;
        color.a = pulse;

        draw_rectangle(x, y, size, thickness, color);
        draw_rectangle(x, y + size - thickness, size, thickness, color);
        draw_rectangle(x, y, thickness, size, color);
        draw_rectangle(x + size - thickness, y, thickness, size, color);
    }

    pub fn set_highlighted_interactable(&mut self, interactable: Option<(IVec2, IVec2)>) {
        if self.highlighted_interactable == interactable {
            return;
        }
        self.highlighted_interactable = interactable;
        self.reset_pulse_timer();
    }

    pub fn reset_pulse_timer(&mut self) {
        self.pulse_timer = INTERACTABLE_HIGHLIGHT_PULSE_OFFSET;
    }

    fn find_clicked_unit(
        &self,
        mouse_world_pos: Vec2,
        unit_positions: &[(usize, Vec2)],
    ) -> Option<usize> {
        let mut closest_unit: Option<usize> = None;
        let mut closest_distance = SELECTION_CLICK_RADIUS;

        for &(unit_i, unit_pos) in unit_positions {
            let distance = mouse_world_pos
                .distance(unit_pos + Vec2::new(TILE_SIZE_X / 2.0, TILE_SIZE_Y / 2.0));
            if distance < closest_distance {
                closest_distance = distance;
                closest_unit = Some(unit_i);
            }
        }

        closest_unit
    }
}

fn find_hovered_unit(mouse_world_pos: Vec2, unit_positions: &[(usize, Vec2)]) -> Option<usize> {
    let mut hovered_unit = None;
    for &(unit_i, unit_pos) in unit_positions {
        let tile_rect = Rect::new(unit_pos.x, unit_pos.y, TILE_SIZE_X, TILE_SIZE_Y);
        if tile_rect.contains(mouse_world_pos) {
            hovered_unit = Some(unit_i);
        }
    }
    hovered_unit
}

pub fn is_mouse_over_overlay(
    camera_manager: &CameraManager,
    level_editor: &LevelEditorManager,
    minimap: &MinimapManager,
    player_manager: &PlayerManager,
) -> bool {
    root_ui().is_mouse_over(Vec2::from(mouse_position()))
        || player_manager.is_mouse_captured
        || minimap.contains_screen_point()
        || level_editor.palette_contains_screen_point(camera_manager)
}

fn is_world_under_mouse(
    camera_manager: &CameraManager,
    level_manager: &LevelManager,
    level_editor: &LevelEditorManager,
    minimap: &MinimapManager,
    player_manager: &PlayerManager,
) -> bool {
    if is_mouse_over_overlay(camera_manager, level_editor, minimap, player_manager) {
        return false;
    }
    if level_editor.is_enabled {
        return true;
    }
    let (Some(view_sector), Some(tile)) = (
        level_manager.view_sector_pos,
        level_manager.tile_under_mouse(camera_manager),
    ) else {
        return true;
    };
    level_manager.gate_arrival(view_sector, tile).is_none()
}
