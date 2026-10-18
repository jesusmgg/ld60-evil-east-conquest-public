use macroquad::prelude::*;
use nanoserde::{DeBin, SerBin};
use std::fmt::Write;
use std::mem;

use crate::assets::AssetManager;
use crate::audio_manager::{AudioManager, VOLUME_LEVEL_MAX};
use crate::camera_manager::CameraManager;
use crate::constants::{TILE_SIZE_X, TILE_SIZE_Y, VIEW_H, VIEW_W};
use crate::game_data::proxy::VecIVec2Proxy;
use crate::history_manager::snapshots::SnapshotKind;
use crate::history_manager::HistoryManager;
use crate::level::level_editor_manager::LevelEditorManager;
use crate::level::level_manager::LevelManager;
use crate::loading_manager::LoadingManager;
use crate::selector_manager::SelectorManager;
use crate::sfx::SfxEvent;
use crate::styles::Styles;
use crate::ui_draw::{
    draw_button, draw_panel, draw_tooltip, draw_ui_rect_lines, draw_ui_text, draw_ui_triangle,
    text_width, virtual_mouse_position, write_resource_amounts, RESOURCE_ICON_CHARS,
    UI_LINE_HEIGHT, UI_PADDING,
};
use crate::unit_design::{
    armor_stats, can_attack, chassis_stats, design_cost, is_design_available, shielding_stats,
    unit_max_hp, unit_move_period, unit_type_for, weapon_stats, Armor, Chassis, Economy, Shielding,
    Technology, UnitDesign, UnitKind, UnitProfile, Weapon, ALL_ARMORS, ALL_CHASSIS, ALL_SHIELDINGS,
    ALL_WEAPONS, DAMAGE_VS_MACHINE, DAMAGE_VS_SOLDIER, DAMAGE_VS_STRUCTURE, DESIGN_SLOTS_MAX,
    MEN_PER_UNIT, RESOURCE_COUNT, RESOURCE_MEN, RESOURCE_NAMES,
};
use crate::unit_manager::{
    draw_machine, draw_soldier_group, TrainOrder, UnitManager, UnitOwner, ARMY_UNIT_MAX,
};

const BUTTON_H: f32 = 13.0;
const ROW_H: f32 = 15.0;
pub const PANEL_Y: f32 = 21.0;
const DESIGN_BUTTON: Rect = Rect::new(4.0, 4.0, 44.0, BUTTON_H);
const TRAIN_BUTTON: Rect = Rect::new(52.0, 4.0, 40.0, BUTTON_H);
const SAVES_BUTTON: Rect = Rect::new(VIEW_W - 132.0, 4.0, 44.0, BUTTON_H);
const HELP_BUTTON: Rect = Rect::new(VIEW_W - 84.0, 4.0, 32.0, BUTTON_H);
const OPTIONS_BUTTON: Rect = Rect::new(VIEW_W - 48.0, 4.0, 44.0, BUTTON_H);
const UNIT_COUNT_SLOT_W: f32 = 60.0;
const RESOURCE_SLOT_W: f32 = 40.0;
const RESOURCE_BAR_W: f32 = UNIT_COUNT_SLOT_W + RESOURCE_SLOT_W * RESOURCE_COUNT as f32;
const RESOURCE_BAR: Rect = Rect::new(
    (VIEW_W - RESOURCE_BAR_W) / 2.0,
    4.0,
    RESOURCE_BAR_W,
    BUTTON_H,
);

const DESIGN_PANEL: Rect = Rect::new(4.0, PANEL_Y, 340.0, 156.0);
const DESIGN_LIST_W: f32 = 96.0;
const DESIGN_NEW_GAP: f32 = 6.0;
const DETAIL_X: f32 = 108.0;
const PART_LEFT_X: f32 = 156.0;
const PART_NAME_X: f32 = 172.0;
const PART_RIGHT_X: f32 = 280.0;
const DESIGN_STATS_ROW: usize = 4;
const DELETE_BUTTON: Rect = Rect::new(290.0, 160.0, 50.0, BUTTON_H);

const TRAIN_PANEL_X: f32 = 52.0;
const TRAIN_PANEL_W: f32 = 150.0;

const OPTIONS_RESET_GAP: f32 = 8.0;
const OPTIONS_PANEL: Rect = Rect::new(
    VIEW_W - 144.0,
    PANEL_Y,
    140.0,
    UI_PADDING * 2.0 + 3.0 * ROW_H + OPTIONS_RESET_GAP + BUTTON_H,
);
const OPTIONS_MASTER_ROW: usize = 0;
const OPTIONS_MUSIC_ROW: usize = 1;
const OPTIONS_SFX_ROW: usize = 2;
const OPTIONS_RESET_BUTTON: Rect = Rect::new(
    OPTIONS_PANEL.x + UI_PADDING,
    PANEL_Y + UI_PADDING + 3.0 * ROW_H + OPTIONS_RESET_GAP,
    OPTIONS_PANEL.w - UI_PADDING * 2.0,
    BUTTON_H,
);
const OPTIONS_TOGGLE_X: f32 = VIEW_W - 98.0;
const OPTIONS_MINUS_X: f32 = VIEW_W - 62.0;
const OPTIONS_LEVEL_X: f32 = VIEW_W - 45.0;
const OPTIONS_PLUS_X: f32 = VIEW_W - 25.0;

const HELP_LINES: [&str; 13] = [
    "Conquer the enemy fortress to the east.",
    "",
    "Take your army to a gate and click it to march forward.",
    "",
    "Design: make unit designs.",
    "",
    "Train: place units of a design.",
    "",
    "Destroy structures to collect resources and technologies.",
    "",
    "Left click: select a unit, place a unit, press a button, use a gate.",
    "",
    "Right click: move the selected unit, or attack an enemy.",
];
const HELP_PANEL: Rect = Rect::new(
    VIEW_W - 328.0,
    PANEL_Y,
    324.0,
    UI_PADDING * 2.0 + UI_LINE_HEIGHT * HELP_LINES.len() as f32,
);

const SAVE_ROWS: usize = 8;
const SAVE_GAP: f32 = 6.0;
const SAVE_SCROLL_Y: f32 =
    PANEL_Y + UI_PADDING + SAVE_ROWS as f32 * ROW_H - (ROW_H - BUTTON_H) + SAVE_GAP;
const SAVE_NAME_Y: f32 = SAVE_SCROLL_Y + BUTTON_H + SAVE_GAP;
const SAVES_PANEL: Rect = Rect::new(
    VIEW_W - 194.0,
    PANEL_Y,
    190.0,
    SAVE_NAME_Y + BUTTON_H + UI_PADDING - PANEL_Y,
);
const SAVE_UP_BUTTON: Rect = Rect::new(VIEW_W - 190.0, SAVE_SCROLL_Y, 40.0, BUTTON_H);
const SAVE_DOWN_BUTTON: Rect = Rect::new(VIEW_W - 146.0, SAVE_SCROLL_Y, 40.0, BUTTON_H);
const SAVE_DELETE_BUTTON: Rect = Rect::new(VIEW_W - 46.0, SAVE_SCROLL_Y, 40.0, BUTTON_H);
const SAVE_NAME_BOX: Rect = Rect::new(VIEW_W - 190.0, SAVE_NAME_Y, 124.0, BUTTON_H);
const SAVE_BUTTON: Rect = Rect::new(VIEW_W - 62.0, SAVE_NAME_Y, 26.0, BUTTON_H);
const LOAD_BUTTON: Rect = Rect::new(VIEW_W - 32.0, SAVE_NAME_Y, 26.0, BUTTON_H);
pub const SAVE_NAME_MAX: usize = 16;

const END_PANEL: Rect = Rect::new(VIEW_W / 2.0 - 100.0, VIEW_H / 2.0 - 34.0, 200.0, 52.0);
const PLAY_AGAIN_BUTTON: Rect = Rect::new(VIEW_W / 2.0 - 40.0, VIEW_H / 2.0 + 1.0, 80.0, BUTTON_H);
const LOAD_CHECKPOINT_BUTTON: Rect =
    Rect::new(VIEW_W / 2.0 - 94.0, VIEW_H / 2.0 + 1.0, 92.0, BUTTON_H);
