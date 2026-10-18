use macroquad::{
    input::{
        is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, mouse_wheel, KeyCode,
        MouseButton,
    },
    prelude::*,
    ui::{hash, root_ui, widgets},
};

use crate::{
    assets::AssetManager,
    camera_manager::CameraManager,
    constants::{TILE_HIGHLIGHT_LERP_SPEED, TILE_SIZE_PX, TILE_SIZE_X, TILE_SIZE_Y, VIEW_H},
    debug::DebugLog,
    enums::Direction,
    flame::draw_flame_on_tile,
    gpu_path_manager::{FIELD_SECTORS_X, FIELD_SECTORS_Y},
    history_manager::HistoryManager,
    level::{
        background::Background,
        interactable::{Interactable, LightIntensity, ALL_INTERACTABLES},
        level_manager::{Level, LevelManager},
        music_track::MusicTrack,
        tile::{get_texture_for_tile, render_tile, tile_center, TileType, ALL_TILES},
    },
    minimap_manager::MinimapManager,
    sprite_sheet::sheet_frame_rect,
    styles::Styles,
    text::draw_scaled_text,
    ui_draw::virtual_mouse_position,
    unit_design::{
        default_profile, structure_profile, unit_name, Armor, Chassis, Shielding, StructureSprite,
        Technology, UnitKind, Weapon, ALL_STRUCTURE_SPRITES, ALL_TECHNOLOGIES, MEN_PER_UNIT,
    },
    unit_manager::{
        crewed_machine_texture, default_unit_spawn, draw_machine, draw_soldier_group,
        draw_structure, man_texture, structure_texture, UnitOwner, UnitSpawnData, UnitType,
        ALL_UNIT_OWNERS, ALL_UNIT_TYPES,
    },
};

use std::collections::HashMap;

const HIGHLIGHT_WIDTH_INTERACTABLE: f32 = 1.0;
const HIGHLIGHT_WIDTH_UNIT: f32 = 2.0;
const UNIT_PREVIEW_ALPHA: f32 = 0.4;
const UNIT_MARKER_RADIUS_PREVIEW: f32 = 4.0;
const UNIT_MARKER_RADIUS: f32 = 3.0;
const UNIT_DIRECTION_OFFSET: f32 = 5.0;
const UNIT_DIRECTION_TRIANGLE_SIZE: f32 = 2.0;
const UNIT_LABEL_FONT: f32 = 8.0;
const UNIT_PREVIEW_LABEL_FONT: f32 = 12.0;
const WAYPOINT_MARKER_SIZE: f32 = 4.0;
const GATE_AREA_TILE_PROPERTY: &str = "transfer_area_tile";
const GATE_AREA_SIZE_PROPERTY: &str = "transfer_area_size";

const PALETTE_X: f32 = 4.0;
const PALETTE_W: f32 = 108.0;
const PALETTE_PAGE: usize = 15;
const PALETTE_BTN_W: f32 = 9.0;
const PALETTE_BTN_H: f32 = 9.0;
const PALETTE_BTN_GAP: f32 = 2.0;
const PALETTE_SPRITE: f32 = 16.0;
const LIGHT_SOURCE_ICON_FRAME: usize = 0;
const PALETTE_ROW_H: f32 = 18.0;
const PALETTE_HEADER_H: f32 = 12.0;
const PALETTE_PAD: f32 = 3.0;
const PALETTE_BORDER: f32 = 1.0;
const PALETTE_FONT: f32 = 10.0;
const PALETTE_OWNER_X: f32 = PALETTE_W - 16.0;

const EDITOR_CONTROLS: &[(&str, &str, &str)] = &[
    ("", "F1", "Toggle editor"),
    ("", "H", "Toggle this window"),
    ("", "P", "Toggle palette"),
    ("Navigate", "W/A/S/D", "Move between sectors"),
    ("Navigate", "+ / -", "Create / remove sector here"),
    ("Navigate", "Backspace", "Set sector as initial"),
    ("Modes", "T", "Tile placement"),
    ("Modes", "I", "Interactable placement"),
    ("Modes", "U", "Unit placement"),
    ("Modes", "R", "Transfer area: click a gate, drag"),
    ("Modes", "E", "Edit hovered (I/U modes), or sector"),
    ("Modes", "K", "Edit hovered unit's waypoints (U mode)"),
    ("Modes", "Escape", "Close property editor"),
    ("Place", "Left click", "Place selected"),
    ("Place", "Right click", "Delete at cursor"),
    ("Place", "Mouse wheel", "Cycle type"),
    ("Waypoints", "Left click", "Add a waypoint"),
    ("Waypoints", "Right click", "Remove the last waypoint"),
    ("Waypoints", "K", "Stop editing waypoints"),
    ("Save", "F5 / F6", "Save sector / all sectors"),
    ("Save", "F7 / F8", "Load sector / all sectors"),
    ("Save", "F9", "Save sector set only"),
];

fn unit_preset(preset_i: usize) -> Option<(UnitOwner, UnitType, StructureSprite)> {
    let mut i = 0;
    for owner in ALL_UNIT_OWNERS {
        for unit_type in ALL_UNIT_TYPES {
            if unit_type.is_structure() {
                continue;
            }
            if i == preset_i {
                return Some((*owner, *unit_type, StructureSprite::WallWood));
            }
            i += 1;
        }
    }
    let structure_sprite = ALL_STRUCTURE_SPRITES.get(preset_i - i)?;
    Some((UnitOwner::P1, UnitType::Structure, *structure_sprite))
}

fn unit_preset_count() -> usize {
    let mut count = ALL_STRUCTURE_SPRITES.len();
    for unit_type in ALL_UNIT_TYPES {
        if !unit_type.is_structure() {
            count += ALL_UNIT_OWNERS.len();
        }
    }
    count
}

fn unit_preset_spawn(preset_i: usize) -> Option<UnitSpawnData> {
    let (owner, unit_type, structure_sprite) = unit_preset(preset_i)?;
    Some(default_unit_spawn(owner, unit_type, structure_sprite))
}

fn interactable_icon<'a>(
    assets: &'a AssetManager,
    interactable: &Interactable,
) -> (&'a Texture2D, Option<Rect>) {
    match interactable {
        Interactable::LightSource(_) => {
            let flame = light_source_icon(assets);
            (
                flame,
                Some(sheet_frame_rect(flame, LIGHT_SOURCE_ICON_FRAME)),
            )
        }
        Interactable::Rubble(_) => (&assets.texture_interactable_rubble, None),
        Interactable::Gate(_) => (&assets.texture_interactable_gate, None),
    }
}

fn icon_size(texture: &Texture2D, source: Option<Rect>) -> Vec2 {
    if let Some(source) = source {
        return source.size();
    }
    texture.size()
}

fn light_source_icon(assets: &AssetManager) -> &Texture2D {
    &assets.spritesheet_flame_bright
}

fn draw_light_source_marker(world_pos: Vec2, alpha: f32, assets: &AssetManager) {
    draw_flame_on_tile(
        light_source_icon(assets),
        LIGHT_SOURCE_ICON_FRAME,
        world_pos,
        Color::new(1.0, 1.0, 1.0, alpha),
    );
}

struct PaletteEntry<'a> {
    name: &'static str,
    owner: Option<UnitOwner>,
    icon: &'a Texture2D,
    icon_source: Option<Rect>,
}

fn render_unit_spawn_sprite(
    unit_spawn: &UnitSpawnData,
    world_pos: Vec2,
    color: Color,
    assets: &AssetManager,
) {
    let flip_x = unit_spawn.direction == Direction::Left;
    match unit_spawn.unit_type {
        UnitType::Soldier => draw_soldier_group(
            assets,
            &unit_spawn.profile.design,
            unit_spawn.owner,
            world_pos,
            MEN_PER_UNIT,
            color,
            flip_x,
        ),
        UnitType::Machine => draw_machine(
            assets,
            &unit_spawn.profile.design,
            unit_spawn.owner,
            world_pos,
            MEN_PER_UNIT,
            color,
            flip_x,
        ),
        UnitType::Structure => draw_structure(
            assets,
            unit_spawn.profile.structure_sprite,
            world_pos,
            color,
        ),
    }
}

#[derive(Clone)]
pub enum PropertyValue {
    Number,
    Enum {
        current: usize,
        options: Vec<String>,
    },
    Vec2(IVec2),
}

enum PlacementMode {
    Tile,
    Interactable,
    Unit,
    TransferArea,
}

pub struct LevelEditorManager {
    pub is_enabled: bool,

    pub is_placing_tiles: bool,
    pub selected_tile_type: TileType,
    pub hovered_tile_pos: Option<IVec2>,

    pub is_placing_interactables: bool,
    pub selected_interactable_type: usize,
    all_interactables: Vec<Interactable>,
    pub selected_interactable_pos: Option<IVec2>,
    pub edit_properties: HashMap<String, PropertyValue>,
    pub edit_property_strings: HashMap<String, String>,

    pub is_placing_units: bool,
    pub selected_unit_preset: usize,
    pub selected_unit_pos: Option<IVec2>,
    pub waypoint_unit_pos: Option<IVec2>,
    pub waypoint_sector_pos: Option<IVec2>,

    pub is_placing_transfer_area: bool,
    transfer_area_gate: Option<IVec2>,
    transfer_area_drag_start: Option<IVec2>,
    transfer_area_drag_end: IVec2,

