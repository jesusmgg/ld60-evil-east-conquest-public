use macroquad::input::{is_key_pressed, KeyCode};
use macroquad::math::IVec2;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

use crate::constants::{
    DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y, HISTORY_AUTO_CHECK_INTERVAL_TURNS,
    HISTORY_AUTO_CHECK_TURNS,
};
use crate::debug::{DebugLog, LogLevel};
use crate::enums::Direction;
use crate::gpu_path_manager::{GpuPathManager, FIELD_SECTORS_X, FIELD_SECTORS_Y};
use crate::history_manager::HistoryManager;
use crate::level::level_manager::LevelManager;
use crate::loading_manager::LoadingManager;
use crate::player_manager::{PlayerManager, SAVE_NAME_MAX};
use crate::selector_manager::SelectorManager;
use crate::unit_design::{
    technology_bit, Armor, Chassis, Resources, Shielding, Technology, UnitDesign, UnitKind,
    UnitProfile, Weapon, ALL_ARMORS, ALL_CHASSIS, ALL_SHIELDINGS, ALL_TECHNOLOGIES, ALL_UNIT_KINDS,
    ALL_WEAPONS, MEN_PER_UNIT,
};
use crate::unit_manager::{
    GoalNode, MoveOrder, TargetOrder, TrainOrder, UnitManager, UnitOwner, UnitType,
};

const DEFAULT_DIR: &str = "debug-control";

const MAX_PARTIAL_BYTES: usize = 64 * 1024;

const MAX_STEPS: u32 = 1000;
const MAX_FORCED_TURNS: u32 = 60_000;

pub struct DebugControlManager {
    pub is_enabled: bool,
    pub is_paused: bool,
    pub steps_pending: u32,
    pub pending: Vec<String>,
    input_undo: bool,

    pub epoch: u32,

    pub historycheck_baseline: Option<Vec<u8>>,
    pub historycheck_baseline_turn: u32,
    pub historycheck_baseline_player_state: Vec<u8>,
    pub historycheck_turns_remaining: u32,
    pub historycheck_total_turns: u32,
    pub is_historycheck_rewinding: bool,
    pub is_undo_output_suppressed: bool,
    pub rewinds_pending: u32,
    pub rewind_started_at: u32,

    cmd_seq: u64,
    hello_sent: bool,
    in_path: String,
    in_cursor: u64,
    in_partial: String,
    out: Option<File>,
}

impl DebugControlManager {
    pub fn new() -> Self {
        let mut control = Self {
            is_enabled: false,
            is_paused: false,
            steps_pending: 0,
            pending: Vec::new(),
            input_undo: false,
            epoch: 0,
            historycheck_baseline: None,
            historycheck_baseline_turn: 0,
            historycheck_baseline_player_state: Vec::new(),
            historycheck_turns_remaining: 0,
            historycheck_total_turns: 0,
            is_historycheck_rewinding: false,
            is_undo_output_suppressed: false,
            rewinds_pending: 0,
            rewind_started_at: 0,
            cmd_seq: 0,
            hello_sent: false,
            in_path: String::new(),
            in_cursor: 0,
            in_partial: String::new(),
            out: None,
        };

        if cfg!(target_arch = "wasm32") {
            return control;
        }

        let mut dir = String::new();
        for arg in std::env::args() {
            if arg == "--control" {
                dir = String::from(DEFAULT_DIR);
            } else if let Some(rest) = arg.strip_prefix("--control=") {
                dir = String::from(rest);
            }
        }
        if dir.is_empty() {
            return control;
        }

        if let Err(error) = std::fs::create_dir_all(&dir) {
            println!("[DebugControl] cannot create {}: {}", dir, error);
            return control;
        }

        let in_path = format!("{}/in.txt", dir);
        let out_path = format!("{}/out.log", dir);
        if let Err(error) = std::fs::write(&in_path, b"") {
            println!("[DebugControl] cannot create {}: {}", in_path, error);
            return control;
        }
        let out = match File::create(&out_path) {
            Ok(file) => file,
            Err(error) => {
                println!("[DebugControl] cannot create {}: {}", out_path, error);
                return control;
            }
        };

        control.is_enabled = true;
        control.in_path = in_path;
        control.out = Some(out);
        println!(
            "[DebugControl] enabled: {} <- commands, {} -> state",
            control.in_path, out_path
        );
        control
    }

    pub fn input(&mut self, unit_manager: &UnitManager) {
        if cfg!(debug_assertions) {
            self.input_undo = is_key_pressed(KeyCode::Z);
        }
        let turn = unit_manager.turn_count;
        if !self.is_enabled {
            return;
        }
        let Ok(metadata) = std::fs::metadata(&self.in_path) else {
            return;
        };
        let len = metadata.len();
        if len < self.in_cursor {
            self.in_cursor = 0;
            self.in_partial.clear();
        }
        if len == self.in_cursor {
            return;
        }

        let Ok(mut file) = File::open(&self.in_path) else {
            return;
        };
        if file.seek(SeekFrom::Start(self.in_cursor)).is_err() {
            return;
        }
        let mut buffer = Vec::new();
        let Ok(read) = file.read_to_end(&mut buffer) else {
            return;
        };
        self.in_cursor += read as u64;

        self.in_partial.push_str(&String::from_utf8_lossy(&buffer));

        while let Some(newline) = self.in_partial.find('\n') {
            let line: String = self.in_partial.drain(..=newline).collect();
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            self.pending.push(String::from(line));
        }

        if self.in_partial.len() > MAX_PARTIAL_BYTES {
            self.in_partial.clear();
            self.write_record(
                turn,
                "log lvl=warn msg=[DebugControl] dropped an oversized line with no newline",
            );
        }
    }