const LOAD_A_SAVE_BUTTON: Rect = Rect::new(VIEW_W / 2.0 + 6.0, VIEW_H / 2.0 + 1.0, 88.0, BUTTON_H);

const RESET_MESSAGE: &str = "Reset the game? All saves will be deleted.";
const CONFIRM_PANEL: Rect = Rect::new(VIEW_W / 2.0 - 120.0, VIEW_H / 2.0 - 34.0, 240.0, 52.0);
const CONFIRM_CANCEL_BUTTON: Rect =
    Rect::new(VIEW_W / 2.0 - 64.0, VIEW_H / 2.0 + 1.0, 60.0, BUTTON_H);
const CONFIRM_ACCEPT_BUTTON: Rect =
    Rect::new(VIEW_W / 2.0 + 4.0, VIEW_H / 2.0 + 1.0, 60.0, BUTTON_H);

const SELL_BUTTON: Rect = Rect::new(8.0, VIEW_H - 21.0, 40.0, BUTTON_H);

const PLACING_ALPHA: f32 = 0.5;
const EAST_HINT_SECONDS: f32 = 20.0;
const GATHER_POPUP_SECONDS: f32 = 5.0;

#[derive(Clone, Default, SerBin, DeBin)]
pub struct PlayerData {
    pub designs: Vec<UnitDesign>,
    #[nserde(proxy = "VecIVec2Proxy")]
    pub visited_levels: Vec<IVec2>,
    pub is_east_hint_done: bool,
}