    pub editing_level_properties: bool,

    pub show_controls: bool,
    pub show_palette: bool,
    palette_page_shown: usize,
    palette_rows_locked: usize,

    input_toggle: bool,
    input_save: bool,
    input_save_all: bool,
    input_load: bool,
    input_load_all: bool,
    input_save_level_set: bool,
    input_nav_up: bool,
    input_nav_down: bool,
    input_nav_left: bool,
    input_nav_right: bool,
    input_add_level: bool,
    input_remove_level: bool,
    input_set_initial: bool,
    input_tile_mode: bool,
    input_place: bool,
    input_delete: bool,
    input_interactable_mode: bool,
    input_edit_interactable: bool,
    input_unit_mode: bool,
    input_transfer_area_mode: bool,
    input_edit_unit: bool,
    input_waypoint_mode: bool,
    input_waypoint_add: bool,
    input_waypoint_remove: bool,
    input_edit_level_properties: bool,
    input_exit_editing: bool,
    input_toggle_controls: bool,
    input_toggle_palette: bool,
    input_palette_row: Option<usize>,
    input_palette_page_step: Option<i32>,

    stroke_place: bool,
    stroke_delete: bool,

    mouse_wheel_delta: f32,

    highlighted_tile: Option<IVec2>,
    highlight_pos: Vec2,
}

impl LevelEditorManager {
    pub fn new() -> Self {
        Self {
            is_enabled: false,

            is_placing_tiles: false,
            selected_tile_type: ALL_TILES[0],
            hovered_tile_pos: None,

            is_placing_interactables: false,
            selected_interactable_type: 0,
            all_interactables: ALL_INTERACTABLES(),
            selected_interactable_pos: None,
            edit_properties: HashMap::new(),
            edit_property_strings: HashMap::new(),

            is_placing_units: false,
            selected_unit_preset: 0,
            selected_unit_pos: None,
            waypoint_unit_pos: None,
            waypoint_sector_pos: None,

            is_placing_transfer_area: false,
            transfer_area_gate: None,
            transfer_area_drag_start: None,
            transfer_area_drag_end: IVec2::ZERO,

            editing_level_properties: false,

            show_controls: false,
            show_palette: true,
            palette_page_shown: 0,
            palette_rows_locked: 0,

            input_toggle: false,
            input_save: false,
            input_save_all: false,
            input_load: false,
            input_load_all: false,
            input_save_level_set: false,
            input_nav_up: false,
            input_nav_down: false,
            input_nav_left: false,
            input_nav_right: false,
            input_add_level: false,
            input_remove_level: false,
            input_set_initial: false,
            input_tile_mode: false,
            input_place: false,
            input_delete: false,
            input_interactable_mode: false,
            input_edit_interactable: false,
            input_unit_mode: false,
            input_transfer_area_mode: false,
            input_edit_unit: false,
            input_waypoint_mode: false,
            input_waypoint_add: false,
            input_waypoint_remove: false,
            input_edit_level_properties: false,
            input_exit_editing: false,
            input_toggle_controls: false,
            input_toggle_palette: false,
            input_palette_row: None,
            input_palette_page_step: None,

            stroke_place: false,
            stroke_delete: false,

            mouse_wheel_delta: 0.0,

            highlighted_tile: None,
            highlight_pos: Vec2::ZERO,
        }
    }

    pub fn is_editing_interactable_properties(&self) -> bool {
        self.selected_interactable_pos.is_some()
    }

    pub fn is_editing_unit_properties(&self) -> bool {
        self.selected_unit_pos.is_some()
    }

    pub fn is_editing_level_properties(&self) -> bool {
        self.editing_level_properties
    }

    pub fn is_editing_properties(&self) -> bool {
        self.is_editing_interactable_properties()
            || self.is_editing_unit_properties()
            || self.is_editing_level_properties()
    }

    pub fn close_all_property_editors(&mut self) {
        self.selected_interactable_pos = None;
        self.selected_unit_pos = None;
        self.waypoint_unit_pos = None;
        self.waypoint_sector_pos = None;
        self.editing_level_properties = false;
        self.edit_properties.clear();
        self.edit_property_strings.clear();
    }

    fn set_placement_mode(&mut self, mode: PlacementMode, enabled: bool, debug_log: &mut DebugLog) {
        match mode {
            PlacementMode::Tile => self.is_placing_tiles = enabled,
            PlacementMode::Interactable => self.is_placing_interactables = enabled,
            PlacementMode::Unit => self.is_placing_units = enabled,
            PlacementMode::TransferArea => self.is_placing_transfer_area = enabled,
        }
        self.transfer_area_gate = None;
        self.transfer_area_drag_start = None;

        if enabled {
            self.show_palette = true;

            self.palette_rows_locked = 0;

            self.is_placing_tiles = matches!(mode, PlacementMode::Tile);
            self.is_placing_interactables = matches!(mode, PlacementMode::Interactable);
            self.is_placing_units = matches!(mode, PlacementMode::Unit);
            self.is_placing_transfer_area = matches!(mode, PlacementMode::TransferArea);
        }

        self.close_all_property_editors();

        let mode_name = match mode {
            PlacementMode::Tile => "Tile",
            PlacementMode::Interactable => "Interactable",
            PlacementMode::Unit => "Unit",
            PlacementMode::TransferArea => "Transfer area",
        };
        debug_log.log(format!(
            "{} placement mode {}",
            mode_name,
            if enabled { "ON" } else { "OFF" }
        ));
    }

    pub fn input(&mut self, minimap: &MinimapManager, camera_manager: &CameraManager) {
        if !is_mouse_button_down(MouseButton::Left) {
            self.stroke_place = false;
        }
        if !is_mouse_button_down(MouseButton::Right) {
            self.stroke_delete = false;
        }

        #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
        {
            self.input_toggle = is_key_pressed(KeyCode::F1);
        }

        if self.is_enabled {
            self.input_load = is_key_pressed(KeyCode::F7);
            self.input_load_all = is_key_pressed(KeyCode::F8);

            if self.is_editing_properties() {
                self.input_exit_editing = is_key_pressed(KeyCode::Escape);
            }

            if self.is_editing_properties() {
                return;
            }

            self.input_save = is_key_pressed(KeyCode::F5);
            self.input_save_all = is_key_pressed(KeyCode::F6);
            self.input_save_level_set = is_key_pressed(KeyCode::F9);
            self.input_nav_up = is_key_pressed(KeyCode::W);
            self.input_nav_down = is_key_pressed(KeyCode::S);
            self.input_nav_left = is_key_pressed(KeyCode::A);
            self.input_nav_right = is_key_pressed(KeyCode::D);
            self.input_add_level = is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd);
            self.input_remove_level =
                is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract);
            self.input_set_initial = is_key_pressed(KeyCode::Backspace);
            self.input_toggle_controls = is_key_pressed(KeyCode::H);
            self.input_toggle_palette = is_key_pressed(KeyCode::P);
            self.input_tile_mode = is_key_pressed(KeyCode::T);
            self.input_interactable_mode = is_key_pressed(KeyCode::I);
            self.input_unit_mode = is_key_pressed(KeyCode::U);
            self.input_transfer_area_mode = is_key_pressed(KeyCode::R);

            self.input_edit_interactable = self.is_placing_interactables
                && !self.is_placing_units
                && is_key_pressed(KeyCode::E);

            self.input_edit_unit = self.is_placing_units
                && !self.is_placing_interactables
                && is_key_pressed(KeyCode::E);
            self.input_waypoint_mode = self.is_placing_units && is_key_pressed(KeyCode::K);

            self.input_edit_level_properties = !self.is_placing_tiles
                && !self.is_placing_interactables
                && !self.is_placing_units
                && !self.is_placing_transfer_area
                && is_key_pressed(KeyCode::E);