    pub fn next_seq(&mut self) -> u64 {
        self.cmd_seq += 1;
        self.cmd_seq
    }

    fn write_line(&mut self, line: &str) {
        let Some(file) = self.out.as_mut() else {
            return;
        };
        let clean = line.replace('\n', " ").replace('\r', " ");
        let _ = file.write_all(clean.as_bytes());
        let _ = file.write_all(b"\n");
        let _ = file.flush();
    }

    fn write_record(&mut self, turn: u32, body: &str) {
        let line = format!("e{} t{} {}", self.epoch, turn, body);
        self.write_line(&line);
    }

    pub fn write_ack(&mut self, turn: u32, seq: u64, error: Option<&str>, command: &str) {
        if !self.is_enabled {
            return;
        }
        let body = match error {
            Some(token) => format!("ack seq={} ok=0 err={} cmd={}", seq, token, command),
            None => format!("ack seq={} ok=1 cmd={}", seq, command),
        };
        self.write_record(turn, &body);
    }

    pub fn write_undo(&mut self, turn: u32, from: u32, left: usize) {
        if !self.is_enabled {
            return;
        }
        self.write_record(turn, &format!("undo from={} left={}", from, left));
    }

    pub fn write_hello(
        &mut self,
        turn: u32,
        period: f32,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
    ) {
        if !self.is_enabled || self.hello_sent {
            return;
        }
        self.hello_sent = true;
        let body = format!(
            "hello version={} period={:.2} units={} view={}",
            env!("CARGO_PKG_VERSION"),
            period,
            unit_manager.len(),
            format_sector(level_manager.view_sector_pos),
        );
        self.write_record(turn, &body);
    }

    pub fn write_logs(&mut self, unit_manager: &UnitManager, debug_log: &mut DebugLog) {
        let turn = unit_manager.turn_count;
        let entries = mem::take(&mut debug_log.pending_mirror);
        if !self.is_enabled {
            return;
        }
        for (level, message) in entries {
            let name = match level {
                LogLevel::Log => "log",
                LogLevel::Warning => "warn",
            };
            self.write_record(turn, &format!("log lvl={} msg={}", name, message));
        }
    }