struct GatherPopup {
    world_pos: Vec2,
    text: String,
    seconds: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Closed,
    Design,
    Train,
    Options,
    Help,
    Saves,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Closed,
    ResetGame,
    DeleteSave(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EndState {
    Playing,
    Won,
    Lost,
}

pub struct PlayerManager {
    pub data: PlayerData,
    is_data_changed: bool,

    panel: Panel,
    end_state: EndState,
    confirm: Confirm,
    selected_design: usize,
    placing_design: Option<usize>,
    placing_problem: Option<&'static str>,
    selected_snapshot: Option<usize>,
    save_scroll: usize,
    save_name: String,
    is_typing: bool,
    pub is_mouse_captured: bool,
    is_mouse_over_hud: bool,
    is_button_clicked: bool,
    is_panel_closed: bool,
    ui_text: String,
    east_hint_seconds: f32,
    gather_popups: Vec<GatherPopup>,

    input_mouse: Vec2,
    input_click: bool,
    input_right_click: bool,
    input_escape: bool,
    input_backspace: bool,
    input_enter: bool,
    input_chars: Vec<char>,
    input_hovered_tile: Option<IVec2>,
}

impl PlayerManager {
    pub fn new() -> Self {
        Self {
            data: PlayerData::default(),
            is_data_changed: false,

            panel: Panel::Closed,
            end_state: EndState::Playing,
            confirm: Confirm::Closed,
            selected_design: 0,
            placing_design: None,
            placing_problem: None,
            selected_snapshot: None,
            save_scroll: 0,
            save_name: String::new(),
            is_typing: false,
            is_mouse_captured: false,
            is_mouse_over_hud: false,
            is_button_clicked: false,
            is_panel_closed: false,
            ui_text: String::new(),
            east_hint_seconds: 0.0,
            gather_popups: Vec::new(),

            input_mouse: Vec2::ZERO,
            input_click: false,
            input_right_click: false,
            input_escape: false,
            input_backspace: false,
            input_enter: false,
            input_chars: Vec::new(),
            input_hovered_tile: None,
        }
    }

    pub fn input(
        &mut self,
        camera_manager: &CameraManager,
        level_manager: &LevelManager,
        unit_manager: &UnitManager,
        selector: &SelectorManager,
        level_editor: &LevelEditorManager,
    ) {
        self.input_mouse = virtual_mouse_position(camera_manager);
        self.input_click = is_mouse_button_pressed(MouseButton::Left);
        self.input_right_click = is_mouse_button_pressed(MouseButton::Right);
        self.input_escape = is_key_pressed(KeyCode::Escape);
        self.input_backspace = is_key_pressed(KeyCode::Backspace);
        self.input_enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        self.input_chars.clear();
        while let Some(character) = get_char_pressed() {
            if self.is_typing && !character.is_control() {
                self.input_chars.push(character);
            }
        }
        self.input_hovered_tile = level_manager.tile_under_mouse(camera_manager);
        self.is_mouse_over_hud =
            !level_editor.is_enabled && self.is_over_hud(unit_manager, selector);
        self.is_mouse_captured =
            !level_editor.is_enabled && (self.is_mouse_over_hud || self.placing_design.is_some());
    }

    pub fn update(
        &mut self,
        dt: f32,
        loading_manager: &LoadingManager,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
        selector: &SelectorManager,
        level_editor: &LevelEditorManager,
        audio_manager: &mut AudioManager,
        history: &mut HistoryManager,
    ) {
        if !loading_manager.is_initialized {
            return;
        }
        if history.is_snapshot_loaded {
            self.load(unit_manager, level_manager, history);
            self.placing_design = None;
            self.gather_popups.clear();
        }
        self.update_gather_popups(dt, unit_manager, level_manager);
        if let Some(level_pos) = unit_manager.transfer_arrival {
            self.visit(level_pos);
        }
        self.end_state = EndState::Playing;
        if unit_manager.is_game_won() {
            self.end_state = EndState::Won;
        } else if unit_manager.is_game_lost() {
            self.end_state = EndState::Lost;
        }
        if level_editor.is_enabled {
            self.panel = Panel::Closed;
            self.placing_design = None;
            self.is_typing = false;
            self.confirm = Confirm::Closed;
        } else {
            self.update_hud(
                unit_manager,
                level_manager,
                selector,
                audio_manager,
                history,
            );
            if self.is_east_hint_visible(unit_manager, level_manager) {
                self.east_hint_seconds += dt;
                if self.east_hint_seconds >= EAST_HINT_SECONDS {
                    self.data.is_east_hint_done = true;
                    self.is_data_changed = true;
                }
            }
        }
        if self.is_data_changed {
            history.player_state = SerBin::serialize_bin(&self.data);
            history.write_head_state_from_world(
                unit_manager.turn_count,
                unit_manager,
                level_manager,
            );
            self.is_data_changed = false;
        }
    }

    fn update_gather_popups(
        &mut self,
        dt: f32,
        unit_manager: &mut UnitManager,
        level_manager: &LevelManager,
    ) {
        for gather in &unit_manager.pending_gathers {
            let mut text = String::new();
            let amounts = gather.resources.amounts();
            for resource_i in 0..RESOURCE_COUNT {
                if amounts[resource_i] == 0 {
                    continue;
                }
                if !text.is_empty() {
                    text.push(' ');
                }
                let _ = write!(text, "+{} {}", amounts[resource_i], RESOURCE_ICON_CHARS[resource_i]);
            }
            if let Some(technology) = gather.technology {
                if !text.is_empty() {
                    text.push('\n');
                }
                let _ = write!(text, "Stolen tech: {}", technology.display_name());
            }
            self.gather_popups.push(GatherPopup {
                world_pos: level_manager.tile_to_world(gather.level_pos, gather.tile),
                text,
                seconds: 0.0,
            });
        }
        unit_manager.pending_gathers.clear();

        let mut popup_i = 0;
        while popup_i < self.gather_popups.len() {
            self.gather_popups[popup_i].seconds += dt;
            if self.gather_popups[popup_i].seconds < GATHER_POPUP_SECONDS {
                popup_i += 1;
                continue;
            }
            self.gather_popups.remove(popup_i);
        }
    }

    pub fn load(
        &mut self,
        unit_manager: &UnitManager,
        level_manager: &mut LevelManager,
        history: &HistoryManager,
    ) {
        self.data = match PlayerData::deserialize_bin(&history.player_state) {
            Ok(data) => data,
            Err(_) => PlayerData::default(),
        };
        if let Some(level_pos) = unit_manager.army_level() {
            level_manager.view_sector_pos = Some(level_pos);
        } else if let Some(level_pos) = level_manager.initial_level_pos() {
            level_manager.view_sector_pos = Some(level_pos);
        }
        if self.data.visited_levels.is_empty() {
            if let Some(level_pos) = level_manager.view_sector_pos {
                self.visit(level_pos);
            }
        }
    }

    pub fn visit(&mut self, level_pos: IVec2) {
        if self.data.visited_levels.contains(&level_pos) {
            return;
        }
        self.data.visited_levels.push(level_pos);
        self.is_data_changed = true;
    }

    pub fn add_design(&mut self, design: UnitDesign) -> Option<usize> {
        if self.data.designs.len() >= DESIGN_SLOTS_MAX {
            return None;
        }
        self.data.designs.push(design);
        self.is_data_changed = true;
        Some(self.data.designs.len() - 1)
    }

    pub fn set_design(&mut self, design_i: usize, design: UnitDesign) {
        if design_i >= self.data.designs.len() {
            return;
        }
        self.data.designs[design_i] = design;
        self.is_data_changed = true;
    }

    pub fn delete_design(&mut self, design_i: usize) {
        if design_i >= self.data.designs.len() {
            return;
        }
        self.data.designs.remove(design_i);
        self.is_data_changed = true;
        self.placing_design = None;
        if self.selected_design > 0 && self.selected_design >= self.data.designs.len() {
            self.selected_design -= 1;
        }
    }

    fn update_hud(
        &mut self,
        unit_manager: &mut UnitManager,
        level_manager: &LevelManager,
        selector: &SelectorManager,
        audio_manager: &mut AudioManager,
        history: &mut HistoryManager,
    ) {
        if self.end_state != EndState::Playing {
            self.placing_design = None;
            if self.panel == Panel::Design || self.panel == Panel::Train {
                self.panel = Panel::Closed;
            }
        }
        self.is_button_clicked = false;
        self.is_panel_closed = false;
        if self.input_escape {
            if self.confirm != Confirm::Closed {
                self.confirm = Confirm::Closed;
            } else if self.placing_design.is_some() {
                self.placing_design = None;
            } else if self.is_typing {
                self.is_typing = false;
            } else {
                self.close_panel();
            }
        }
        if self.input_right_click {
            self.placing_design = None;
        }
        if self.is_typing {
            self.update_typing(history);
        }
        self.update_placing_problem(unit_manager, level_manager);

        if self.input_click {
            self.click_hud(
                unit_manager,
                level_manager,
                selector,
                audio_manager,
                history,
            );
        }
        if self.is_panel_closed {
            audio_manager.pending_sfx.push(SfxEvent::UiPanelClose);
        } else if self.is_button_clicked {
            audio_manager.pending_sfx.push(SfxEvent::UiButtonClick);
        }
    }

    pub fn is_keyboard_captured(&self) -> bool {
        self.is_typing || self.confirm != Confirm::Closed
    }

    fn close_panel(&mut self) {
        if self.panel == Panel::Closed {
            return;
        }
        self.panel = Panel::Closed;
        self.is_panel_closed = true;
    }

    fn click_button(&mut self, rect: Rect, is_enabled: bool) -> bool {
        if !is_enabled || !rect.contains(self.input_mouse) {
            return false;
        }
        self.is_button_clicked = true;
        true
    }

    fn click_hud(
        &mut self,
        unit_manager: &mut UnitManager,
        level_manager: &LevelManager,
        selector: &SelectorManager,
        audio_manager: &mut AudioManager,
        history: &mut HistoryManager,
    ) {
        self.is_typing = false;
        if self.confirm != Confirm::Closed {
            if self.click_button(CONFIRM_ACCEPT_BUTTON, true) {
                match self.confirm {
                    Confirm::Closed => {}
                    Confirm::ResetGame => history.input_reset_game = true,
                    Confirm::DeleteSave(snapshot_i) => {
                        history.input_delete_snapshot = Some(snapshot_i);
                        self.selected_snapshot = None;
                    }
                }
                self.confirm = Confirm::Closed;
            } else if self.click_button(CONFIRM_CANCEL_BUTTON, true) {
                self.confirm = Confirm::Closed;
            }
            return;
        }
        if self.placing_design.is_some() && !self.is_mouse_over_hud {
            self.place(unit_manager, level_manager);
            return;
        }
        let is_playing = self.end_state == EndState::Playing;
        if DESIGN_BUTTON.contains(self.input_mouse) {
            if self.click_button(DESIGN_BUTTON, is_playing) {
                self.toggle_panel(Panel::Design);
            }
            return;
        }
        if TRAIN_BUTTON.contains(self.input_mouse) {
            if self.click_button(TRAIN_BUTTON, is_playing) {
                self.toggle_panel(Panel::Train);
            }
            return;
        }
        if self.click_button(SAVES_BUTTON, true) {
            self.toggle_panel(Panel::Saves);
            return;
        }
        if self.click_button(HELP_BUTTON, true) {
            self.toggle_panel(Panel::Help);
            return;
        }
        if self.click_button(OPTIONS_BUTTON, true) {
            self.toggle_panel(Panel::Options);
            return;
        }
        match self.panel {
            Panel::Closed | Panel::Help => {}
            Panel::Design => self.click_design_panel(unit_manager),
            Panel::Train => self.click_train_panel(unit_manager),
            Panel::Options => self.click_options_panel(audio_manager),
            Panel::Saves => self.click_saves_panel(history),
        }
        if self.is_end_screen_visible() {
            self.click_end_screen(history);
        }
        if let Some(unit_i) = selector.selected_unit_i {
            if self.click_button(SELL_BUTTON, unit_manager.can_sell(unit_i)) {
                unit_manager.pending_sells.push(unit_i);
            }
        }
    }

    fn toggle_panel(&mut self, panel: Panel) {
        self.placing_design = None;
        if self.panel == panel {
            self.close_panel();
            return;
        }
        self.panel = panel;
        if panel == Panel::Saves {
            self.save_scroll = usize::MAX;
        }
    }

    fn update_typing(&mut self, history: &mut HistoryManager) {
        for char_i in 0..self.input_chars.len() {
            if self.save_name.chars().count() >= SAVE_NAME_MAX {
                break;
            }
            self.save_name.push(self.input_chars[char_i]);
        }
        if self.input_backspace {
            self.save_name.pop();
        }
        if self.input_enter {
            self.request_save(history);
        }
    }

    fn request_save(&mut self, history: &mut HistoryManager) {
        if !self.can_save(history) {
            return;
        }
        history.input_save_snapshot = Some(mem::take(&mut self.save_name));
        self.is_typing = false;
        self.save_scroll = usize::MAX;
    }

    fn update_placing_problem(&mut self, unit_manager: &UnitManager, level_manager: &LevelManager) {
        self.placing_problem = None;
        let Some(design_i) = self.placing_design else {
            return;
        };
        if design_i >= self.data.designs.len() {
            self.placing_design = None;
            return;
        }
        if unit_manager.army_unit_count() >= ARMY_UNIT_MAX {
            self.placing_problem = Some("Too many units");
            return;
        }
        if !unit_manager.can_afford(&self.data.designs[design_i]) {
            self.placing_problem = Some("Not enough resources");
            return;
        }
        let (Some(level_pos), Some(tile)) =
            (level_manager.view_sector_pos, self.input_hovered_tile)
        else {
            self.placing_problem = Some("Can't place a unit here");
            return;
        };
        if !unit_manager.is_placement_valid(level_manager, level_pos, tile) {
            self.placing_problem = Some("Can't place a unit here");
        }
    }

    fn place(&mut self, unit_manager: &mut UnitManager, level_manager: &LevelManager) {
        let Some(design_i) = self.placing_design else {
            return;
        };
        if self.placing_problem.is_some() {
            return;
        }
        let (Some(level_pos), Some(tile)) =
            (level_manager.view_sector_pos, self.input_hovered_tile)
        else {
            return;
        };
        unit_manager.pending_trains.push(TrainOrder {
            design: self.data.designs[design_i],
            level_pos,
            tile,
        });
        self.placing_design = None;
    }

    fn click_design_panel(&mut self, unit_manager: &UnitManager) {
        let count = self.data.designs.len();
        for design_i in 0..count {
            if self.click_button(design_row_rect(design_i), true) {
                self.selected_design = design_i;
                return;
            }
        }
        if count < DESIGN_SLOTS_MAX {
            let mut kind = None;
            if self.click_button(design_new_rect(count, 0), true) {
                kind = Some(UnitKind::Soldier);
            } else if self.click_button(design_new_rect(count, 1), true) {
                kind = Some(UnitKind::Machine);
            }
            if let Some(kind) = kind {
                if let Some(design_i) = self.add_design(UnitDesign::default_for(kind)) {
                    self.selected_design = design_i;
                }
                return;
            }
        }
        if self.selected_design >= count {
            return;
        }
        let mut design = self.data.designs[self.selected_design];
        for part_i in 0..part_count(design.kind) {
            let mut step = 0;
            if self.click_button(part_left_rect(part_i), true) {
                step = -1;
            } else if self.click_button(part_right_rect(part_i), true) {
                step = 1;
            }
            if step != 0 {
                cycle_part(&mut design, part_i, step, &unit_manager.economy);
                self.set_design(self.selected_design, design);
                return;
            }
        }
        if self.click_button(DELETE_BUTTON, true) {
            self.delete_design(self.selected_design);
        }
    }

    fn click_train_panel(&mut self, unit_manager: &UnitManager) {
        for design_i in 0..self.data.designs.len() {
            let can_afford = unit_manager.can_afford(&self.data.designs[design_i]);
            if !self.click_button(train_row_rect(design_i), can_afford) {
                continue;
            }
            self.placing_design = Some(design_i);
            self.close_panel();
            return;
        }
    }

    fn click_options_panel(&mut self, audio_manager: &mut AudioManager) {
        if self.click_button(options_toggle_rect(OPTIONS_MUSIC_ROW), true) {
            audio_manager.is_music_muted = !audio_manager.is_music_muted;
        }
        if self.click_button(options_toggle_rect(OPTIONS_SFX_ROW), true) {
            audio_manager.is_sfx_muted = !audio_manager.is_sfx_muted;
        }
        if self.click_button(OPTIONS_RESET_BUTTON, true) {
            self.confirm = Confirm::ResetGame;
        }
        for row in 0..3 {
            let level = match row {
                OPTIONS_MASTER_ROW => &mut audio_manager.master_level,
                OPTIONS_MUSIC_ROW => &mut audio_manager.music_level,
                _ => &mut audio_manager.sfx_level,
            };
            if self.click_button(options_minus_rect(row), *level > 0) {
                *level -= 1;
            }
            if self.click_button(options_plus_rect(row), *level < VOLUME_LEVEL_MAX) {
                *level += 1;
            }
        }
    }

    fn click_saves_panel(&mut self, history: &mut HistoryManager) {
        let count = history.snapshots.len();
        let first = first_save_row(self.save_scroll, count);
        for row in 0..SAVE_ROWS {
            if first + row >= count {
                break;
            }
            if self.click_button(save_row_rect(row), true) {
                self.selected_snapshot = Some(first + row);
                return;
            }
        }
        if count > SAVE_ROWS {
            if self.click_button(SAVE_UP_BUTTON, first > 0) {
                self.save_scroll = first.saturating_sub(1);
                return;
            }
            if self.click_button(SAVE_DOWN_BUTTON, first + SAVE_ROWS < count) {
                self.save_scroll = first + 1;
                return;
            }
        }
        if SAVE_NAME_BOX.contains(self.input_mouse) {
            self.is_typing = true;
            return;
        }
        if self.click_button(SAVE_BUTTON, self.can_save(history)) {
            self.request_save(history);
            return;
        }
        let selected_i = self.selected_save(history);
        if self.click_button(LOAD_BUTTON, selected_i.is_some()) {
            history.input_load_snapshot = selected_i;
            self.close_panel();
            return;
        }
        let delete_i = self.deletable_save(history);
        if self.click_button(SAVE_DELETE_BUTTON, delete_i.is_some()) {
            if let Some(snapshot_i) = delete_i {
                self.confirm = Confirm::DeleteSave(snapshot_i);
            }
        }
    }

    fn deletable_save(&self, history: &HistoryManager) -> Option<usize> {
        let snapshot_i = self.selected_save(history)?;
        if history.snapshots[snapshot_i].kind != SnapshotKind::Manual {
            return None;
        }
        Some(snapshot_i)
    }

    fn can_save(&self, history: &HistoryManager) -> bool {
        !self.save_name.is_empty() && history.can_save()
    }

    fn selected_save(&self, history: &HistoryManager) -> Option<usize> {
        let snapshot_i = self.selected_snapshot?;
        if snapshot_i >= history.snapshots.len() {
            return None;
        }
        Some(snapshot_i)
    }

    fn click_end_screen(&mut self, history: &mut HistoryManager) {
        match self.end_state {
            EndState::Playing => {}
            EndState::Won => {
                let new_game_i = history.find_snapshot(SnapshotKind::NewGame);
                if self.click_button(PLAY_AGAIN_BUTTON, new_game_i.is_some()) {
                    history.input_load_snapshot = new_game_i;
                }
            }
            EndState::Lost => {
                let snapshot_i = history.find_checkpoint();
                if self.click_button(LOAD_CHECKPOINT_BUTTON, snapshot_i.is_some()) {
                    history.input_load_snapshot = snapshot_i;
                } else if self.click_button(LOAD_A_SAVE_BUTTON, true) {
                    self.toggle_panel(Panel::Saves);
                }
            }
        }
    }

    fn is_end_screen_visible(&self) -> bool {
        self.end_state != EndState::Playing && self.panel != Panel::Saves
    }

    fn panel_rect(&self) -> Option<Rect> {
        match self.panel {
            Panel::Closed => None,
            Panel::Design => Some(DESIGN_PANEL),
            Panel::Train => Some(train_panel_rect(self.data.designs.len())),
            Panel::Options => Some(OPTIONS_PANEL),
            Panel::Help => Some(HELP_PANEL),
            Panel::Saves => Some(SAVES_PANEL),
        }
    }

    fn is_over_hud(&self, unit_manager: &UnitManager, selector: &SelectorManager) -> bool {
        if self.confirm != Confirm::Closed {
            return true;
        }
        let mouse = self.input_mouse;
        if DESIGN_BUTTON.contains(mouse)
            || TRAIN_BUTTON.contains(mouse)
            || SAVES_BUTTON.contains(mouse)
            || HELP_BUTTON.contains(mouse)
            || OPTIONS_BUTTON.contains(mouse)
            || RESOURCE_BAR.contains(mouse)
        {
            return true;
        }
        if let Some(rect) = self.panel_rect() {
            if rect.contains(mouse) {
                return true;
            }
        }
        if self.is_end_screen_visible() && END_PANEL.contains(mouse) {
            return true;
        }
        let Some(unit_i) = selector.selected_unit_i else {
            return false;
        };
        unit_manager.can_sell(unit_i) && SELL_BUTTON.contains(mouse)
    }

    pub fn render(&self, level_manager: &LevelManager, assets: &AssetManager, styles: &Styles) {
        let Some(design_i) = self.placing_design else {
            return;
        };
        if self.is_mouse_over_hud || design_i >= self.data.designs.len() {
            return;
        }
        let (Some(level_pos), Some(tile)) =
            (level_manager.view_sector_pos, self.input_hovered_tile)
        else {
            return;
        };
        let world_pos = level_manager.tile_to_world(level_pos, tile);
        let mut tint = styles.colors.cyan_bright;
        if self.placing_problem.is_some() {
            tint = styles.colors.red_alarm;
        }
        tint.a = PLACING_ALPHA;
        let design = &self.data.designs[design_i];
        match design.kind {
            UnitKind::Soldier => draw_soldier_group(
                assets,
                design,
                UnitOwner::P0,
                world_pos,
                MEN_PER_UNIT,
                tint,
                false,
            ),
            UnitKind::Machine => draw_machine(
                assets,
                design,
                UnitOwner::P0,
                world_pos,
                MEN_PER_UNIT,
                tint,
                false,
            ),
        }
        draw_rectangle_lines(
            world_pos.x,
            world_pos.y,
            TILE_SIZE_X,
            TILE_SIZE_Y,
            1.0,
            tint,
        );
    }

    pub fn ui(
        &mut self,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        selector: &SelectorManager,
        history: &HistoryManager,
        audio_manager: &AudioManager,
        level_editor: &LevelEditorManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        if level_editor.is_enabled {
            return;
        }
        self.ui_gather_popups(styles, assets, camera_manager);
        let is_playing = self.end_state == EndState::Playing;
        draw_button(
            DESIGN_BUTTON,
            "Design",
            is_playing,
            self.panel == Panel::Design,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            TRAIN_BUTTON,
            "Train",
            is_playing,
            self.panel == Panel::Train,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            SAVES_BUTTON,
            "Saves",
            true,
            self.panel == Panel::Saves,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            HELP_BUTTON,
            "Help",
            true,
            self.panel == Panel::Help,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            OPTIONS_BUTTON,
            "Options",
            true,
            self.panel == Panel::Options,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        self.ui_resources(unit_manager, styles, assets, camera_manager);

        match self.panel {
            Panel::Closed => {}
            Panel::Design => self.ui_design_panel(unit_manager, styles, assets, camera_manager),
            Panel::Train => self.ui_train_panel(unit_manager, styles, assets, camera_manager),
            Panel::Options => self.ui_options_panel(audio_manager, styles, assets, camera_manager),
            Panel::Help => self.ui_help_panel(styles, assets, camera_manager),
            Panel::Saves => self.ui_saves_panel(history, styles, assets, camera_manager),
        }
        if let Some(unit_i) = selector.selected_unit_i {
            if unit_manager.can_sell(unit_i) {
                draw_button(
                    SELL_BUTTON,
                    "Sell",
                    true,
                    false,
                    self.input_mouse,
                    styles,
                    assets,
                    camera_manager,
                );
            }
        }
        if self.is_end_screen_visible() {
            self.ui_end_screen(history, styles, assets, camera_manager);
        }
        self.ui_tutorial(unit_manager, level_manager, styles, assets, camera_manager);
        if self.confirm != Confirm::Closed {
            self.ui_confirm_dialog(history, styles, assets, camera_manager);
            return;
        }

        if self.write_tooltip(unit_manager, selector, history) {
            draw_tooltip(
                &self.ui_text,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }
    }

    fn ui_resources(
        &mut self,
        unit_manager: &UnitManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        draw_panel(RESOURCE_BAR, styles, camera_manager);
        self.ui_text.clear();
        let _ = write!(
            self.ui_text,
            "{}/{} units",
            unit_manager.army_unit_count(),
            ARMY_UNIT_MAX
        );
        draw_ui_text(
            &self.ui_text,
            Vec2::new(RESOURCE_BAR.x + UI_PADDING, RESOURCE_BAR.y + 1.0),
            styles.colors.cyan_bright,
            assets,
            camera_manager,
        );
        let amounts = unit_manager.economy.resources.amounts();
        for i in 0..amounts.len() {
            let slot = resource_slot_rect(i);
            self.ui_text.clear();
            let _ = write!(self.ui_text, "{}{}", amounts[i], RESOURCE_ICON_CHARS[i]);
            draw_ui_text(
                &self.ui_text,
                Vec2::new(slot.x + UI_PADDING, slot.y + 1.0),
                styles.colors.cyan_bright,
                assets,
                camera_manager,
            );
        }
    }

    fn ui_design_panel(
        &mut self,
        unit_manager: &UnitManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        draw_panel(DESIGN_PANEL, styles, camera_manager);
        let count = self.data.designs.len();
        for design_i in 0..count {
            let design = self.data.designs[design_i];
            let rect = design_row_rect(design_i);
            write_fitting_design_name(&design, rect.w, assets, &mut self.ui_text);
            draw_button(
                rect,
                &self.ui_text,
                true,
                design_i == self.selected_design,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }
        if count < DESIGN_SLOTS_MAX {
            draw_button(
                design_new_rect(count, 0),
                "New soldier",
                true,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
            draw_button(
                design_new_rect(count, 1),
                "New machine",
                true,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }
        let text_color = styles.colors.cyan_bright;
        if self.selected_design >= count {
            draw_ui_text(
                "Make a soldier or a machine",
                Vec2::new(DETAIL_X, row_y(0) + 1.0),
                text_color,
                assets,
                camera_manager,
            );
            return;
        }

        let design = self.data.designs[self.selected_design];
        draw_ui_text(
            design.kind.name(),
            Vec2::new(DETAIL_X, row_y(0) + 1.0),
            text_color,
            assets,
            camera_manager,
        );
        for part_i in 0..part_count(design.kind) {
            let y = row_y(part_i + 1);
            draw_ui_text(
                part_label(design.kind, part_i),
                Vec2::new(DETAIL_X, y + 1.0),
                text_color,
                assets,
                camera_manager,
            );
            draw_button(
                part_left_rect(part_i),
                "<",
                true,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
            draw_ui_text(
                part_name(&design, part_i),
                Vec2::new(PART_NAME_X, y + 1.0),
                text_color,
                assets,
                camera_manager,
            );
            draw_button(
                part_right_rect(part_i),
                ">",
                true,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }

        let unit_type = unit_type_for(design.kind);
        let weapon = weapon_stats(design.weapon);
        let mut line = Vec2::new(DETAIL_X, row_y(DESIGN_STATS_ROW) + 1.0);
        self.ui_text.clear();
        let _ = write!(
            self.ui_text,
            "HP {}, moves every {} turns",
            unit_max_hp(unit_type, &UnitProfile::new(design)),
            unit_move_period(unit_type, &design)
        );
        draw_ui_text(&self.ui_text, line, text_color, assets, camera_manager);
        line.y += UI_LINE_HEIGHT;
        self.ui_text.clear();
        let _ = write!(
            self.ui_text,
            "Range {}, attacks every {} turns",
            weapon.range, weapon.attack_period
        );
        draw_ui_text(&self.ui_text, line, text_color, assets, camera_manager);
        line.y += UI_LINE_HEIGHT;
        self.ui_text.clear();
        write_damage(weapon.damage, &mut self.ui_text);
        draw_ui_text(&self.ui_text, line, text_color, assets, camera_manager);
        line.y += UI_LINE_HEIGHT;
        self.ui_text.clear();
        self.ui_text.push_str("Cost ");
        write_resource_amounts(&design_cost(&design), &mut self.ui_text);
        draw_ui_text(&self.ui_text, line, text_color, assets, camera_manager);
        if !is_design_available(&design, &unit_manager.economy) {
            line.y += UI_LINE_HEIGHT;
            draw_ui_text(
                "A part is not unlocked",
                line,
                styles.colors.red_alarm,
                assets,
                camera_manager,
            );
        }
        draw_button(
            DELETE_BUTTON,
            "Delete",
            true,
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
    }

    fn ui_train_panel(
        &mut self,
        unit_manager: &UnitManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        let count = self.data.designs.len();
        draw_panel(train_panel_rect(count), styles, camera_manager);
        if count == 0 {
            draw_ui_text(
                "No designs yet",
                Vec2::new(TRAIN_PANEL_X + UI_PADDING, row_y(0) + 1.0),
                styles.colors.cyan_bright,
                assets,
                camera_manager,
            );
            return;
        }
        for design_i in 0..count {
            let design = self.data.designs[design_i];
            let rect = train_row_rect(design_i);
            write_fitting_design_name(&design, rect.w, assets, &mut self.ui_text);
            draw_button(
                rect,
                &self.ui_text,
                unit_manager.can_afford(&design),
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }
    }

    fn ui_options_panel(
        &mut self,
        audio_manager: &AudioManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        draw_panel(OPTIONS_PANEL, styles, camera_manager);
        let labels = ["Master", "Music", "Effects"];
        let levels = [
            audio_manager.master_level,
            audio_manager.music_level,
            audio_manager.sfx_level,
        ];
        let mutes = [
            None,
            Some(audio_manager.is_music_muted),
            Some(audio_manager.is_sfx_muted),
        ];
        for row in 0..3 {
            let y = row_y(row);
            draw_ui_text(
                labels[row],
                Vec2::new(OPTIONS_PANEL.x + UI_PADDING, y + 1.0),
                styles.colors.cyan_bright,
                assets,
                camera_manager,
            );
            if let Some(is_muted) = mutes[row] {
                let mut text = "On";
                if is_muted {
                    text = "Off";
                }
                draw_button(
                    options_toggle_rect(row),
                    text,
                    true,
                    !is_muted,
                    self.input_mouse,
                    styles,
                    assets,
                    camera_manager,
                );
            }
            draw_button(
                options_minus_rect(row),
                "-",
                levels[row] > 0,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
            self.ui_text.clear();
            let _ = write!(self.ui_text, "{}", levels[row]);
            draw_ui_text(
                &self.ui_text,
                Vec2::new(OPTIONS_LEVEL_X, y + 1.0),
                styles.colors.cyan_bright,
                assets,
                camera_manager,
            );
            draw_button(
                options_plus_rect(row),
                "+",
                levels[row] < VOLUME_LEVEL_MAX,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }
        draw_button(
            OPTIONS_RESET_BUTTON,
            "Reset game",
            true,
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
    }

    fn ui_help_panel(
        &self,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        draw_panel(HELP_PANEL, styles, camera_manager);
        for line_i in 0..HELP_LINES.len() {
            draw_ui_text(
                HELP_LINES[line_i],
                Vec2::new(
                    HELP_PANEL.x + UI_PADDING,
                    HELP_PANEL.y + UI_PADDING + line_i as f32 * UI_LINE_HEIGHT,
                ),
                styles.colors.cyan_bright,
                assets,
                camera_manager,
            );
        }
    }

    fn ui_saves_panel(
        &mut self,
        history: &HistoryManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        draw_panel(SAVES_PANEL, styles, camera_manager);
        let count = history.snapshots.len();
        if count == 0 {
            draw_ui_text(
                "No saves yet",
                Vec2::new(SAVES_PANEL.x + UI_PADDING, row_y(0) + 1.0),
                styles.colors.cyan_bright,
                assets,
                camera_manager,
            );
        }
        let first = first_save_row(self.save_scroll, count);
        for row in 0..SAVE_ROWS {
            let snapshot_i = first + row;
            if snapshot_i >= count {
                break;
            }
            let snapshot = &history.snapshots[snapshot_i];
            self.ui_text.clear();
            let _ = write!(self.ui_text, "{}, turn {}", snapshot.name, snapshot.turn);
            draw_button(
                save_row_rect(row),
                &self.ui_text,
                true,
                self.selected_snapshot == Some(snapshot_i),
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }
        if count > SAVE_ROWS {
            draw_button(
                SAVE_UP_BUTTON,
                "Up",
                first > 0,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
            draw_button(
                SAVE_DOWN_BUTTON,
                "Down",
                first + SAVE_ROWS < count,
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
        }

        draw_panel(SAVE_NAME_BOX, styles, camera_manager);
        let text_position = Vec2::new(SAVE_NAME_BOX.x + 2.0, SAVE_NAME_BOX.y + 1.0);
        if self.save_name.is_empty() && !self.is_typing {
            draw_ui_text(
                "Name",
                text_position,
                styles.colors.gray,
                assets,
                camera_manager,
            );
        } else {
            self.ui_text.clear();
            self.ui_text.push_str(&self.save_name);
            if self.is_typing {
                self.ui_text.push('_');
                draw_ui_rect_lines(SAVE_NAME_BOX, styles.colors.cyan_bright, camera_manager);
            }
            draw_ui_text(
                &self.ui_text,
                text_position,
                styles.colors.cyan_bright,
                assets,
                camera_manager,
            );
        }
        draw_button(
            SAVE_BUTTON,
            "Save",
            self.can_save(history),
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            LOAD_BUTTON,
            "Load",
            self.selected_save(history).is_some(),
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            SAVE_DELETE_BUTTON,
            "Delete",
            self.deletable_save(history).is_some(),
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
    }

    fn ui_end_screen(
        &mut self,
        history: &HistoryManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        draw_panel(END_PANEL, styles, camera_manager);
        let mut message = "The final fortress has fallen.";
        if self.end_state == EndState::Lost {
            message = "The army is gone and no Men are left.";
        }
        let width = text_width(message, assets);
        draw_ui_text(
            message,
            Vec2::new(
                END_PANEL.x + (END_PANEL.w - width) / 2.0,
                END_PANEL.y + UI_PADDING * 2.0,
            ),
            styles.colors.cyan_bright,
            assets,
            camera_manager,
        );
        if self.end_state == EndState::Won {
            draw_button(
                PLAY_AGAIN_BUTTON,
                "Play again",
                history.find_snapshot(SnapshotKind::NewGame).is_some(),
                false,
                self.input_mouse,
                styles,
                assets,
                camera_manager,
            );
            return;
        }
        draw_button(
            LOAD_CHECKPOINT_BUTTON,
            "Load checkpoint",
            history.find_checkpoint().is_some(),
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            LOAD_A_SAVE_BUTTON,
            "Load a save",
            true,
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
    }

    fn ui_confirm_dialog(
        &mut self,
        history: &HistoryManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        self.ui_text.clear();
        let mut accept_label = "Reset";
        if let Confirm::DeleteSave(snapshot_i) = self.confirm {
            accept_label = "Delete";
            if snapshot_i < history.snapshots.len() {
                let snapshot = &history.snapshots[snapshot_i];
                let _ = write!(
                    self.ui_text,
                    "Delete {}, turn {}?",
                    snapshot.name, snapshot.turn
                );
            }
        } else {
            self.ui_text.push_str(RESET_MESSAGE);
        }
        draw_panel(CONFIRM_PANEL, styles, camera_manager);
        let width = text_width(&self.ui_text, assets);
        draw_ui_text(
            &self.ui_text,
            Vec2::new(
                CONFIRM_PANEL.x + (CONFIRM_PANEL.w - width) / 2.0,
                CONFIRM_PANEL.y + UI_PADDING * 2.0,
            ),
            styles.colors.cyan_bright,
            assets,
            camera_manager,
        );
        draw_button(
            CONFIRM_ACCEPT_BUTTON,
            accept_label,
            true,
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
        draw_button(
            CONFIRM_CANCEL_BUTTON,
            "Cancel",
            true,
            false,
            self.input_mouse,
            styles,
            assets,
            camera_manager,
        );
    }

    fn ui_gather_popups(&self, styles: &Styles, assets: &AssetManager, camera_manager: &CameraManager) {
        let view_rect = camera_manager.world_view_rect(0.0);
        for popup in &self.gather_popups {
            if !view_rect.contains(popup.world_pos) {
                continue;
            }
            let anchor = popup.world_pos - view_rect.point();
            draw_tooltip(&popup.text, anchor, styles, assets, camera_manager);
        }
    }

    fn is_east_hint_visible(&self, unit_manager: &UnitManager, level_manager: &LevelManager) -> bool {
        !self.data.is_east_hint_done
            && self.panel == Panel::Closed
            && self.placing_design.is_none()
            && self.end_state == EndState::Playing
            && !self.data.designs.is_empty()
            && unit_manager.has_trained(level_manager)
            && self.data.visited_levels.len() <= 1
    }

    fn ui_tutorial(
        &self,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        styles: &Styles,
        assets: &AssetManager,
        camera_manager: &CameraManager,
    ) {
        if self.panel != Panel::Closed
            || self.placing_design.is_some()
            || self.end_state != EndState::Playing
        {
            return;
        }
        let color = styles.colors.yellow_hot;
        if self.data.designs.is_empty() || !unit_manager.has_trained(level_manager) {
            let mut button = TRAIN_BUTTON;
            let mut text = "Train a unit";
            if self.data.designs.is_empty() {
                button = DESIGN_BUTTON;
                text = "Design a unit";
            }
            let tip = Vec2::new(button.x + button.w / 2.0, button.bottom() + 2.0);
            draw_ui_triangle(
                tip,
                Vec2::new(tip.x - 5.0, tip.y + 8.0),
                Vec2::new(tip.x + 5.0, tip.y + 8.0),
                color,
                camera_manager,
            );
            draw_tooltip(
                text,
                Vec2::new(button.x - 8.0, tip.y + 2.0),
                styles,
                assets,
                camera_manager,
            );
            return;
        }
        if self.is_east_hint_visible(unit_manager, level_manager) {
            let tip = Vec2::new(VIEW_W - 4.0, VIEW_H / 2.0);
            draw_ui_triangle(
                tip,
                Vec2::new(tip.x - 8.0, tip.y - 6.0),
                Vec2::new(tip.x - 8.0, tip.y + 6.0),
                color,
                camera_manager,
            );
            draw_tooltip(
                "Go east and conquer",
                Vec2::new(VIEW_W, tip.y - 34.0),
                styles,
                assets,
                camera_manager,
            );
        }
    }

    fn write_tooltip(
        &mut self,
        unit_manager: &UnitManager,
        selector: &SelectorManager,
        history: &HistoryManager,
    ) -> bool {
        let mouse = self.input_mouse;
        self.ui_text.clear();
        if self.placing_design.is_some() && !self.is_mouse_over_hud {
            if let Some(problem) = self.placing_problem {
                self.ui_text.push_str(problem);
                return true;
            }
            return false;
        }
        for i in 0..RESOURCE_COUNT {
            if resource_slot_rect(i).contains(mouse) {
                self.ui_text.push_str(RESOURCE_NAMES[i]);
                return true;
            }
        }
        if DESIGN_BUTTON.contains(mouse) {
            self.ui_text.push_str("Design units");
            return true;
        }
        if TRAIN_BUTTON.contains(mouse) {
            self.ui_text.push_str("Train a unit from a design");
            return true;
        }
        if SAVES_BUTTON.contains(mouse) {
            self.ui_text.push_str("Save and load");
            return true;
        }
        if HELP_BUTTON.contains(mouse) {
            self.ui_text.push_str("How to play");
            return true;
        }
        if OPTIONS_BUTTON.contains(mouse) {
            self.ui_text.push_str("Sound and reset");
            return true;
        }
        match self.panel {
            Panel::Design => {
                if self.selected_design < self.data.designs.len() {
                    let design = self.data.designs[self.selected_design];
                    for part_i in 0..part_count(design.kind) {
                        if part_row_rect(part_i).contains(mouse) {
                            write_part_tooltip(&design, part_i, &mut self.ui_text);
                            return true;
                        }
                    }
                }
            }
            Panel::Train => {
                for design_i in 0..self.data.designs.len() {
                    if train_row_rect(design_i).contains(mouse) {
                        let design = self.data.designs[design_i];
                        write_train_tooltip(&design, unit_manager, &mut self.ui_text);
                        return true;
                    }
                }
            }
            Panel::Saves => {
                if SAVE_BUTTON.contains(mouse) {
                    if !history.can_save() {
                        self.ui_text.push_str("Not enough storage left to save");
                        return true;
                    }
                    if let Some(count) = history.saves_left() {
                        let _ = write!(self.ui_text, "About {} more saves fit", count);
                        return true;
                    }
                }
            }
            Panel::Options => {
                if OPTIONS_RESET_BUTTON.contains(mouse) {
                    self.ui_text.push_str("Delete every save and start over");
                    return true;
                }
            }
            Panel::Closed | Panel::Help => {}
        }
        if let Some(unit_i) = selector.selected_unit_i {
            if unit_manager.can_sell(unit_i) && SELL_BUTTON.contains(mouse) {
                self.ui_text.push_str("Sell for ");
                write_resource_amounts(&unit_manager.sell_refund(unit_i), &mut self.ui_text);
                return true;
            }
        }
        if unit_manager.transfer_tooltip.is_some() {
            return false;
        }
        let Some(unit_i) = selector.hovered_unit_i else {
            return false;
        };
        if unit_i >= unit_manager.len() || unit_manager.is_dead[unit_i] {
            return false;
        }
        write_unit_stats(unit_manager, unit_i, &mut self.ui_text);
        true
    }
}

fn first_save_row(save_scroll: usize, count: usize) -> usize {
    save_scroll.min(count.saturating_sub(SAVE_ROWS))
}

fn row_y(row: usize) -> f32 {
    PANEL_Y + UI_PADDING + row as f32 * ROW_H
}

fn design_row_rect(row: usize) -> Rect {
    Rect::new(
        DESIGN_PANEL.x + UI_PADDING,
        row_y(row),
        DESIGN_LIST_W,
        BUTTON_H,
    )
}

fn design_new_rect(design_count: usize, new_i: usize) -> Rect {
    let mut rect = design_row_rect(design_count + new_i);
    if design_count > 0 {
        rect.y += DESIGN_NEW_GAP;
    }
    rect
}

fn part_left_rect(part_i: usize) -> Rect {
    Rect::new(PART_LEFT_X, row_y(part_i + 1), BUTTON_H, BUTTON_H)
}

fn part_right_rect(part_i: usize) -> Rect {
    Rect::new(PART_RIGHT_X, row_y(part_i + 1), BUTTON_H, BUTTON_H)
}

fn part_row_rect(part_i: usize) -> Rect {
    Rect::new(
        DETAIL_X,
        row_y(part_i + 1),
        PART_RIGHT_X + BUTTON_H - DETAIL_X,
        BUTTON_H,
    )
}

fn train_panel_rect(design_count: usize) -> Rect {
    let rows = design_count.max(1) as f32;
    Rect::new(
        TRAIN_PANEL_X,
        PANEL_Y,
        TRAIN_PANEL_W,
        UI_PADDING * 2.0 + rows * ROW_H - (ROW_H - BUTTON_H),
    )
}

fn train_row_rect(row: usize) -> Rect {
    Rect::new(
        TRAIN_PANEL_X + UI_PADDING,
        row_y(row),
        TRAIN_PANEL_W - UI_PADDING * 2.0,
        BUTTON_H,
    )
}

fn options_toggle_rect(row: usize) -> Rect {
    Rect::new(OPTIONS_TOGGLE_X, row_y(row), 30.0, BUTTON_H)
}

fn options_minus_rect(row: usize) -> Rect {
    Rect::new(OPTIONS_MINUS_X, row_y(row), BUTTON_H, BUTTON_H)
}

fn options_plus_rect(row: usize) -> Rect {
    Rect::new(OPTIONS_PLUS_X, row_y(row), BUTTON_H, BUTTON_H)
}

fn save_row_rect(row: usize) -> Rect {
    Rect::new(
        SAVES_PANEL.x + UI_PADDING,
        row_y(row),
        SAVES_PANEL.w - UI_PADDING * 2.0,
        BUTTON_H,
    )
}

fn write_fitting_design_name(
    design: &UnitDesign,
    width: f32,
    assets: &AssetManager,
    output: &mut String,
) {
    output.clear();
    design.write_display_name(output);
    if text_width(output, assets) > width - UI_PADDING {
        output.clear();
        output.push_str(design.weapon.display_name());
    }
}

fn write_damage(damage: [i32; 3], output: &mut String) {
    let _ = write!(
        output,
        "Damage {} soldier, {} machine, {} structure",
        damage[DAMAGE_VS_SOLDIER], damage[DAMAGE_VS_MACHINE], damage[DAMAGE_VS_STRUCTURE]
    );
}

fn write_unit_stats(unit_manager: &UnitManager, unit_i: usize, output: &mut String) {
    let unit_type = unit_manager.unit_type[unit_i];
    let profile = &unit_manager.profile[unit_i];
    let design = profile.design;
    let is_player = unit_manager.owner[unit_i] == UnitOwner::P0;

    if is_player {
        output.push_str("Your ");
    } else {
        output.push_str("Enemy ");
    }
    let _ = write!(
        output,
        "{}  HP {}/{}",
        unit_manager.name(unit_i),
        unit_manager.hp[unit_i],
        unit_manager.max_hp(unit_i)
    );
    let is_crewed = !unit_type.is_structure();
    if is_crewed {
        let _ = write!(
            output,
            "  {}{}\n",
            unit_manager.men(unit_i),
            RESOURCE_ICON_CHARS[RESOURCE_MEN]
        );
        design.write_display_name(output);
    }

    if can_attack(unit_type, &design) {
        let weapon = weapon_stats(design.weapon);
        let attack_period = unit_manager.effective_attack_period(unit_i, weapon.attack_period);
        let _ = write!(
            output,
            "\n{}, range {}, every {} turns\n",
            design.weapon.display_name(),
            weapon.range,
            attack_period
        );
        write_damage(weapon.damage, output);
    }

    let reward = profile.reward;
    if !is_player && (!reward.resources.is_empty() || reward.technology.is_some()) {
        output.push_str("\nReward ");
        if !reward.resources.is_empty() {
            write_resource_amounts(&reward.resources, output);
        }
        if let Some(technology) = reward.technology {
            if !reward.resources.is_empty() {
                output.push_str(", ");
            }
            output.push_str(technology.display_name());
        }
    }
    if can_attack(unit_type, &design) {
        let _ = write!(output, "\n{} kills", unit_manager.kills[unit_i]);
    }
}

fn resource_slot_rect(resource_i: usize) -> Rect {
    Rect::new(
        RESOURCE_BAR.x + UNIT_COUNT_SLOT_W + resource_i as f32 * RESOURCE_SLOT_W,
        RESOURCE_BAR.y,
        RESOURCE_SLOT_W,
        BUTTON_H,
    )
}

fn write_train_tooltip(design: &UnitDesign, unit_manager: &UnitManager, output: &mut String) {
    let Some(cost) = unit_manager.train_cost(design) else {
        output.push_str("No Men left");
        return;
    };
    output.push_str("Cost ");
    write_resource_amounts(&cost, output);
    if !is_design_available(design, &unit_manager.economy) {
        output.push_str("\nA part is not unlocked");
    } else if !unit_manager.economy.resources.covers(&cost) {
        output.push_str("\nNot enough resources");
    }
}

fn part_count(kind: UnitKind) -> usize {
    match kind {
        UnitKind::Soldier => 2,
        UnitKind::Machine => 3,
    }
}

fn part_label(kind: UnitKind, part_i: usize) -> &'static str {
    match (kind, part_i) {
        (_, 0) => "Weapon",
        (UnitKind::Soldier, _) => "Armor",
        (UnitKind::Machine, 1) => "Shielding",
        (UnitKind::Machine, _) => "Chassis",
    }
}

fn part_name(design: &UnitDesign, part_i: usize) -> &'static str {
    match (design.kind, part_i) {
        (_, 0) => design.weapon.display_name(),
        (UnitKind::Soldier, _) => design.armor.display_name(),
        (UnitKind::Machine, 1) => design.shielding.display_name(),
        (UnitKind::Machine, _) => design.chassis.display_name(),
    }
}

fn write_part_tooltip(design: &UnitDesign, part_i: usize, output: &mut String) {
    match (design.kind, part_i) {
        (_, 0) => {
            let stats = weapon_stats(design.weapon);
            let _ = write!(
                output,
                "Range {}, attacks every {} turns\n",
                stats.range, stats.attack_period
            );
            write_damage(stats.damage, output);
            if stats.splash_radius_squared > 0 {
                output.push_str("\nHits everything around the target, half per tile out");
            }
            if stats.attacks_while_moving {
                output.push_str("\nAttacks while moving");
            }
            if stats.armor_bonus_hp > 0 {
                let _ = write!(output, "\nAdds {} HP", stats.armor_bonus_hp);
            }
            output.push_str("\nCost ");
            write_resource_amounts(&stats.cost, output);
        }
        (UnitKind::Soldier, _) => {
            let stats = armor_stats(design.armor);
            let _ = write!(
                output,
                "HP {}, moves every {} turns\nCost ",
                stats.hp, stats.move_period
            );
            write_resource_amounts(&stats.cost, output);
        }
        (UnitKind::Machine, 1) => {
            let stats = shielding_stats(design.shielding);
            let _ = write!(output, "HP {}\nCost ", stats.hp);
            write_resource_amounts(&stats.cost, output);
        }
        (UnitKind::Machine, _) => {
            let stats = chassis_stats(design.chassis);
            let _ = write!(output, "Moves every {} turns\nCost ", stats.move_period);
            write_resource_amounts(&stats.cost, output);
        }
    }
}

fn cycle_part(design: &mut UnitDesign, part_i: usize, step: i32, economy: &Economy) {
    match (design.kind, part_i) {
        (_, 0) => design.weapon = cycled_weapon(design.weapon, design.kind, step, economy),
        (UnitKind::Soldier, _) => {
            let index = design.armor.to_sequential_index();
            design.armor = cycled_part(&ALL_ARMORS, index, step, armor_technology, economy)
        }
        (UnitKind::Machine, 1) => {
            let index = design.shielding.to_sequential_index();
            design.shielding =
                cycled_part(&ALL_SHIELDINGS, index, step, shielding_technology, economy)
        }
        (UnitKind::Machine, _) => {
            let index = design.chassis.to_sequential_index();
            design.chassis = cycled_part(&ALL_CHASSIS, index, step, chassis_technology, economy)
        }
    }
}

fn cycled_weapon(current: Weapon, kind: UnitKind, step: i32, economy: &Economy) -> Weapon {
    let count = ALL_WEAPONS.len() as i32;
    let mut index = current.to_sequential_index() as i32;
    for _ in 0..count {
        index = (index + step).rem_euclid(count);
        let stats = weapon_stats(ALL_WEAPONS[index as usize]);
        if stats.kind == kind && economy.is_unlocked(stats.technology) {
            return ALL_WEAPONS[index as usize];
        }
    }
    current
}

fn armor_technology(armor: Armor) -> Option<Technology> {
    armor_stats(armor).technology
}

fn shielding_technology(shielding: Shielding) -> Option<Technology> {
    shielding_stats(shielding).technology
}

fn chassis_technology(chassis: Chassis) -> Option<Technology> {
    chassis_stats(chassis).technology
}

fn cycled_part<T: Copy>(
    all: &[T],
    current_index: usize,
    step: i32,
    technology_of: fn(T) -> Option<Technology>,
    economy: &Economy,
) -> T {
    let count = all.len() as i32;
    let mut index = current_index as i32;
    for _ in 0..count {
        index = (index + step).rem_euclid(count);
        if economy.is_unlocked(technology_of(all[index as usize])) {
            return all[index as usize];
        }
    }
    all[current_index]
}