            if self.is_placing_tiles
                || self.is_placing_interactables
                || self.is_placing_units
                || self.is_placing_transfer_area
            {
                let mouse_over_window = root_ui().is_mouse_over(Vec2::from(mouse_position()));

                let mouse_over_panel = mouse_over_window
                    || self.palette_contains_screen_point(camera_manager)
                    || minimap.contains_screen_point();

                if is_mouse_button_pressed(MouseButton::Left) {
                    if !mouse_over_window {
                        self.input_palette_page_step =
                            self.palette_page_step_at_screen_point(camera_manager);
                        self.input_palette_row = self.palette_row_at_screen_point(camera_manager);
                    }
                    self.stroke_place = !mouse_over_panel;
                }
                if is_mouse_button_pressed(MouseButton::Right) {
                    self.stroke_delete = !mouse_over_panel;
                }

                self.input_place = self.stroke_place;
                self.input_delete = self.stroke_delete;

                if self.waypoint_unit_pos.is_some() {
                    self.input_waypoint_add =
                        is_mouse_button_pressed(MouseButton::Left) && !mouse_over_panel;
                    self.input_waypoint_remove =
                        is_mouse_button_pressed(MouseButton::Right) && !mouse_over_panel;
                }

                let mouse_over_wheel_blocker = mouse_over_window || minimap.contains_screen_point();
                self.mouse_wheel_delta = if mouse_over_wheel_blocker {
                    0.0
                } else {
                    mouse_wheel().1
                };
            }
        }
    }

    fn add_level_at_current_pos(
        &self,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        let Some(pos) = level_manager.view_sector_pos else {
            return;
        };
        let is_new = !level_manager.levels.contains_key(&pos);
        level_manager.create_level_at(pos, debug_log);
        if is_new {
            history.register_sector(pos);
        }
    }

    fn remove_level_at_current_pos(
        &self,
        level_manager: &mut LevelManager,
        debug_log: &mut DebugLog,
    ) {
        let Some(pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(pos) else {
            return;
        };
        let file_name = level.file_name.clone();

        level_manager.levels.remove(&pos);

        level_manager.ensure_level_set(debug_log);
        if let Some(ref mut level_set) = level_manager.level_set {
            level_set.level_grid.remove(&pos);
        }

        if !level_manager.levels.is_empty() {
            level_manager.view_sector_pos = level_manager.first_sector_pos();
        }

        debug_log.log(format!(
            "Removed level '{}' at ({}, {})",
            file_name, pos.x, pos.y
        ));
    }

    fn set_current_as_initial(&self, level_manager: &mut LevelManager, debug_log: &mut DebugLog) {
        let Some(pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(pos) else {
            return;
        };
        let level_name = level.name.clone();
        level_manager.ensure_level_set(debug_log);
        if let Some(ref mut level_set) = level_manager.level_set {
            level_set.initial_level_name = level_name.clone();
            debug_log.log(format!("Set '{}' as initial level", level_name));
        }
    }

    fn cycle_index(current: usize, total: usize, direction: i32) -> usize {
        if direction > 0 {
            (current + 1) % total
        } else {
            (current + total - 1) % total
        }
    }

    fn cycle_tile_type(&mut self, direction: i32) {
        let current_index = self.selected_palette_index();
        let new_index = Self::cycle_index(current_index, ALL_TILES.len(), direction);
        self.set_palette_index(new_index);
    }

    fn cycle_interactable_type(&mut self, direction: i32) {
        let new_index = Self::cycle_index(
            self.selected_interactable_type,
            self.all_interactables.len(),
            direction,
        );
        self.set_palette_index(new_index);
    }

    fn get_hovered_tile_pos(
        &self,
        level_manager: &LevelManager,
        mouse_world_pos: Vec2,
    ) -> Option<IVec2> {
        if !self.is_placing_tiles
            && !self.is_placing_interactables
            && !self.is_placing_units
            && !self.is_placing_transfer_area
        {
            return None;
        }
        let active_pos = level_manager.view_sector_pos?;
        let level = level_manager.get_level_at_pos(active_pos)?;
        level.world_to_tile_pos(active_pos, mouse_world_pos)
    }

    fn place_tile_at(&self, level_manager: &mut LevelManager, tile_pos: IVec2) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        level.tile_layout.insert(tile_pos, self.selected_tile_type);
    }

    fn delete_tile_at(&self, level_manager: &mut LevelManager, tile_pos: IVec2) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        level.tile_layout.remove(&tile_pos);
    }

    fn place_interactable_at(
        &self,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        tile_pos: IVec2,
    ) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        let Some(interactable) = self.all_interactables.get(self.selected_interactable_type) else {
            return;
        };
        let old = level.interactables.get(&tile_pos).cloned();
        history.record_event_interactable(active_pos, tile_pos, old.as_ref());
        let mut interactable = interactable.clone();
        if let Interactable::Gate(gate) = &mut interactable {
            gate.transfer_area_size = level.grid_tiles;
        }
        level.interactables.insert(tile_pos, interactable);
    }

    fn delete_interactable_at(
        &self,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        tile_pos: IVec2,
    ) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        let old = level.interactables.get(&tile_pos).cloned();
        history.record_event_interactable(active_pos, tile_pos, old.as_ref());
        level.interactables.remove(&tile_pos);
    }

    fn select_interactable_for_editing(&mut self, level_manager: &LevelManager, tile_pos: IVec2) {
        self.close_all_property_editors();
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };
        if level.interactables.contains_key(&tile_pos) {
            self.selected_interactable_pos = Some(tile_pos);
            self.populate_edit_properties(level_manager);
        }
    }

    fn release_transfer_area_drag(
        &mut self,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        start: IVec2,
        end: IVec2,
        debug_log: &mut DebugLog,
    ) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        if start == end && level.gate_at(start).is_some() {
            self.transfer_area_gate = Some(start);
            debug_log.log(format!("Transfer area gate {},{} picked", start.x, start.y));
            return;
        }
        let Some(gate_tile) = self.transfer_area_gate else {
            debug_log.warning(String::from(
                "[LevelEditorManager] Click a gate before dragging its transfer area",
            ));
            return;
        };
        let Some(old_interactable) = level.interactables.get(&gate_tile).cloned() else {
            self.transfer_area_gate = None;
            return;
        };
        let Interactable::Gate(_) = old_interactable else {
            self.transfer_area_gate = None;
            return;
        };
        history.record_event_interactable(active_pos, gate_tile, Some(&old_interactable));
        let Some(Interactable::Gate(gate)) = level.interactables.get_mut(&gate_tile) else {
            return;
        };
        gate.transfer_area_tile = start.min(end);
        gate.transfer_area_size = (start - end).abs() + IVec2::ONE;
        debug_log.log(format!(
            "Gate {},{} transfer area set to {},{} size {},{}",
            gate_tile.x,
            gate_tile.y,
            gate.transfer_area_tile.x,
            gate.transfer_area_tile.y,
            gate.transfer_area_size.x,
            gate.transfer_area_size.y
        ));
    }

    fn insert_vec2_property(&mut self, key: &str, value: IVec2) {
        self.edit_properties
            .insert(key.to_string(), PropertyValue::Vec2(value));
        self.edit_property_strings
            .insert(format!("{}_x", key), value.x.to_string());
        self.edit_property_strings
            .insert(format!("{}_y", key), value.y.to_string());
    }

    fn get_vec2_property(&self, key: &str) -> Option<IVec2> {
        let x = self
            .edit_property_strings
            .get(&format!("{}_x", key))?
            .parse::<i32>()
            .ok()?;
        let y = self
            .edit_property_strings
            .get(&format!("{}_y", key))?
            .parse::<i32>()
            .ok()?;
        Some(IVec2::new(x, y))
    }

    fn populate_edit_properties(&mut self, level_manager: &LevelManager) {
        self.edit_properties.clear();
        self.edit_property_strings.clear();

        let Some(pos) = self.selected_interactable_pos else {
            return;
        };
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };
        let Some(interactable) = level.interactables.get(&pos) else {
            return;
        };
        match interactable {
            Interactable::LightSource(light) => {
                self.edit_properties.insert(
                    "intensity".to_string(),
                    PropertyValue::Enum {
                        current: light.intensity.to_sequential_index(),
                        options: LightIntensity::variant_names(),
                    },
                );
            }
            Interactable::Rubble(_) => {}
            Interactable::Gate(gate) => {
                self.edit_properties.insert(
                    "direction".to_string(),
                    PropertyValue::Enum {
                        current: gate.direction.to_sequential_index(),
                        options: Direction::variant_names(),
                    },
                );
                self.insert_vec2_property(GATE_AREA_TILE_PROPERTY, gate.transfer_area_tile);
                self.insert_vec2_property(GATE_AREA_SIZE_PROPERTY, gate.transfer_area_size);
            }
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
        camera_manager: &CameraManager,
    ) {
        let mouse_world_pos = camera_manager.mouse_world_pos;
        self.hovered_tile_pos = self.get_hovered_tile_pos(level_manager, mouse_world_pos);
        self.update_tile_highlight(dt, level_manager, mouse_world_pos);
        if self.input_toggle {
            self.is_enabled = !self.is_enabled;
            if self.is_enabled {
                self.show_controls = true;
            } else {
                self.close_all_property_editors();
                self.transfer_area_drag_start = None;
            }
        }

        if self.input_exit_editing {
            self.close_all_property_editors();
        }

        if let Some(step) = self.input_palette_page_step {
            let page = self.palette_page();
            if step < 0 {
                if page > 0 {
                    self.palette_page_shown = page - 1;
                }
            } else if page + 1 < self.palette_page_count() {
                self.palette_page_shown = page + 1;
            }
        }

        if let Some(row) = self.input_palette_row {
            self.set_palette_index(row);
        }

        if self.input_toggle_controls {
            self.show_controls = !self.show_controls;
        }
        if self.input_toggle_palette {
            self.show_palette = !self.show_palette;
        }

        if self.input_tile_mode {
            let new_state = !self.is_placing_tiles;
            self.set_placement_mode(PlacementMode::Tile, new_state, debug_log);
        }

        if self.is_placing_tiles {
            if self.mouse_wheel_delta != 0.0 {
                let direction = if self.mouse_wheel_delta > 0.0 { -1 } else { 1 };
                self.cycle_tile_type(direction);
            }

            if self.input_place {
                if let Some(tile_pos) = self.hovered_tile_pos {
                    self.place_tile_at(level_manager, tile_pos);
                }
            }

            if self.input_delete {
                if let Some(tile_pos) = self.hovered_tile_pos {
                    self.delete_tile_at(level_manager, tile_pos);
                }
            }
        }

        if self.input_interactable_mode {
            let new_state = !self.is_placing_interactables;
            self.set_placement_mode(PlacementMode::Interactable, new_state, debug_log);
        }

        if self.is_placing_interactables && self.selected_interactable_pos.is_none() {
            if self.mouse_wheel_delta != 0.0 {
                let direction = if self.mouse_wheel_delta > 0.0 { -1 } else { 1 };
                self.cycle_interactable_type(direction);
            }

            if self.input_place {
                if let Some(tile_pos) = self.hovered_tile_pos {
                    self.place_interactable_at(level_manager, history, tile_pos);
                }
            }

            if self.input_delete {
                if let Some(tile_pos) = self.hovered_tile_pos {
                    self.delete_interactable_at(level_manager, history, tile_pos);
                }
            }
        }

        if self.input_edit_interactable {
            if let Some(tile_pos) = self.hovered_tile_pos {
                self.select_interactable_for_editing(level_manager, tile_pos);
            }
        }

        if self.input_unit_mode {
            let new_state = !self.is_placing_units;
            self.set_placement_mode(PlacementMode::Unit, new_state, debug_log);
        }

        if self.is_placing_units {
            if self.mouse_wheel_delta != 0.0 {
                let direction = if self.mouse_wheel_delta > 0.0 { -1 } else { 1 };
                self.cycle_unit_preset(direction);
            }

            if self.input_waypoint_mode {
                if self.waypoint_unit_pos.is_some() {
                    self.waypoint_unit_pos = None;
                    self.waypoint_sector_pos = None;
                } else if let Some(tile_pos) = self.hovered_tile_pos {
                    if Self::unit_spawn_waypoints(level_manager, tile_pos).is_some() {
                        self.waypoint_unit_pos = Some(tile_pos);
                        self.waypoint_sector_pos = level_manager.view_sector_pos;
                    }
                }
            }

            if self.waypoint_sector_pos != level_manager.view_sector_pos {
                self.waypoint_unit_pos = None;
                self.waypoint_sector_pos = None;
            }

            if let Some(unit_pos) = self.waypoint_unit_pos {
                let hovered_tile_pos = self.hovered_tile_pos;
                let is_add = self.input_waypoint_add;
                let is_remove = self.input_waypoint_remove;
                match Self::unit_spawn_waypoints(level_manager, unit_pos) {
                    Some(waypoints) => {
                        if is_add {
                            if let Some(tile_pos) = hovered_tile_pos {
                                if waypoints.last() != Some(&tile_pos) {
                                    waypoints.push(tile_pos);
                                }
                            }
                        }
                        if is_remove {
                            waypoints.pop();
                        }
                    }
                    None => self.waypoint_unit_pos = None,
                }
            } else {
                if self.input_place {
                    if let Some(tile_pos) = self.hovered_tile_pos {
                        self.place_unit_at(level_manager, tile_pos);
                    }
                }

                if self.input_delete {
                    if let Some(tile_pos) = self.hovered_tile_pos {
                        self.delete_unit_at(level_manager, tile_pos);
                    }
                }
            }

            if self.input_edit_unit {
                if let Some(tile_pos) = self.hovered_tile_pos {
                    self.start_editing_unit(level_manager, tile_pos, debug_log);
                }
            }
        }

        if self.input_transfer_area_mode {
            let new_state = !self.is_placing_transfer_area;
            self.set_placement_mode(PlacementMode::TransferArea, new_state, debug_log);
        }

        if self.is_enabled && self.is_placing_transfer_area {
            if self.input_place {
                if let Some(tile_pos) = self.hovered_tile_pos {
                    if self.transfer_area_drag_start.is_none() {
                        self.transfer_area_drag_start = Some(tile_pos);
                    }
                    self.transfer_area_drag_end = tile_pos;
                }
            } else if let Some(start) = self.transfer_area_drag_start.take() {
                let end = self.transfer_area_drag_end;
                self.release_transfer_area_drag(level_manager, history, start, end, debug_log);
            }
        }

        if self.input_edit_level_properties {
            self.start_editing_level_properties(level_manager, debug_log);
        }

        if !self.is_editing_properties()
            && (self.input_nav_up
                || self.input_nav_down
                || self.input_nav_left
                || self.input_nav_right)
        {
            if let Some(active_pos) = level_manager.view_sector_pos {
                let mut new_pos = active_pos;

                if self.input_nav_up {
                    new_pos.y = (new_pos.y - 1).max(0);
                }
                if self.input_nav_down {
                    new_pos.y = (new_pos.y + 1).min(FIELD_SECTORS_Y - 1);
                }
                if self.input_nav_left {
                    new_pos.x = (new_pos.x - 1).max(0);
                }
                if self.input_nav_right {
                    new_pos.x = (new_pos.x + 1).min(FIELD_SECTORS_X - 1);
                }

                if new_pos != active_pos {
                    level_manager.view_sector_pos = Some(new_pos);
                    self.close_all_property_editors();
                    self.transfer_area_gate = None;
                    self.transfer_area_drag_start = None;
                }
            }
        }

        if self.input_save {
            if let Some(active_pos) = level_manager.view_sector_pos {
                level_manager.save_single_level(active_pos, debug_log);
            } else {
                debug_log.warning(String::from("[LevelEditorManager] No active level to save"));
            }
        }

        if self.input_save_all {
            if let Some(active_pos) = level_manager.view_sector_pos {
                level_manager.save_single_level(active_pos, debug_log);
            }
            level_manager.save_all_levels(debug_log);
        }

        if self.input_load {
            self.close_all_property_editors();
            if let Some(active_pos) = level_manager.view_sector_pos {
                if let Some(level) = level_manager.get_level_at_pos(active_pos) {
                    let file_name = level.file_name.clone();
                    level_manager.load_single_level_async(active_pos, &file_name, debug_log);
                } else {
                    debug_log.warning(String::from(
                        "[LevelEditorManager] No level at active position",
                    ));
                }
            } else {
                debug_log.warning(String::from("[LevelEditorManager] No active level to load"));
            }
        }

        if self.input_load_all {
            self.close_all_property_editors();
            level_manager.load_all_levels_async(debug_log);
        }

        if self.input_save_level_set {
            level_manager.save_level_set(debug_log);
        }

        if self.input_add_level {
            self.add_level_at_current_pos(level_manager, history, debug_log);
        }

        if self.input_remove_level {
            self.remove_level_at_current_pos(level_manager, debug_log);
        }

        if self.input_set_initial {
            self.set_current_as_initial(level_manager, debug_log);
        }

        self.update_palette_size_lock();

        self.clear_input();
    }

    fn update_palette_size_lock(&mut self) {
        if !self.palette_is_open() {
            self.palette_rows_locked = 0;
            return;
        }

        let rows = self.palette_page_len();
        if rows > self.palette_rows_locked {
            self.palette_rows_locked = rows;
        }
    }

    fn clear_input(&mut self) {
        self.input_toggle = false;
        self.input_save = false;
        self.input_save_all = false;
        self.input_load = false;
        self.input_load_all = false;
        self.input_save_level_set = false;
        self.input_nav_up = false;
        self.input_nav_down = false;
        self.input_nav_left = false;
        self.input_nav_right = false;
        self.input_add_level = false;
        self.input_remove_level = false;
        self.input_set_initial = false;
        self.input_tile_mode = false;
        self.input_place = false;
        self.input_delete = false;
        self.input_toggle_controls = false;
        self.input_toggle_palette = false;
        self.input_palette_row = None;
        self.input_palette_page_step = None;
        self.input_interactable_mode = false;
        self.input_edit_interactable = false;
        self.input_unit_mode = false;
        self.input_transfer_area_mode = false;
        self.input_edit_unit = false;
        self.input_waypoint_mode = false;
        self.input_waypoint_add = false;
        self.input_waypoint_remove = false;
        self.input_edit_level_properties = false;
        self.input_exit_editing = false;

        self.mouse_wheel_delta = 0.0;
    }

    fn render_waypoints(
        &self,
        level: &Level,
        level_pos: IVec2,
        unit_pos: IVec2,
        unit_spawn: &UnitSpawnData,
        styles: &Styles,
    ) {
        if unit_spawn.waypoints.is_empty() {
            return;
        }
        let mut color = self.get_unit_owner_color(&unit_spawn.owner, 0.8);
        if self.waypoint_unit_pos == Some(unit_pos) {
            color = styles.colors.yellow_hot;
        }
        let first = tile_center(level.tile_to_world_pos(level_pos, unit_spawn.waypoints[0]));
        let mut previous = first;
        for waypoint_i in 0..unit_spawn.waypoints.len() {
            let point =
                tile_center(level.tile_to_world_pos(level_pos, unit_spawn.waypoints[waypoint_i]));
            if waypoint_i > 0 {
                draw_line(previous.x, previous.y, point.x, point.y, 1.0, color);
            }
            draw_rectangle(
                point.x - WAYPOINT_MARKER_SIZE / 2.0,
                point.y - WAYPOINT_MARKER_SIZE / 2.0,
                WAYPOINT_MARKER_SIZE,
                WAYPOINT_MARKER_SIZE,
                color,
            );
            previous = point;
        }
        if unit_spawn.waypoints.len() > 1 {
            draw_line(previous.x, previous.y, first.x, first.y, 1.0, color);
        }
    }

    fn render_selection_highlight(
        &self,
        level_manager: &LevelManager,
        tile_pos: IVec2,
        color: Color,
        width: f32,
    ) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };
        let world_pos = level.tile_to_world_pos(active_pos, tile_pos);
        draw_rectangle_lines(
            world_pos.x,
            world_pos.y,
            TILE_SIZE_PX,
            TILE_SIZE_PX,
            width,
            color,
        );
    }

    fn get_unit_owner_color(&self, owner: &UnitOwner, alpha: f32) -> Color {
        match owner {
            UnitOwner::P0 => Color::new(0.0, 0.8, 1.0, alpha),
            UnitOwner::P1 => Color::new(1.0, 0.3, 0.3, alpha),
        }
    }

    fn calculate_direction_triangle(
        center: Vec2,
        direction: Direction,
        offset: f32,
        size: f32,
    ) -> (Vec2, Vec2, Vec2) {
        let (dx, dy) = match direction {
            Direction::Up => (0.0, -offset),
            Direction::Right => (offset, 0.0),
            Direction::Down => (0.0, offset),
            Direction::Left => (-offset, 0.0),
        };

        let tip = Vec2::new(center.x + dx, center.y + dy);

        match direction {
            Direction::Up => (
                tip,
                Vec2::new(tip.x - size, tip.y + size),
                Vec2::new(tip.x + size, tip.y + size),
            ),
            Direction::Right => (
                tip,
                Vec2::new(tip.x - size, tip.y - size),
                Vec2::new(tip.x - size, tip.y + size),
            ),
            Direction::Down => (
                tip,
                Vec2::new(tip.x - size, tip.y - size),
                Vec2::new(tip.x + size, tip.y - size),
            ),
            Direction::Left => (
                tip,
                Vec2::new(tip.x + size, tip.y - size),
                Vec2::new(tip.x + size, tip.y + size),
            ),
        }
    }

    fn update_tile_highlight(
        &mut self,
        dt: f32,
        level_manager: &LevelManager,
        mouse_world_pos: Vec2,
    ) {
        if !self.is_enabled {
            self.highlighted_tile = None;
            return;
        }

        if let Some(level_pos) = level_manager.view_sector_pos {
            if let Some(level) = level_manager.current_level() {
                let grid_origin = level.grid_origin(level_pos);
                let grid_size = level.grid_size_px();

                if mouse_world_pos.x >= grid_origin.x
                    && mouse_world_pos.x < grid_origin.x + grid_size.x
                    && mouse_world_pos.y >= grid_origin.y
                    && mouse_world_pos.y < grid_origin.y + grid_size.y
                {
                    let tile_x = ((mouse_world_pos.x - grid_origin.x) / TILE_SIZE_X) as i32;
                    let tile_y = ((mouse_world_pos.y - grid_origin.y) / TILE_SIZE_Y) as i32;

                    self.highlighted_tile = Some(IVec2::new(tile_x, tile_y));
                } else {
                    self.highlighted_tile = None;
                }
            } else {
                self.highlighted_tile = None;
            }
        } else {
            self.highlighted_tile = None;
        }

        if let Some(tile) = self.highlighted_tile {
            let target = Vec2::new(tile.x as f32, tile.y as f32);
            let t = 1.0 - (1.0 - TILE_HIGHLIGHT_LERP_SPEED * dt).max(0.0);
            self.highlight_pos = self.highlight_pos + (target - self.highlight_pos) * t;
        }
    }

    fn render_tile_highlight(&self, styles: &Styles, level_manager: &LevelManager) {
        if !self.is_enabled || self.highlighted_tile.is_none() {
            return;
        }

        if let Some(level_pos) = level_manager.view_sector_pos {
            if let Some(level) = level_manager.current_level() {
                let mut highlight_color = styles.colors.cyan_bright;
                highlight_color.a = 0.4;

                let grid_origin = level.grid_origin(level_pos);

                let x = grid_origin.x + (self.highlight_pos.x * TILE_SIZE_X);
                let y = grid_origin.y + (self.highlight_pos.y * TILE_SIZE_Y);

                draw_rectangle(x, y, TILE_SIZE_X, TILE_SIZE_Y, highlight_color);
            }
        }
    }

    fn render_light_source_markers(&self, level_manager: &LevelManager, assets: &AssetManager) {
        if !self.is_enabled {
            return;
        }
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };
        for (tile_pos, interactable) in &level.interactables {
            if let Interactable::LightSource(_) = interactable {
                let world_pos = level.tile_to_world_pos(active_pos, *tile_pos);
                draw_light_source_marker(world_pos, 1.0, assets);
            }
        }
    }

    pub fn render(&self, styles: &Styles, level_manager: &LevelManager, assets: &AssetManager) {
        self.render_tile_highlight(styles, level_manager);
        self.render_light_source_markers(level_manager, assets);

        if let Some(tile_pos) = self.hovered_tile_pos {
            let Some(active_pos) = level_manager.view_sector_pos else {
                return;
            };
            let Some(level) = level_manager.get_level_at_pos(active_pos) else {
                return;
            };

            if self.is_placing_tiles {
                let world_pos = level.tile_to_world_pos(active_pos, tile_pos);

                render_tile(
                    assets,
                    self.selected_tile_type,
                    world_pos,
                    Color::new(1.0, 1.0, 1.0, UNIT_PREVIEW_ALPHA),
                );
            }

            if self.is_placing_interactables {
                if let Some(interactable) =
                    self.all_interactables.get(self.selected_interactable_type)
                {
                    if let Interactable::LightSource(_) = interactable {
                        let world_pos = level.tile_to_world_pos(active_pos, tile_pos);
                        draw_light_source_marker(world_pos, UNIT_PREVIEW_ALPHA, assets);
                    } else {
                        interactable.render(tile_pos, active_pos, level, assets);
                    }
                }
            }

            if self.is_placing_units && !self.is_editing_unit_properties() {
                if let Some(unit_spawn) = unit_preset_spawn(self.selected_unit_preset) {
                    let world_pos = level.tile_to_world_pos(active_pos, tile_pos);
                    let unit_name = unit_name(unit_spawn.unit_type, &unit_spawn.profile);

                    let preview_color = Color::new(1.0, 1.0, 1.0, UNIT_PREVIEW_ALPHA);
                    render_unit_spawn_sprite(&unit_spawn, world_pos, preview_color, assets);

                    let center = tile_center(world_pos);
                    let color = self.get_unit_owner_color(&unit_spawn.owner, UNIT_PREVIEW_ALPHA);
                    draw_circle(center.x, center.y, UNIT_MARKER_RADIUS_PREVIEW, color);

                    draw_scaled_text(
                        unit_name,
                        world_pos.x,
                        world_pos.y - 5.0,
                        UNIT_PREVIEW_LABEL_FONT,
                        &WHITE,
                        &assets.font,
                    );
                }
            }
        }

        if !self.is_enabled {
            return;
        }

        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };

        for (gate_tile, interactable) in &level.interactables {
            let Interactable::Gate(gate) = interactable else {
                continue;
            };
            let mut area_tile = gate.transfer_area_tile;
            let mut area_size = gate.transfer_area_size;
            let is_edited = self.selected_interactable_pos == Some(*gate_tile);
            let is_picked = self.transfer_area_gate == Some(*gate_tile);
            if is_edited {
                if let Some(tile) = self.get_vec2_property(GATE_AREA_TILE_PROPERTY) {
                    area_tile = tile;
                }
                if let Some(size) = self.get_vec2_property(GATE_AREA_SIZE_PROPERTY) {
                    area_size = size;
                }
            }
            let area_pos = level.tile_to_world_pos(active_pos, area_tile);
            let area_w = area_size.x as f32 * TILE_SIZE_X;
            let area_h = area_size.y as f32 * TILE_SIZE_Y;
            draw_rectangle_lines(
                area_pos.x,
                area_pos.y,
                area_w,
                area_h,
                2.0,
                styles.colors.yellow_hot,
            );
            if is_edited || is_picked {
                let mut fill_color = styles.colors.yellow_hot;
                fill_color.a = UNIT_PREVIEW_ALPHA;
                draw_rectangle(area_pos.x, area_pos.y, area_w, area_h, fill_color);
            }
        }
        if let Some(gate_tile) = self.transfer_area_gate {
            self.render_selection_highlight(
                level_manager,
                gate_tile,
                YELLOW,
                HIGHLIGHT_WIDTH_UNIT,
            );
        }
        if let Some(start) = self.transfer_area_drag_start {
            let end = self.transfer_area_drag_end;
            let drag_pos = level.tile_to_world_pos(active_pos, start.min(end));
            let drag_size = (start - end).abs() + IVec2::ONE;
            draw_rectangle_lines(
                drag_pos.x,
                drag_pos.y,
                drag_size.x as f32 * TILE_SIZE_X,
                drag_size.y as f32 * TILE_SIZE_Y,
                2.0,
                styles.colors.cyan_bright,
            );
        }

        for (unit_tile_pos, unit_spawn) in &level.unit_spawns {
            self.render_waypoints(level, active_pos, *unit_tile_pos, unit_spawn, styles);
        }

        for (unit_tile_pos, unit_spawn) in &level.unit_spawns {
            let world_pos = level.tile_to_world_pos(active_pos, *unit_tile_pos);

            let preview_color = Color::new(1.0, 1.0, 1.0, UNIT_PREVIEW_ALPHA);
            render_unit_spawn_sprite(unit_spawn, world_pos, preview_color, assets);

            let center = tile_center(world_pos);
            let color = self.get_unit_owner_color(&unit_spawn.owner, 1.0);
            draw_circle(center.x, center.y, UNIT_MARKER_RADIUS, color);

            let (p1, p2, p3) = Self::calculate_direction_triangle(
                center,
                unit_spawn.direction,
                UNIT_DIRECTION_OFFSET,
                UNIT_DIRECTION_TRIANGLE_SIZE,
            );
            draw_triangle(p1, p2, p3, WHITE);

            draw_scaled_text(
                unit_name(unit_spawn.unit_type, &unit_spawn.profile),
                world_pos.x,
                world_pos.y - 2.0,
                UNIT_LABEL_FONT,
                &WHITE,
                &assets.font,
            );
        }

        if let Some(edit_pos) = self.selected_interactable_pos {
            self.render_selection_highlight(
                level_manager,
                edit_pos,
                YELLOW,
                HIGHLIGHT_WIDTH_INTERACTABLE,
            );
        }

        if let Some(unit_pos) = self.selected_unit_pos {
            self.render_selection_highlight(
                level_manager,
                unit_pos,
                Color::new(0.0, 1.0, 0.5, 1.0),
                HIGHLIGHT_WIDTH_UNIT,
            );
        }
    }

    fn unit_spawn_waypoints(
        level_manager: &mut LevelManager,
        tile_pos: IVec2,
    ) -> Option<&mut Vec<IVec2>> {
        let active_pos = level_manager.view_sector_pos?;
        let level = level_manager.levels.get_mut(&active_pos)?;
        let unit_spawn = level.unit_spawns.get_mut(&tile_pos)?;
        Some(&mut unit_spawn.waypoints)
    }

    fn cycle_unit_preset(&mut self, direction: i32) {
        let new_index =
            Self::cycle_index(self.selected_unit_preset, unit_preset_count(), direction);
        self.set_palette_index(new_index);
    }

    fn place_unit_at(&self, level_manager: &mut LevelManager, tile_pos: IVec2) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        let Some(mut spawn_data) = unit_preset_spawn(self.selected_unit_preset) else {
            return;
        };
        spawn_data.tile_pos = tile_pos;
        level.unit_spawns.insert(tile_pos, spawn_data);
    }

    fn delete_unit_at(&self, level_manager: &mut LevelManager, tile_pos: IVec2) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        level.unit_spawns.remove(&tile_pos);
    }

    fn start_editing_unit(
        &mut self,
        level_manager: &LevelManager,
        tile_pos: IVec2,
        _debug_log: &mut DebugLog,
    ) {
        self.close_all_property_editors();
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };
        if level.unit_spawns.contains_key(&tile_pos) {
            self.selected_unit_pos = Some(tile_pos);
            self.populate_unit_properties(level_manager);
        }
    }

    fn populate_unit_properties(&mut self, level_manager: &LevelManager) {
        self.edit_properties.clear();
        self.edit_property_strings.clear();

        let Some(pos) = self.selected_unit_pos else {
            return;
        };
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };
        let Some(unit_spawn) = level.unit_spawns.get(&pos) else {
            return;
        };

        self.edit_properties.insert(
            "direction".to_string(),
            PropertyValue::Enum {
                current: unit_spawn.direction.to_index(),
                options: Direction::variant_names(),
            },
        );

        self.edit_properties.insert(
            "unit_type".to_string(),
            PropertyValue::Enum {
                current: unit_spawn.unit_type.to_sequential_index(),
                options: UnitType::variant_names(),
            },
        );

        self.edit_properties.insert(
            "owner".to_string(),
            PropertyValue::Enum {
                current: unit_spawn.owner.to_sequential_index(),
                options: UnitOwner::variant_names(),
            },
        );

        let profile = unit_spawn.profile;
        self.insert_enum_property(
            "design_kind",
            profile.design.kind.to_sequential_index(),
            UnitKind::variant_names(),
        );
        self.insert_enum_property(
            "design_weapon",
            profile.design.weapon.to_sequential_index(),
            Weapon::variant_names(),
        );
        self.insert_enum_property(
            "design_armor",
            profile.design.armor.to_sequential_index(),
            Armor::variant_names(),
        );
        self.insert_enum_property(
            "design_shielding",
            profile.design.shielding.to_sequential_index(),
            Shielding::variant_names(),
        );
        self.insert_enum_property(
            "design_chassis",
            profile.design.chassis.to_sequential_index(),
            Chassis::variant_names(),
        );
        self.insert_enum_property(
            "structure_sprite",
            profile.structure_sprite.to_sequential_index(),
            StructureSprite::variant_names(),
        );
        self.insert_number_property("structure_hp", profile.structure_hp as i32);
        self.insert_number_property("aggro_range", profile.aggro_range as i32);
        self.insert_number_property("reward_men", profile.reward.resources.men);
        self.insert_number_property("reward_iron", profile.reward.resources.iron);
        self.insert_number_property("reward_wood", profile.reward.resources.wood);
        self.insert_number_property("reward_mercury", profile.reward.resources.mercury);
        self.insert_number_property("reward_sulfur", profile.reward.resources.sulfur);
        let mut technology_names = vec![String::from("None")];
        technology_names.extend(Technology::variant_names());
        let technology_current = match profile.reward.technology {
            Some(technology) => technology.to_sequential_index() + 1,
            None => 0,
        };
        self.insert_enum_property("reward_technology", technology_current, technology_names);
        self.insert_enum_property(
            "final_fortress",
            profile.is_final_fortress as usize,
            vec![String::from("No"), String::from("Yes")],
        );
    }

    fn insert_enum_property(&mut self, key: &str, current: usize, options: Vec<String>) {
        self.edit_properties
            .insert(key.to_string(), PropertyValue::Enum { current, options });
    }

    fn insert_number_property(&mut self, key: &str, value: i32) {
        self.edit_properties
            .insert(key.to_string(), PropertyValue::Number);
        self.edit_property_strings
            .insert(key.to_string(), format!("{}", value));
    }

    fn enum_property(&self, key: &str) -> Option<usize> {
        if let Some(PropertyValue::Enum { current, .. }) = self.edit_properties.get(key) {
            return Some(*current);
        }
        None
    }

    fn number_property(&self, key: &str) -> Option<i32> {
        let Some(text) = self.edit_property_strings.get(key) else {
            return None;
        };
        text.trim().parse::<i32>().ok()
    }

    fn apply_unit_properties(&self, level_manager: &mut LevelManager, debug_log: &mut DebugLog) {
        let Some(pos) = self.selected_unit_pos else {
            return;
        };
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        let Some(unit_spawn) = level.unit_spawns.get_mut(&pos) else {
            return;
        };

        if let Some(PropertyValue::Enum { current, .. }) = self.edit_properties.get("direction") {
            unit_spawn.direction = Direction::from_index(*current);
        }

        if let Some(PropertyValue::Enum { current, .. }) = self.edit_properties.get("unit_type") {
            unit_spawn.unit_type = UnitType::from_sequential_index(*current);
        }

        if let Some(PropertyValue::Enum { current, .. }) = self.edit_properties.get("owner") {
            unit_spawn.owner = UnitOwner::from_sequential_index(*current);
        }

        let profile = &mut unit_spawn.profile;
        if let Some(index) = self.enum_property("design_kind") {
            profile.design.kind = UnitKind::from_sequential_index(index);
        }
        if unit_spawn.unit_type == UnitType::Soldier {
            profile.design.kind = UnitKind::Soldier;
        } else if unit_spawn.unit_type == UnitType::Machine {
            profile.design.kind = UnitKind::Machine;
        }
        if let Some(index) = self.enum_property("design_weapon") {
            profile.design.weapon = Weapon::from_sequential_index(index);
        }
        if let Some(index) = self.enum_property("design_armor") {
            profile.design.armor = Armor::from_sequential_index(index);
        }
        if let Some(index) = self.enum_property("design_shielding") {
            profile.design.shielding = Shielding::from_sequential_index(index);
        }
        if let Some(index) = self.enum_property("design_chassis") {
            profile.design.chassis = Chassis::from_sequential_index(index);
        }
        if let Some(index) = self.enum_property("structure_sprite") {
            profile.structure_sprite = StructureSprite::from_sequential_index(index);
        }
        if let Some(value) = self.number_property("structure_hp") {
            profile.structure_hp = value.clamp(0, u16::MAX as i32) as u16;
        }
        if unit_spawn.unit_type == UnitType::Structure && profile.structure_hp == 0 {
            profile.structure_hp = structure_profile(profile.structure_sprite).structure_hp;
        }
        if let Some(value) = self.number_property("aggro_range") {
            profile.aggro_range = value.clamp(0, u8::MAX as i32) as u8;
        }
        if let Some(value) = self.number_property("reward_men") {
            profile.reward.resources.men = value.max(0);
        }
        if let Some(value) = self.number_property("reward_iron") {
            profile.reward.resources.iron = value.max(0);
        }
        if let Some(value) = self.number_property("reward_wood") {
            profile.reward.resources.wood = value.max(0);
        }
        if let Some(value) = self.number_property("reward_mercury") {
            profile.reward.resources.mercury = value.max(0);
        }
        if let Some(value) = self.number_property("reward_sulfur") {
            profile.reward.resources.sulfur = value.max(0);
        }
        if let Some(index) = self.enum_property("reward_technology") {
            profile.reward.technology = if index == 0 {
                None
            } else {
                ALL_TECHNOLOGIES.get(index - 1).copied()
            };
        }
        if let Some(index) = self.enum_property("final_fortress") {
            profile.is_final_fortress = index == 1;
        }

        debug_log.log(format!(
            "Applied property changes to unit at ({}, {})",
            pos.x, pos.y
        ));
    }

    fn start_editing_level_properties(
        &mut self,
        level_manager: &LevelManager,
        _debug_log: &mut DebugLog,
    ) {
        self.close_all_property_editors();
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        if level_manager.get_level_at_pos(active_pos).is_some() {
            self.editing_level_properties = true;
            self.populate_level_properties(level_manager);
        }
    }

    fn populate_level_properties(&mut self, level_manager: &LevelManager) {
        self.edit_properties.clear();
        self.edit_property_strings.clear();

        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(active_pos) else {
            return;
        };

        self.edit_property_strings
            .insert("name".to_string(), level.name.clone());

        self.insert_vec2_property("grid_tiles", level.grid_tiles);

        self.edit_properties
            .insert("ambient_light_level".to_string(), PropertyValue::Number);
        self.edit_property_strings.insert(
            "ambient_light_level".to_string(),
            format!("{}", level.ambient_light_level),
        );

        self.edit_properties.insert(
            "background_type".to_string(),
            PropertyValue::Enum {
                current: level.background_type.to_index(),
                options: Background::variant_names(),
            },
        );

        self.edit_properties.insert(
            "music_track".to_string(),
            PropertyValue::Enum {
                current: level.music_track.to_sequential_index(),
                options: MusicTrack::variant_names(),
            },
        );
    }

    fn apply_level_properties(&self, level_manager: &mut LevelManager, debug_log: &mut DebugLog) {
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };

        if let Some(text) = self.edit_property_strings.get("name") {
            let Some(level) = level_manager.levels.get_mut(&active_pos) else {
                return;
            };
            let old_name = level.name.clone();

            if old_name != *text {
                let mut was_initial_level = false;
                if let Some(ls) = &level_manager.level_set {
                    was_initial_level = ls.initial_level_name == old_name;
                }

                level.name = text.clone();

                if was_initial_level {
                    self.set_current_as_initial(level_manager, debug_log);
                }
            }
        }

        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };

        if let Some(tiles) = self.get_vec2_property("grid_tiles") {
            if level.grid_tiles != tiles {
                level.grid_tiles = tiles;
            }
        }

        if let Some(text) = self.edit_property_strings.get("ambient_light_level") {
            if let Ok(val) = text.parse::<f32>() {
                level.ambient_light_level = val.clamp(0.0, 1.0);
            }
        }

        if let Some(PropertyValue::Enum { current, .. }) =
            self.edit_properties.get("background_type")
        {
            level.background_type = Background::from_index(*current);
        }

        if let Some(PropertyValue::Enum { current, .. }) = self.edit_properties.get("music_track") {
            level.music_track = MusicTrack::from_sequential_index(*current);
        }

        debug_log.log(format!(
            "Applied property changes to level '{}'",
            level.name
        ));
    }

    fn apply_edit_properties(
        &self,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        let Some(pos) = self.selected_interactable_pos else {
            return;
        };
        let Some(active_pos) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.levels.get_mut(&active_pos) else {
            return;
        };
        let Some(old_interactable) = level.interactables.get(&pos).cloned() else {
            return;
        };
        history.record_event_interactable(active_pos, pos, Some(&old_interactable));
        let grid_tiles = level.grid_tiles;
        let Some(interactable) = level.interactables.get_mut(&pos) else {
            return;
        };

        match interactable {
            Interactable::LightSource(light) => {
                if let Some(PropertyValue::Enum { current, .. }) =
                    self.edit_properties.get("intensity")
                {
                    light.intensity = LightIntensity::from_sequential_index(*current);
                }
            }
            Interactable::Rubble(_) => {}
            Interactable::Gate(gate) => {
                if let Some(PropertyValue::Enum { current, .. }) =
                    self.edit_properties.get("direction")
                {
                    gate.direction = Direction::from_sequential_index(*current);
                }
                let last_tile = grid_tiles.max(IVec2::ONE) - IVec2::ONE;
                if let Some(tile) = self.get_vec2_property(GATE_AREA_TILE_PROPERTY) {
                    gate.transfer_area_tile = tile.clamp(IVec2::ZERO, last_tile);
                }
                if let Some(size) = self.get_vec2_property(GATE_AREA_SIZE_PROPERTY) {
                    let room = (grid_tiles - gate.transfer_area_tile).max(IVec2::ONE);
                    gate.transfer_area_size = size.clamp(IVec2::ONE, room);
                }
            }
        }

        debug_log.log(format!(
            "Applied property changes to interactable at ({}, {})",
            pos.x, pos.y
        ));
    }

    fn ui_controls(&mut self) {
        if !self.show_controls {
            return;
        }

        let open = widgets::Window::new(hash!(), vec2(380.0, 50.0), vec2(260.0, 340.0))
            .label("Controls (H)")
            .titlebar(true)
            .close_button(true)
            .ui(&mut *root_ui(), |ui| {
                let mut section = "";
                for (group, key, description) in EDITOR_CONTROLS {
                    if group != &section {
                        if !section.is_empty() {
                            ui.separator();
                        }
                        if !group.is_empty() {
                            ui.label(None, group);
                        }
                        section = group;
                    }
                    ui.label(None, &format!("{:12} {}", key, description));
                }
            });

        self.show_controls = open;
    }

    fn palette_rect(&self) -> (f32, f32, f32, f32) {
        let mut rows = self.palette_page_len();
        if self.palette_rows_locked > rows {
            rows = self.palette_rows_locked;
        }

        let height = PALETTE_HEADER_H + rows as f32 * PALETTE_ROW_H;
        let y = ((VIEW_H - height) / 2.0).floor();
        (PALETTE_X, y, PALETTE_W, height)
    }

    fn palette_button_rect(&self, step: i32) -> (f32, f32, f32, f32) {
        let (x, y, w, _) = self.palette_rect();
        let slot = if step < 0 { 1.0 } else { 0.0 };
        let bx = x + w - PALETTE_PAD - PALETTE_BTN_W - slot * (PALETTE_BTN_W + PALETTE_BTN_GAP);
        let by = y + (PALETTE_HEADER_H - PALETTE_BTN_H) / 2.0;
        (bx, by, PALETTE_BTN_W, PALETTE_BTN_H)
    }

    fn palette_page_step_at_screen_point(&self, camera_manager: &CameraManager) -> Option<i32> {
        if !self.palette_is_open() || camera_manager.ui_zoom <= 0.0 {
            return None;
        }
        if self.palette_page_count() <= 1 {
            return None;
        }

        let virtual_mouse = virtual_mouse_position(camera_manager);

        for step in [-1, 1] {
            let (bx, by, bw, bh) = self.palette_button_rect(step);
            if virtual_mouse.x >= bx
                && virtual_mouse.x < bx + bw
                && virtual_mouse.y >= by
                && virtual_mouse.y < by + bh
            {
                return Some(step);
            }
        }
        None
    }

    pub fn palette_contains_screen_point(&self, camera_manager: &CameraManager) -> bool {
        if !self.palette_is_open() || camera_manager.ui_zoom <= 0.0 {
            return false;
        }

        let virtual_mouse = virtual_mouse_position(camera_manager);
        let (x, y, w, h) = self.palette_rect();

        virtual_mouse.x >= x
            && virtual_mouse.x < x + w
            && virtual_mouse.y >= y
            && virtual_mouse.y < y + h
    }

    fn palette_is_open(&self) -> bool {
        self.is_enabled
            && self.show_palette
            && (self.is_placing_tiles || self.is_placing_interactables || self.is_placing_units)
    }

    fn palette_total_count(&self) -> usize {
        if self.is_placing_tiles {
            return ALL_TILES.len();
        }
        if self.is_placing_interactables {
            return self.all_interactables.len();
        }
        if self.is_placing_units {
            return unit_preset_count();
        }
        0
    }

    fn palette_page(&self) -> usize {
        let pages = self.palette_page_count();
        if pages == 0 {
            return 0;
        }
        if self.palette_page_shown >= pages {
            return pages - 1;
        }
        self.palette_page_shown
    }

    fn palette_page_count(&self) -> usize {
        let total = self.palette_total_count();
        if total == 0 {
            return 0;
        }
        (total + PALETTE_PAGE - 1) / PALETTE_PAGE
    }

    fn palette_page_start(&self) -> usize {
        self.palette_page() * PALETTE_PAGE
    }

    fn palette_page_len(&self) -> usize {
        let total = self.palette_total_count();
        let start = self.palette_page_start();
        if start >= total {
            return 0;
        }
        let remaining = total - start;
        if remaining < PALETTE_PAGE {
            remaining
        } else {
            PALETTE_PAGE
        }
    }

    fn palette_row_at_screen_point(&self, camera_manager: &CameraManager) -> Option<usize> {
        if !self.palette_contains_screen_point(camera_manager) {
            return None;
        }

        let virtual_y = virtual_mouse_position(camera_manager).y;
        let (_, y, _, _) = self.palette_rect();
        let rows_top = y + PALETTE_HEADER_H;
        if virtual_y < rows_top {
            return None;
        }

        let row = ((virtual_y - rows_top) / PALETTE_ROW_H) as usize;
        if row >= self.palette_page_len() {
            return None;
        }
        Some(self.palette_page_start() + row)
    }

    fn palette_entry<'a>(&self, i: usize, assets: &'a AssetManager) -> Option<PaletteEntry<'a>> {
        if self.is_placing_tiles {
            let tile = *ALL_TILES.get(i)?;
            return Some(PaletteEntry {
                name: tile.name(),
                owner: None,
                icon: get_texture_for_tile(assets, tile),
                icon_source: None,
            });
        }
        if self.is_placing_interactables {
            let interactable = self.all_interactables.get(i)?;
            let (icon, icon_source) = interactable_icon(assets, interactable);
            return Some(PaletteEntry {
                name: interactable.name(),
                owner: None,
                icon,
                icon_source,
            });
        }
        let (owner, unit_type, structure_sprite) = unit_preset(i)?;
        let design = default_profile(unit_type, structure_sprite).design;
        let (name, icon) = match unit_type {
            UnitType::Soldier => (
                unit_type.name(),
                man_texture(assets, design.armor, design.weapon, owner),
            ),
            UnitType::Machine => (
                unit_type.name(),
                crewed_machine_texture(assets, &design, owner),
            ),
            UnitType::Structure => (
                structure_sprite.name(),
                structure_texture(assets, structure_sprite),
            ),
        };
        Some(PaletteEntry {
            name,
            owner: Some(owner),
            icon,
            icon_source: None,
        })
    }

    pub fn ui_palette(
        &self,
        assets: &AssetManager,
        styles: &Styles,
        camera_manager: &CameraManager,
    ) {
        if !self.palette_is_open() {
            return;
        }

        let ui_zoom = camera_manager.ui_zoom;
        let ui_offset_x = camera_manager.ui_offset_x;
        let ui_offset_y = camera_manager.ui_offset_y;

        let label = if self.is_placing_tiles {
            "Tiles"
        } else if self.is_placing_interactables {
            "Interactables"
        } else {
            "Units"
        };

        let selected = self.selected_palette_index();
        let page_start = self.palette_page_start();
        let page_len = self.palette_page_len();

        let (rect_x, rect_y, rect_w, rect_h) = self.palette_rect();
        let x = ui_offset_x + rect_x * ui_zoom;
        let y = ui_offset_y + rect_y * ui_zoom;
        let w = rect_w * ui_zoom;
        let h = rect_h * ui_zoom;

        draw_rectangle(x, y, w, h, styles.colors.blue_shadow);
        draw_rectangle_lines(
            x,
            y,
            w,
            h,
            PALETTE_BORDER * 2.0 * ui_zoom,
            styles.colors.gray_metal,
        );

        let page_count = self.palette_page_count();
        let header = if page_count > 1 {
            format!("{} {}/{}", label, self.palette_page() + 1, page_count)
        } else {
            label.to_string()
        };
        draw_scaled_text(
            &header,
            x + PALETTE_PAD * ui_zoom,
            y + (PALETTE_HEADER_H - 3.0) * ui_zoom,
            PALETTE_FONT * ui_zoom,
            &styles.colors.cyan_bright,
            &assets.font,
        );

        if page_count > 1 {
            let page = self.palette_page();
            for step in [-1, 1] {
                let (bx, by, bw, bh) = self.palette_button_rect(step);
                let live = if step < 0 {
                    page > 0
                } else {
                    page + 1 < page_count
                };

                let sx = ui_offset_x + bx * ui_zoom;
                let sy = ui_offset_y + by * ui_zoom;

                draw_rectangle(
                    sx,
                    sy,
                    bw * ui_zoom,
                    bh * ui_zoom,
                    if live {
                        styles.colors.blue_server
                    } else {
                        styles.colors.gray_dark
                    },
                );
                draw_scaled_text(
                    if step < 0 { "<" } else { ">" },
                    sx + 2.0 * ui_zoom,
                    sy + (bh - 1.0) * ui_zoom,
                    PALETTE_FONT * ui_zoom,
                    if live {
                        &styles.colors.cyan_bright
                    } else {
                        &styles.colors.gray_metal
                    },
                    &assets.font,
                );
            }
        }

        for row in 0..page_len {
            let i = page_start + row;
            let Some(entry) = self.palette_entry(i, assets) else {
                continue;
            };
            let row_y = y + (PALETTE_HEADER_H + row as f32 * PALETTE_ROW_H) * ui_zoom;

            if i == selected {
                draw_rectangle(
                    x + PALETTE_BORDER * ui_zoom,
                    row_y,
                    w - PALETTE_BORDER * 2.0 * ui_zoom,
                    PALETTE_ROW_H * ui_zoom,
                    styles.colors.blue_server,
                );
            }

            let sprite_pos = vec2(
                x + PALETTE_PAD * ui_zoom,
                row_y + ((PALETTE_ROW_H - PALETTE_SPRITE) / 2.0) * ui_zoom,
            );
            let size = icon_size(entry.icon, entry.icon_source);
            let centering = (vec2(PALETTE_SPRITE, PALETTE_SPRITE) - size) / 2.0 * ui_zoom;
            draw_texture_ex(
                entry.icon,
                sprite_pos.x + centering.x,
                sprite_pos.y + centering.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(size * ui_zoom),
                    source: entry.icon_source,
                    ..Default::default()
                },
            );

            let text_color = if i == selected {
                styles.colors.cyan_bright
            } else {
                styles.colors.gray_ui
            };
            let text_y = row_y + (PALETTE_ROW_H - 5.0) * ui_zoom;
            draw_scaled_text(
                entry.name,
                x + (PALETTE_PAD + PALETTE_SPRITE + 4.0) * ui_zoom,
                text_y,
                PALETTE_FONT * ui_zoom,
                &text_color,
                &assets.font,
            );
            if let Some(owner) = entry.owner {
                draw_scaled_text(
                    owner.name(),
                    x + PALETTE_OWNER_X * ui_zoom,
                    text_y,
                    PALETTE_FONT * ui_zoom,
                    &text_color,
                    &assets.font,
                );
            }
        }
    }

    fn selected_palette_index(&self) -> usize {
        if self.is_placing_tiles {
            let mut index = 0;
            for i in 0..ALL_TILES.len() {
                if ALL_TILES[i] == self.selected_tile_type {
                    index = i;
                    break;
                }
            }
            return index;
        }
        if self.is_placing_interactables {
            return self.selected_interactable_type;
        }
        self.selected_unit_preset
    }

    fn set_palette_index(&mut self, index: usize) {
        self.palette_page_shown = index / PALETTE_PAGE;

        if self.is_placing_tiles {
            if index < ALL_TILES.len() {
                self.selected_tile_type = ALL_TILES[index];
            }
            return;
        }
        if self.is_placing_interactables {
            if index < self.all_interactables.len() {
                self.selected_interactable_type = index;
            }
            return;
        }
        if index < unit_preset_count() {
            self.selected_unit_preset = index;
        }
    }

    pub fn ui(
        &mut self,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        if !self.is_enabled {
            return;
        }

        self.ui_controls();

        if !self.is_editing_properties() {
            return;
        }

        let window_size = vec2(320.0, 640.0);
        let window_pos = vec2(50.0, 50.0);

        let label = if self.editing_level_properties {
            "Edit Level Properties"
        } else if self.selected_unit_pos.is_some() {
            "Edit Unit"
        } else {
            "Edit Interactable"
        };

        widgets::Window::new(hash!(), window_pos, window_size)
            .label(label)
            .titlebar(true)
            .ui(&mut *root_ui(), |ui| {
                if self.editing_level_properties {
                    if let Some(text) = self.edit_property_strings.get_mut("name") {
                        ui.input_text(hash!("name"), "name", text);
                        ui.separator();
                    }
                }

                let mut keys: Vec<String> = self.edit_properties.keys().cloned().collect();
                keys.sort();

                for key in keys {
                    if let Some(prop_value) = self.edit_properties.get_mut(&key) {
                        match prop_value {
                            PropertyValue::Number => {
                                if let Some(text) = self.edit_property_strings.get_mut(&key) {
                                    ui.input_text(hash!(&key), &key, text);
                                }
                            }
                            PropertyValue::Enum { current, options } => {
                                let mut variants: Vec<&str> = Vec::with_capacity(options.len());
                                for s in options.iter() {
                                    variants.push(s.as_str());
                                }
                                let selected =
                                    ui.combo_box(hash!(&key), &key, &variants, Some(&mut *current));
                                *current = selected;
                            }
                            PropertyValue::Vec2(_val) => {
                                let x_key = format!("{}_x", key);
                                let y_key = format!("{}_y", key);

                                if let Some(x_text) = self.edit_property_strings.get_mut(&x_key) {
                                    ui.input_text(hash!(&x_key), &format!("{} X", key), x_text);
                                }
                                if let Some(y_text) = self.edit_property_strings.get_mut(&y_key) {
                                    ui.input_text(hash!(&y_key), &format!("{} Y", key), y_text);
                                }
                            }
                        }
                    }

                    ui.separator();
                }

                ui.separator();

                if ui.button(None, "Apply") {
                    if self.editing_level_properties {
                        self.apply_level_properties(level_manager, debug_log);
                    } else if self.selected_unit_pos.is_some() {
                        self.apply_unit_properties(level_manager, debug_log);
                    } else {
                        self.apply_edit_properties(level_manager, history, debug_log);
                    }
                }

                ui.same_line(0.0);

                if ui.button(None, "Close") {
                    self.close_all_property_editors();
                }
            });
    }
}