    pub fn write_turn_block(
        &mut self,
        kind: &str,
        turn: u32,
        is_dump: bool,
        unit_manager: &UnitManager,
        selector: &SelectorManager,
        level_manager: &LevelManager,
    ) {
        if !self.is_enabled {
            return;
        }

        let mut dead = 0;
        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] {
                dead += 1;
            }
        }
        let selected = match selector.selected_unit_i {
            Some(unit_i) => unit_i.to_string(),
            None => String::from("-"),
        };
        let resources = &unit_manager.economy.resources;
        let mut end = "-";
        if unit_manager.is_game_won() {
            end = "won";
        } else if unit_manager.is_game_lost() {
            end = "lost";
        }
        let header = format!(
            "{} view={} sel={} units={} dead={} res={},{},{},{},{} tech={:b} end={}",
            kind,
            format_sector(level_manager.view_sector_pos),
            selected,
            unit_manager.len(),
            dead,
            resources.men,
            resources.iron,
            resources.wood,
            resources.mercury,
            resources.sulfur,
            unit_manager.economy.technology,
            end,
        );
        self.write_record(turn, &header);

        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] && !is_dump {
                continue;
            }
            let line = format_unit(unit_i, is_dump, unit_manager);
            self.write_record(turn, &line);
        }

        if is_dump {
            let mut sectors: Vec<IVec2> = level_manager.levels.keys().copied().collect();
            sectors.sort_by_key(ivec2_yx_sort_key);
            for sector in sectors {
                self.write_interactables(turn, level_manager, sector);
            }
        } else if let Some(view) = level_manager.view_sector_pos {
            self.write_interactables(turn, level_manager, view);
        }
    }

    pub fn undo(
        &mut self,
        persist_head_state: bool,
        unit_manager: &mut UnitManager,
        selector: &mut SelectorManager,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
    ) -> bool {
        let turn_abandoned = unit_manager.turn_count;
        let reached = history.step_back(unit_manager, level_manager);
        let Some(turn_reached) = reached else {
            if history.request_load_for_rewind() {
                debug_log.log(String::from(
                    "[History] undo: older history is on disk, loading before it can be applied - retry shortly",
                ));
            }
            return false;
        };

        unit_manager.rebuild_occupancy();
        unit_manager.turn_count = turn_reached;
        unit_manager.turn_timer = 0.0;

        let mut selection_valid = false;
        if let Some(unit_i) = selector.selected_unit_i {
            selection_valid = unit_i < unit_manager.len() && !unit_manager.is_dead[unit_i];
        }
        if !selection_valid {
            selector.selected_unit_i = unit_manager.find_first_player_unit();
        }

        if !self.is_undo_output_suppressed {
            self.epoch += 1;
            let rewinds_left = history.rewinds_left(unit_manager.turn_count);
            self.write_undo(unit_manager.turn_count, turn_abandoned, rewinds_left);
            self.write_turn_block(
                "turn",
                unit_manager.turn_count,
                false,
                unit_manager,
                selector,
                level_manager,
            );
        }

        if persist_head_state {
            history.write_head_state_from_world(
                unit_manager.turn_count,
                unit_manager,
                level_manager,
            );
        }

        true
    }

    fn parse_step_count(tokens: &[&str], implicit_one: bool, max: u32) -> Option<u32> {
        if tokens.len() == 1 {
            return if implicit_one { Some(1) } else { None };
        }
        if tokens.len() != 2 {
            return None;
        }
        let count = tokens[1].parse::<u32>().ok()?;
        if count == 0 || count > max {
            return None;
        }
        Some(count)
    }

    fn start_historycheck(
        &mut self,
        count: u32,
        turn_count: u32,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        history: &HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        let baseline = history.encode_head_state(turn_count, unit_manager, level_manager);
        self.historycheck_baseline = Some(baseline);
        self.historycheck_baseline_player_state = history.player_state.clone();
        self.historycheck_baseline_turn = turn_count;
        self.historycheck_turns_remaining = count;
        self.historycheck_total_turns = count;
        self.steps_pending += count;
        debug_log.log(format!("historycheck: advancing {} turns", count));
    }

    pub fn is_rewinding(&self) -> bool {
        self.is_historycheck_rewinding || self.rewinds_pending > 0
    }

    pub fn tick_rewinds(
        &mut self,
        loading_manager: &LoadingManager,
        unit_manager: &mut UnitManager,
        selector: &mut SelectorManager,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        if !loading_manager.is_initialized || self.steps_pending != 0 {
            return;
        }
        if self.is_historycheck_rewinding {
            self.tick_historycheck_rewind(
                unit_manager,
                selector,
                level_manager,
                history,
                debug_log,
            );
        } else if self.rewinds_pending > 0 {
            self.tick_rewind_command(unit_manager, selector, level_manager, history, debug_log);
        }
    }

    fn tick_historycheck_rewind(
        &mut self,
        unit_manager: &mut UnitManager,
        selector: &mut SelectorManager,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        if unit_manager.turn_count <= self.historycheck_baseline_turn {
            self.is_historycheck_rewinding = false;
            self.finish_historycheck_compare(
                unit_manager.turn_count,
                unit_manager,
                level_manager,
                history,
                debug_log,
            );
            return;
        }
        if !self.undo(
            false,
            unit_manager,
            selector,
            level_manager,
            history,
            debug_log,
        ) {
            if !history.pending_load_identity_present() {
                self.is_historycheck_rewinding = false;
                self.is_undo_output_suppressed = false;
                debug_log.warning(format!(
                    "historycheck {}: undo failed at turn {} before reaching baseline turn {}",
                    self.historycheck_total_turns,
                    unit_manager.turn_count,
                    self.historycheck_baseline_turn
                ));
                self.historycheck_baseline = None;
            }
        }
    }

    fn tick_rewind_command(
        &mut self,
        unit_manager: &mut UnitManager,
        selector: &mut SelectorManager,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        let before = unit_manager.turn_count;
        if !self.undo(
            false,
            unit_manager,
            selector,
            level_manager,
            history,
            debug_log,
        ) {
            if !history.pending_load_identity_present() {
                self.rewinds_pending = 0;
            }
        } else if unit_manager.turn_count != before {
            self.rewinds_pending -= 1;
        }
        if self.rewinds_pending == 0 {
            self.finalize_rewind_command(
                unit_manager.turn_count,
                unit_manager,
                level_manager,
                history,
                debug_log,
            );
        }
    }

    fn finalize_rewind_command(
        &mut self,
        turn_count: u32,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        history: &HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        let reached = turn_count;
        if reached != self.rewind_started_at {
            history.write_head_state_from_world(turn_count, unit_manager, level_manager);
        }
        debug_log.log(format!("rewind: reached turn {}", reached));
    }

    fn finish_historycheck_compare(
        &mut self,
        turn_count: u32,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        history: &HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        let total = self.historycheck_total_turns;
        self.is_undo_output_suppressed = false;
        let after = history.encode_head_state_with_player_state(
            turn_count,
            unit_manager,
            level_manager,
            &self.historycheck_baseline_player_state,
        );
        let baseline_turn = self.historycheck_baseline_turn;
        let baseline = self.historycheck_baseline.take().unwrap_or_default();
        if turn_count < baseline_turn {
            debug_log.warning(format!(
                "historycheck {}: INCONCLUSIVE - rewound to turn {}, past baseline turn {} (merged frames land on group boundaries)",
                total, turn_count, baseline_turn
            ));
        } else if baseline == after {
            debug_log.log(format!("historycheck {}: PASS", total));
        } else {
            let common_len = baseline.len().min(after.len());
            let mut first_diff = common_len;
            for i in 0..common_len {
                if baseline[i] != after[i] {
                    first_diff = i;
                    break;
                }
            }
            let location = HistoryManager::describe_head_state_offset(&baseline, first_diff);
            debug_log.warning(format!(
                "historycheck {}: FAIL at byte {} ({}) (baseline len={}, after len={})",
                total,
                first_diff,
                location,
                baseline.len(),
                after.len()
            ));
        }
        history.write_head_state_from_world(turn_count, unit_manager, level_manager);
    }

    pub fn post_turn(
        &mut self,
        unit_manager: &UnitManager,
        selector: &SelectorManager,
        level_manager: &LevelManager,
        history: &HistoryManager,
        debug_log: &mut DebugLog,
    ) {
        if !unit_manager.turn_tick_this_frame {
            return;
        }
        let turn_count = unit_manager.turn_count;
        self.write_turn_block(
            "turn",
            turn_count,
            false,
            unit_manager,
            selector,
            level_manager,
        );

        if self.historycheck_turns_remaining > 0 {
            self.historycheck_turns_remaining -= 1;
            if self.historycheck_turns_remaining == 0 {
                self.is_historycheck_rewinding = true;
                self.is_undo_output_suppressed = true;
            }
        } else if cfg!(debug_assertions)
            && history.is_enabled()
            && !self.is_paused
            && turn_count > 0
            && turn_count % HISTORY_AUTO_CHECK_INTERVAL_TURNS == 0
        {
            self.start_historycheck(
                HISTORY_AUTO_CHECK_TURNS,
                turn_count,
                unit_manager,
                level_manager,
                history,
                debug_log,
            );
        }
    }

    pub fn apply_commands(
        &mut self,
        unit_manager: &mut UnitManager,
        selector: &mut SelectorManager,
        level_manager: &mut LevelManager,
        player_manager: &mut PlayerManager,
        history: &mut HistoryManager,
        gpu_path: &mut GpuPathManager,
        debug_log: &mut DebugLog,
    ) {
        if self.input_undo {
            self.input_undo = false;
            if !player_manager.is_keyboard_captured() {
                self.undo(
                    false,
                    unit_manager,
                    selector,
                    level_manager,
                    history,
                    debug_log,
                );
            }
        }
        if !self.is_enabled {
            return;
        }
        let commands = mem::take(&mut self.pending);
        for command in commands {
            let seq = self.next_seq();
            let error = self.apply_control_command(
                &command,
                unit_manager,
                selector,
                level_manager,
                player_manager,
                history,
                gpu_path,
                debug_log,
            );
            self.write_ack(unit_manager.turn_count, seq, error, &command);
        }
    }

    fn apply_control_command(
        &mut self,
        command: &str,
        unit_manager: &mut UnitManager,
        selector: &mut SelectorManager,
        level_manager: &mut LevelManager,
        player_manager: &mut PlayerManager,
        history: &mut HistoryManager,
        gpu_path: &mut GpuPathManager,
        debug_log: &mut DebugLog,
    ) -> Option<&'static str> {
        let tokens: Vec<&str> = command.split_whitespace().collect();
        let Some(verb) = tokens.first() else {
            return Some("bad_verb");
        };

        match *verb {
            "echo" => None,

            "save" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                if !history.is_enabled() {
                    return Some("no_history");
                }
                history.save_game(unit_manager, level_manager, debug_log);
                None
            }

            "select" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let unit_i = match Self::parse_live_unit(unit_manager, tokens[1]) {
                    Ok(unit_i) => unit_i,
                    Err(token) => return Some(token),
                };
                selector.input_selection = Some(Some(unit_i));
                None
            }

            "deselect" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                selector.input_selection = Some(None);
                None
            }

            "move" => {
                if tokens.len() != 4 && tokens.len() != 5 {
                    return Some("bad_args");
                }
                let unit_i = match Self::parse_live_unit(unit_manager, tokens[1]) {
                    Ok(unit_i) => unit_i,
                    Err(token) => return Some(token),
                };
                let Some(level_pos) = parse_ivec2(tokens[2]) else {
                    return Some("bad_args");
                };
                let Some(tile) = parse_ivec2(tokens[3]) else {
                    return Some("bad_args");
                };
                let mut is_queued = false;
                if tokens.len() == 5 {
                    if tokens[4] != "queue" {
                        return Some("bad_args");
                    }
                    is_queued = true;
                }
                if level_manager.get_level_at_pos(level_pos).is_none() {
                    return Some("no_level");
                }
                if !UnitManager::is_order_target_valid(level_manager, level_pos, tile) {
                    return Some("out_of_bounds");
                }
                unit_manager.pending_target_orders.push(TargetOrder {
                    unit_i,
                    target: None,
                });
                unit_manager.pending_orders.push(MoveOrder {
                    unit_i,
                    target: GoalNode { level_pos, tile },
                    is_queued,
                });
                None
            }

            "target" => {
                if tokens.len() != 3 {
                    return Some("bad_args");
                }
                let unit_i = match Self::parse_live_unit(unit_manager, tokens[1]) {
                    Ok(unit_i) => unit_i,
                    Err(token) => return Some(token),
                };
                let target_i = match Self::parse_live_unit(unit_manager, tokens[2]) {
                    Ok(target_i) => target_i,
                    Err(token) => return Some(token),
                };
                unit_manager.pending_target_orders.push(TargetOrder {
                    unit_i,
                    target: Some(target_i),
                });
                None
            }

            "train" => {
                if tokens.len() != 4 {
                    return Some("bad_args");
                }
                let Ok(design_i) = tokens[1].parse::<usize>() else {
                    return Some("bad_args");
                };
                if design_i >= player_manager.data.designs.len() {
                    return Some("no_design");
                }
                let Some(level_pos) = parse_ivec2(tokens[2]) else {
                    return Some("bad_args");
                };
                let Some(tile) = parse_ivec2(tokens[3]) else {
                    return Some("bad_args");
                };
                if level_manager.get_level_at_pos(level_pos).is_none() {
                    return Some("no_level");
                }
                unit_manager.pending_trains.push(TrainOrder {
                    design: player_manager.data.designs[design_i],
                    level_pos,
                    tile,
                });
                None
            }

            "sell" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let unit_i = match Self::parse_live_unit(unit_manager, tokens[1]) {
                    Ok(unit_i) => unit_i,
                    Err(token) => return Some(token),
                };
                if !unit_manager.can_sell(unit_i) {
                    return Some("cannot_sell");
                }
                unit_manager.pending_sells.push(unit_i);
                None
            }

            "transfer" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Some(tile) = parse_ivec2(tokens[1]) else {
                    return Some("bad_args");
                };
                let Some(army_level) = unit_manager.army_level() else {
                    return Some("no_army");
                };
                if level_manager.gate_arrival(army_level, tile).is_none() {
                    return Some("no_gate");
                }
                unit_manager.pending_transfer = Some(tile);
                None
            }

            "design" => {
                let Some(kind_token) = tokens.get(1) else {
                    return Some("bad_args");
                };
                let Some(kind) = parse_variant(kind_token, &ALL_UNIT_KINDS, UnitKind::name) else {
                    return Some("bad_args");
                };
                let mut design = UnitDesign::default_for(kind);
                let mut is_parsed = false;
                if kind == UnitKind::Soldier && tokens.len() == 4 {
                    let weapon = parse_variant(tokens[2], &ALL_WEAPONS, Weapon::name);
                    let armor = parse_variant(tokens[3], &ALL_ARMORS, Armor::name);
                    if let (Some(weapon), Some(armor)) = (weapon, armor) {
                        design.weapon = weapon;
                        design.armor = armor;
                        is_parsed = true;
                    }
                }
                if kind == UnitKind::Machine && tokens.len() == 5 {
                    let weapon = parse_variant(tokens[2], &ALL_WEAPONS, Weapon::name);
                    let shielding = parse_variant(tokens[3], &ALL_SHIELDINGS, Shielding::name);
                    let chassis = parse_variant(tokens[4], &ALL_CHASSIS, Chassis::name);
                    if let (Some(weapon), Some(shielding), Some(chassis)) =
                        (weapon, shielding, chassis)
                    {
                        design.weapon = weapon;
                        design.shielding = shielding;
                        design.chassis = chassis;
                        is_parsed = true;
                    }
                }
                if !is_parsed {
                    return Some("bad_args");
                }
                if player_manager.add_design(design).is_none() {
                    return Some("full");
                }
                None
            }

            "undesign" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Ok(design_i) = tokens[1].parse::<usize>() else {
                    return Some("bad_args");
                };
                if design_i >= player_manager.data.designs.len() {
                    return Some("no_design");
                }
                player_manager.delete_design(design_i);
                None
            }

            "designs" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                let mut name = String::new();
                for design_i in 0..player_manager.data.designs.len() {
                    name.clear();
                    player_manager.data.designs[design_i].write_name(&mut name);
                    let line = format!("design i={} name={}", design_i, name);
                    self.write_record(unit_manager.turn_count, &line);
                }
                None
            }

            "economy" => {
                if tokens.len() != 6 {
                    return Some("bad_args");
                }
                if !cfg!(debug_assertions) {
                    return Some("debug_only");
                }
                let mut amounts = [0i32; 5];
                for amount_i in 0..amounts.len() {
                    let Ok(amount) = tokens[amount_i + 1].parse::<i32>() else {
                        return Some("bad_args");
                    };
                    if amount < 0 {
                        return Some("bad_args");
                    }
                    amounts[amount_i] = amount;
                }
                history.record_event_resources(&unit_manager.economy.resources);
                unit_manager.economy.resources =
                    Resources::new(amounts[0], amounts[1], amounts[2], amounts[3], amounts[4]);
                None
            }

            "unlock" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                if !cfg!(debug_assertions) {
                    return Some("debug_only");
                }
                let Some(technology) =
                    parse_variant(tokens[1], &ALL_TECHNOLOGIES, Technology::name)
                else {
                    return Some("bad_args");
                };
                history.record_event_technology(unit_manager.economy.technology);
                unit_manager.economy.technology |= technology_bit(technology);
                None
            }

            "snapshotsave" => {
                if tokens.len() < 2 {
                    return Some("bad_args");
                }
                if !history.is_enabled() {
                    return Some("no_history");
                }
                if !history.can_save() {
                    return Some("full");
                }
                let name = tokens[1..].join(" ");
                if name.chars().count() > SAVE_NAME_MAX {
                    return Some("bad_args");
                }
                history.input_save_snapshot = Some(name);
                None
            }

            "snapshotload" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Ok(snapshot_i) = tokens[1].parse::<usize>() else {
                    return Some("bad_args");
                };
                if snapshot_i >= history.snapshots.len() {
                    return Some("no_snapshot");
                }
                history.input_load_snapshot = Some(snapshot_i);
                None
            }

            "snapshots" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                for snapshot_i in 0..history.snapshots.len() {
                    let snapshot = &history.snapshots[snapshot_i];
                    let line = format!(
                        "snapshot i={} kind={:?} turn={} bytes={} name={}",
                        snapshot_i,
                        snapshot.kind,
                        snapshot.turn,
                        snapshot.picture.len(),
                        snapshot.name
                    );
                    self.write_record(unit_manager.turn_count, &line);
                }
                None
            }

            "build" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                if !cfg!(debug_assertions) {
                    return Some("debug_only");
                }
                let unit_i = match Self::parse_live_unit(unit_manager, tokens[1]) {
                    Ok(unit_i) => unit_i,
                    Err(token) => return Some(token),
                };
                selector.selected_unit_i = Some(unit_i);
                unit_manager.input_build = true;
                None
            }

            "undo" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                if !self.undo(
                    true,
                    unit_manager,
                    selector,
                    level_manager,
                    history,
                    debug_log,
                ) {
                    if history.pending_load_identity_present() {
                        return Some("loading");
                    }
                    return Some("no_history");
                }
                None
            }

            "view" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Some(sector) = parse_ivec2(tokens[1]) else {
                    return Some("bad_args");
                };
                if level_manager.get_level_at_pos(sector).is_none() {
                    return Some("no_level");
                }
                level_manager.view_sector_pos = Some(sector);
                None
            }

            "pause" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                self.is_paused = true;
                self.steps_pending = 0;
                if self.rewinds_pending > 0 {
                    self.rewinds_pending = 0;
                    self.finalize_rewind_command(
                        unit_manager.turn_count,
                        unit_manager,
                        level_manager,
                        history,
                        debug_log,
                    );
                }
                self.historycheck_turns_remaining = 0;
                self.historycheck_total_turns = 0;
                self.historycheck_baseline = None;
                if self.is_historycheck_rewinding {
                    self.is_historycheck_rewinding = false;
                    self.is_undo_output_suppressed = false;
                }
                None
            }

            "resume" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                self.is_paused = false;
                None
            }

            "step" => {
                let count = match Self::parse_step_count(&tokens, true, MAX_STEPS) {
                    Some(count) => count,
                    None => return Some("bad_args"),
                };
                self.steps_pending += count;
                None
            }

            "rewind" => {
                let count = match Self::parse_step_count(&tokens, true, MAX_STEPS) {
                    Some(count) => count,
                    None => return Some("bad_args"),
                };
                self.rewind_started_at = unit_manager.turn_count;
                self.rewinds_pending = count;
                None
            }

            "historystat" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                history.log_statistics(debug_log);
                None
            }

            "fastforward" => {
                let count = match Self::parse_step_count(&tokens, false, MAX_FORCED_TURNS) {
                    Some(count) => count,
                    None => return Some("bad_args"),
                };
                self.steps_pending += count;
                None
            }

            "historycheck" => {
                if !history.is_enabled() {
                    return Some("no_history");
                }
                let count = match Self::parse_step_count(&tokens, false, MAX_FORCED_TURNS) {
                    Some(count) => count,
                    None => return Some("bad_args"),
                };
                self.start_historycheck(
                    count,
                    unit_manager.turn_count,
                    unit_manager,
                    level_manager,
                    history,
                    debug_log,
                );
                None
            }

            "dump" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                self.write_turn_block(
                    "dump",
                    unit_manager.turn_count,
                    true,
                    unit_manager,
                    selector,
                    level_manager,
                );
                None
            }

            "gpufield" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                gpu_path.build_and_run(level_manager, unit_manager);
                gpu_path.log_stats(debug_log, unit_manager);
                None
            }

            "gpuunit" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Ok(unit_index) = tokens[1].parse::<usize>() else {
                    return Some("bad_args");
                };
                if unit_index >= unit_manager.len() {
                    return Some("bad_unit");
                }
                gpu_path.build_and_run(level_manager, unit_manager);
                gpu_path.log_unit_window(debug_log, unit_manager, unit_index);
                None
            }

            #[cfg(not(target_arch = "wasm32"))]
            "gpubench" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Ok(iterations) = tokens[1].parse::<usize>() else {
                    return Some("bad_args");
                };
                if iterations == 0 || iterations > 1000 {
                    return Some("bad_args");
                }

                gpu_path.build_and_run(level_manager, unit_manager);

                let mut walkability_total = Duration::ZERO;
                let mut occupancy_total = Duration::ZERO;
                let mut upload_total = Duration::ZERO;
                let mut sector_pass_total = Duration::ZERO;
                let mut tile_pass_total = Duration::ZERO;
                let mut readback_total = Duration::ZERO;

                for _ in 0..iterations {
                    let t0 = Instant::now();
                    gpu_path.rebuild_walkability(level_manager);
                    let t1 = Instant::now();
                    gpu_path.upload_walkability();
                    let t2 = Instant::now();
                    gpu_path.run_border_pass();
                    gpu_path.run_sector_pass();
                    gpu_path.run_next_hop_pass();
                    gpu_path.flush_passes();
                    let t3 = Instant::now();
                    gpu_path.build_unit_pictures(unit_manager);
                    let t4 = Instant::now();
                    gpu_path.upload_unit_pictures();
                    let t5 = Instant::now();
                    gpu_path.run_pass();
                    let t6 = Instant::now();
                    gpu_path.readback();
                    let t7 = Instant::now();

                    walkability_total += t1 - t0;
                    occupancy_total += t4 - t3;
                    upload_total += (t2 - t1) + (t5 - t4);
                    sector_pass_total += t3 - t2;
                    tile_pass_total += t6 - t5;
                    readback_total += t7 - t6;
                }

                let draw_readback_total = sector_pass_total + tile_pass_total + readback_total;
                let total =
                    walkability_total + occupancy_total + upload_total + draw_readback_total;
                let scale = 1000.0 / iterations as f64;

                debug_log.log(format!(
                    "gpubench: units={} iterations={} walkability={:.3}ms unit_pictures={:.3}ms upload={:.3}ms sector_pass={:.3}ms tile_pass={:.3}ms readback={:.3}ms draw+readback={:.3}ms total={:.3}ms budget=0.500ms",
                    unit_manager.len(),
                    iterations,
                    walkability_total.as_secs_f64() * scale,
                    occupancy_total.as_secs_f64() * scale,
                    upload_total.as_secs_f64() * scale,
                    sector_pass_total.as_secs_f64() * scale,
                    tile_pass_total.as_secs_f64() * scale,
                    readback_total.as_secs_f64() * scale,
                    draw_readback_total.as_secs_f64() * scale,
                    total.as_secs_f64() * scale,
                ));
                None
            }

            #[cfg(not(target_arch = "wasm32"))]
            "gpuprofile" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Ok(iterations) = tokens[1].parse::<usize>() else {
                    return Some("bad_args");
                };
                if iterations == 0 || iterations > 1000 {
                    return Some("bad_args");
                }

                gpu_path.build_and_run(level_manager, unit_manager);

                let mut baseline_total = Duration::ZERO;
                let mut border_total = Duration::ZERO;
                let mut sector_total = Duration::ZERO;
                let mut next_hop_total = Duration::ZERO;
                let mut tile_total = Duration::ZERO;
                let mut readback_total = Duration::ZERO;

                for _ in 0..iterations {
                    gpu_path.rebuild_walkability(level_manager);
                    gpu_path.upload_walkability();
                    gpu_path.build_unit_pictures(unit_manager);
                    gpu_path.upload_unit_pictures();

                    let t0 = Instant::now();
                    gpu_path.finish();
                    let t1 = Instant::now();
                    gpu_path.run_border_pass();
                    gpu_path.finish();
                    let t1b = Instant::now();
                    gpu_path.run_sector_pass();
                    gpu_path.finish();
                    let t2 = Instant::now();
                    gpu_path.run_next_hop_pass();
                    gpu_path.finish();
                    let t3 = Instant::now();
                    gpu_path.run_pass();
                    gpu_path.finish();
                    let t4 = Instant::now();
                    gpu_path.readback();
                    let t5 = Instant::now();

                    baseline_total += t1 - t0;
                    border_total += t1b - t1;
                    sector_total += t2 - t1b;
                    next_hop_total += t3 - t2;
                    tile_total += t4 - t3;
                    readback_total += t5 - t4;
                }

                let scale = 1000.0 / iterations as f64;
                debug_log.log(format!(
                    "gpuprofile: units={} iterations={} idle_finish={:.3}ms border={:.3}ms sector={:.3}ms next_hop={:.3}ms tile={:.3}ms readback_after_drain={:.3}ms",
                    unit_manager.len(),
                    iterations,
                    baseline_total.as_secs_f64() * scale,
                    border_total.as_secs_f64() * scale,
                    sector_total.as_secs_f64() * scale,
                    next_hop_total.as_secs_f64() * scale,
                    tile_total.as_secs_f64() * scale,
                    readback_total.as_secs_f64() * scale,
                ));
                None
            }

            "spawnmany" => {
                if tokens.len() != 2 {
                    return Some("bad_args");
                }
                let Ok(count) = tokens[1].parse::<usize>() else {
                    return Some("bad_args");
                };
                if count == 0 || count > 10000 {
                    return Some("bad_args");
                }
                let mut spawned = 0usize;
                'outer: for ty in 0..DEFAULT_LEVEL_TILES_Y {
                    for tx in 0..DEFAULT_LEVEL_TILES_X {
                        for sy in 0..FIELD_SECTORS_Y {
                            for sx in 0..FIELD_SECTORS_X {
                                if spawned >= count {
                                    break 'outer;
                                }
                                let level_pos = IVec2::new(sx, sy);
                                if level_manager.get_level_at_pos(level_pos).is_none() {
                                    continue;
                                }
                                let tile_pos = IVec2::new(tx, ty);
                                if !UnitManager::is_tile_walkable(
                                    level_manager,
                                    level_pos,
                                    tile_pos,
                                ) {
                                    continue;
                                }
                                if unit_manager.is_tile_occupied(level_pos, tile_pos) {
                                    continue;
                                }
                                let unit_i = unit_manager.add(
                                    UnitOwner::P1,
                                    UnitType::Soldier,
                                    level_pos,
                                    tile_pos,
                                    Direction::Down,
                                    UnitProfile::new(UnitDesign::default_for(UnitKind::Soldier)),
                                    MEN_PER_UNIT,
                                );
                                history.record_event_spawn(unit_i as u32);
                                unit_manager.occupancy.insert(level_pos, tile_pos, unit_i);
                                spawned += 1;
                            }
                        }
                    }
                }
                debug_log.log(format!("spawnmany: placed {}", spawned));
                None
            }

            #[cfg(not(target_arch = "wasm32"))]
            "moveall" => {
                if tokens.len() != 1 {
                    return Some("bad_args");
                }
                let mut ordered = 0usize;
                for unit_i in 0..unit_manager.len() {
                    if unit_manager.is_dead[unit_i] {
                        continue;
                    }
                    let from = unit_manager.level_pos[unit_i];
                    let target_level =
                        IVec2::new(FIELD_SECTORS_X - 1 - from.x, FIELD_SECTORS_Y - 1 - from.y);
                    if level_manager.get_level_at_pos(target_level).is_none() {
                        continue;
                    }
                    let tile = unit_manager.tile_position[unit_i];
                    if !UnitManager::is_order_target_valid(level_manager, target_level, tile) {
                        continue;
                    }
                    unit_manager.pending_orders.push(MoveOrder {
                        unit_i,
                        target: GoalNode {
                            level_pos: target_level,
                            tile,
                        },
                        is_queued: false,
                    });
                    ordered += 1;
                }
                debug_log.log(format!("moveall: ordered {}", ordered));
                None
            }

            _ => Some("bad_verb"),
        }
    }

    fn parse_live_unit(unit_manager: &UnitManager, token: &str) -> Result<usize, &'static str> {
        let Ok(unit_i) = token.parse::<usize>() else {
            return Err("bad_args");
        };
        if unit_i >= unit_manager.len() || unit_manager.is_dead[unit_i] {
            return Err("bad_unit");
        }
        Ok(unit_i)
    }

    fn write_interactables(&mut self, turn: u32, level_manager: &LevelManager, level_pos: IVec2) {
        let Some(level) = level_manager.get_level_at_pos(level_pos) else {
            return;
        };
        let mut tiles: Vec<IVec2> = level.interactables.keys().copied().collect();
        tiles.sort_by_key(ivec2_yx_sort_key);
        for tile in tiles {
            let Some(interactable) = level.interactables.get(&tile) else {
                continue;
            };
            let line = format!(
                "item level={},{} tile={},{} kind={}",
                level_pos.x,
                level_pos.y,
                tile.x,
                tile.y,
                interactable.name(),
            );
            self.write_record(turn, &line);
        }
    }
}

fn parse_variant<T: Copy>(token: &str, variants: &[T], name: fn(&T) -> &'static str) -> Option<T> {
    for variant in variants {
        if name(variant) == token {
            return Some(*variant);
        }
    }
    None
}

fn ivec2_yx_sort_key(v: &IVec2) -> (i32, i32) {
    (v.y, v.x)
}

fn format_sector(sector: Option<IVec2>) -> String {
    match sector {
        Some(pos) => format!("{},{}", pos.x, pos.y),
        None => String::from("-"),
    }
}

fn format_unit(unit_i: usize, is_dump: bool, unit_manager: &UnitManager) -> String {
    let mut line = format!(
        "unit={} owner={} type={} level={},{} tile={},{} dir={}",
        unit_i,
        unit_manager.owner[unit_i].name(),
        unit_manager.unit_type[unit_i].name(),
        unit_manager.level_pos[unit_i].x,
        unit_manager.level_pos[unit_i].y,
        unit_manager.tile_position[unit_i].x,
        unit_manager.tile_position[unit_i].y,
        unit_manager.direction[unit_i].name(),
    );
    if unit_manager.unit_type[unit_i].is_structure() {
        let _ = write!(
            line,
            " sprite={}",
            unit_manager.profile[unit_i].structure_sprite.name()
        );
    }

    let crossed_sector = unit_manager.prev_level_pos[unit_i] != unit_manager.level_pos[unit_i];
    if unit_manager.prev_tile_position[unit_i] != unit_manager.tile_position[unit_i]
        || crossed_sector
    {
        if crossed_sector {
            let _ = write!(
                line,
                " state=move from={},{}:{},{} to={},{}:{},{}",
                unit_manager.prev_level_pos[unit_i].x,
                unit_manager.prev_level_pos[unit_i].y,
                unit_manager.prev_tile_position[unit_i].x,
                unit_manager.prev_tile_position[unit_i].y,
                unit_manager.level_pos[unit_i].x,
                unit_manager.level_pos[unit_i].y,
                unit_manager.tile_position[unit_i].x,
                unit_manager.tile_position[unit_i].y,
            );
        } else {
            let _ = write!(
                line,
                " state=move from={},{} to={},{}",
                unit_manager.prev_tile_position[unit_i].x,
                unit_manager.prev_tile_position[unit_i].y,
                unit_manager.tile_position[unit_i].x,
                unit_manager.tile_position[unit_i].y,
            );
        }
    } else {
        line.push_str(" state=idle");
    }

    let _ = write!(
        line,
        " hp={}/{} men={}",
        unit_manager.hp[unit_i],
        unit_manager.max_hp(unit_i),
        unit_manager.men(unit_i)
    );
    if let Some(target) = unit_manager.target[unit_i] {
        let _ = write!(line, " target={}", target);
    }
    let goals = unit_manager.goal_queue[unit_i].len();
    if goals > 0 {
        let _ = write!(line, " goals={}", goals);
    }
    if is_dump && unit_manager.is_dead[unit_i] {
        line.push_str(" dead=1");
    }

    line
}

pub fn parse_ivec2(token: &str) -> Option<IVec2> {
    let mut parts = token.split(',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(IVec2::new(x, y))
}
