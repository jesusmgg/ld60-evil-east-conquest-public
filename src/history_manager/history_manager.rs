use crate::constants::HISTORY_TIER_RATIO;
#[cfg(not(target_arch = "wasm32"))]
use crate::constants::SAVE_DIR_NAME;
use crate::debug::DebugLog;
use crate::enums::Direction;
use crate::history_manager::recorder::{
    decode_head_state, describe_head_state_offset, encode_head_state, HistoryRecorder,
};
use crate::history_manager::service::HistoryStorageService;
use crate::history_manager::snapshots::{encode_snapshots, Snapshot, SnapshotKind};
use crate::level::interactable::Interactable;
use crate::level::level_manager::LevelManager;
use crate::unit_design::Resources;
use crate::unit_manager::{GoalNode, UnitManager};
use macroquad::prelude::*;
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::io::ErrorKind;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::process::{self, Command};

#[cfg(target_arch = "wasm32")]
const SNAPSHOT_BYTES_FLOOR: usize = 4096;

pub struct HistoryManager {
    pub(super) recorder: HistoryRecorder,
    pub(super) storage: HistoryStorageService,

    pub(super) is_history_disabled: bool,

    pub(super) input_save_game: bool,

    pub(super) is_chunk_sealed: bool,

    pub player_state: Vec<u8>,

    pub snapshots: Vec<Snapshot>,
    pub input_save_snapshot: Option<String>,
    pub input_load_snapshot: Option<usize>,
    pub input_delete_snapshot: Option<usize>,
    pub input_reset_game: bool,
    pub is_snapshot_loaded: bool,
    pub is_world_rewound: bool,
    pub is_save_cleared: bool,

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) is_level_bundle_checked: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) existing_save: Option<(PathBuf, Vec<u8>)>,

    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    pub(super) persisted_sector_count: usize,

    #[cfg(target_arch = "wasm32")]
    pub(super) is_web_probe_started: bool,
    #[cfg(target_arch = "wasm32")]
    pub(super) is_web_persistence_logged: bool,
    #[cfg(target_arch = "wasm32")]
    pub(super) web_boot_blob: Option<Vec<u8>>,
}

impl HistoryManager {
    pub(crate) fn parse_no_history_flag() -> bool {
        if cfg!(target_arch = "wasm32") {
            return false;
        }
        for arg in env::args() {
            if arg == "--no-history" {
                return true;
            }
        }
        false
    }

    pub fn new_disabled(is_history_disabled: bool) -> Self {
        Self {
            recorder: HistoryRecorder::new_disabled(),
            storage: HistoryStorageService::new(),

            is_history_disabled,

            input_save_game: false,
            is_chunk_sealed: false,

            player_state: Vec::new(),

            snapshots: Vec::new(),
            input_save_snapshot: None,
            input_load_snapshot: None,
            input_delete_snapshot: None,
            input_reset_game: false,
            is_snapshot_loaded: false,
            is_world_rewound: false,
            is_save_cleared: false,

            #[cfg(not(target_arch = "wasm32"))]
            is_level_bundle_checked: false,
            #[cfg(not(target_arch = "wasm32"))]
            existing_save: None,

            #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
            persisted_sector_count: 0,

            #[cfg(target_arch = "wasm32")]
            is_web_probe_started: false,
            #[cfg(target_arch = "wasm32")]
            is_web_persistence_logged: false,
            #[cfg(target_arch = "wasm32")]
            web_boot_blob: None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.recorder.is_enabled
    }

    pub fn input(&mut self) {
        self.input_save_game = is_key_pressed(KeyCode::F10);
    }

    pub fn begin_turn(&mut self, turn: u32) {
        self.recorder.begin_turn(turn);
    }

    pub fn end_turn(&mut self) {
        self.is_chunk_sealed = self.recorder.end_turn(&self.storage);
    }

    pub fn record_move(
        &mut self,
        unit_index: u32,
        level_pos: IVec2,
        tile_position: IVec2,
        direction: Direction,
    ) {
        self.recorder
            .record_move(unit_index, level_pos, tile_position, direction);
    }

    pub fn record_event_spawn(&mut self, unit_index: u32) {
        self.recorder.record_event_spawn(unit_index);
    }

    pub fn record_event_goal_push(&mut self, unit_index: u32) {
        self.recorder.record_event_goal_push(unit_index);
    }

    pub fn record_event_goal_pop_front(&mut self, unit_index: u32, level_pos: IVec2, tile: IVec2) {
        self.recorder
            .record_event_goal_pop_front(unit_index, level_pos, tile);
    }

    pub fn record_event_goal_clear(&mut self, unit_index: u32, old_queue: &[GoalNode]) {
        self.recorder.record_event_goal_clear(unit_index, old_queue);
    }

    pub fn record_event_unreachable_streak(&mut self, unit_index: u32, old_streak: u8) {
        self.recorder
            .record_event_unreachable_streak(unit_index, old_streak);
    }

    pub fn record_event_death(&mut self, unit_index: u32) {
        self.recorder.record_event_death(unit_index);
    }

    pub fn record_event_hp(&mut self, unit_index: u32, old_hp: u16) {
        self.recorder.record_event_hp(unit_index, old_hp);
    }

    pub fn record_event_kills(&mut self, unit_index: u32, old_kills: u16) {
        self.recorder.record_event_kills(unit_index, old_kills);
    }

    pub fn record_event_target(&mut self, unit_index: u32, old_target: Option<usize>) {
        self.recorder.record_event_target(unit_index, old_target);
    }

    pub fn record_event_resources(&mut self, old_resources: &Resources) {
        self.recorder.record_event_resources(old_resources);
    }

    pub fn record_event_technology(&mut self, old_technology: u32) {
        self.recorder.record_event_technology(old_technology);
    }

    pub fn record_event_interactable(
        &mut self,
        level_pos: IVec2,
        tile: IVec2,
        old_interactable: Option<&Interactable>,
    ) {
        self.recorder
            .record_event_interactable(level_pos, tile, old_interactable);
    }

    pub fn register_sector(&mut self, level_pos: IVec2) {
        self.recorder.register_sector(level_pos);
    }

    pub fn step_back(
        &mut self,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
    ) -> Option<u32> {
        let turn_reached = self.recorder.step_back(unit_manager, level_manager);
        if turn_reached.is_some() {
            self.is_world_rewound = true;
        }
        turn_reached
    }

    pub fn request_load_for_rewind(&mut self) -> bool {
        self.recorder.request_load_for_rewind(&self.storage)
    }

    pub fn pending_load_identity_present(&self) -> bool {
        self.recorder.pending_load_identity.is_some()
    }

    pub fn rewinds_left(&self, current_turn: u32) -> usize {
        let mut oldest_resident = current_turn;
        for tier in &self.recorder.tiers {
            if !tier.frames.is_empty() && tier.turn_oldest < oldest_resident {
                oldest_resident = tier.turn_oldest;
            }
        }
        let mut rewinds_left = self.recorder.resident_frame_count();
        for file_ref in &self.storage.files {
            if file_ref.turn_last >= oldest_resident
                || file_ref.turn_last < self.recorder.rewind_floor
            {
                continue;
            }
            let ratio = HISTORY_TIER_RATIO[file_ref.tier as usize];
            rewinds_left +=
                (file_ref.turn_last.saturating_sub(file_ref.turn_first) / ratio + 1) as usize;
        }
        rewinds_left
    }

    pub fn encode_head_state(
        &self,
        turn: u32,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
    ) -> Vec<u8> {
        self.encode_head_state_with_player_state(
            turn,
            unit_manager,
            level_manager,
            &self.player_state,
        )
    }

    pub fn encode_head_state_with_player_state(
        &self,
        turn: u32,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        player_state: &[u8],
    ) -> Vec<u8> {
        encode_head_state(
            turn,
            unit_manager,
            &self.recorder.sector_lookup,
            &level_manager.levels,
            player_state,
            self.recorder.rewind_floor,
        )
    }

    pub fn write_head_state_from_world(
        &self,
        turn: u32,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
    ) {
        if !self.recorder.is_enabled {
            return;
        }
        let bytes = self.encode_head_state(turn, unit_manager, level_manager);
        self.storage.write_head_state(bytes);
    }

    pub fn save_game(
        &mut self,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        debug_log: &mut DebugLog,
    ) {
        if !self.recorder.is_enabled {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.recorder.flush_all_chunks(&self.storage);
        self.write_head_state_from_world(unit_manager.turn_count, unit_manager, level_manager);
        debug_log.log(format!(
            "[History] Manual save at turn {}",
            unit_manager.turn_count
        ));
    }

    pub fn update_snapshots(
        &mut self,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
        debug_log: &mut DebugLog,
    ) {
        self.is_snapshot_loaded = false;
        let Some(snapshot_i) = self.input_load_snapshot else {
            return;
        };
        if !unit_manager.is_initialized || self.recorder.pending_load_identity.is_some() {
            return;
        }
        self.input_load_snapshot = None;
        if !self.recorder.is_enabled || snapshot_i >= self.snapshots.len() {
            return;
        }
        let Some(state) = decode_head_state(&self.snapshots[snapshot_i].picture) else {
            debug_log.warning(format!(
                "[History] snapshot \"{}\" does not decode",
                self.snapshots[snapshot_i].name
            ));
            return;
        };
        self.recorder
            .apply_head_state(&state, unit_manager, level_manager);
        self.recorder.start_after_load(unit_manager.turn_count);
        self.player_state = state.player_state;
        unit_manager.clear_pending_orders();
        self.write_head_state_from_world(unit_manager.turn_count, unit_manager, level_manager);
        self.is_snapshot_loaded = true;
        self.is_world_rewound = true;
        debug_log.log(format!(
            "[History] Loaded \"{}\" at turn {}",
            self.snapshots[snapshot_i].name, unit_manager.turn_count
        ));
    }

    pub fn find_snapshot(&self, kind: SnapshotKind) -> Option<usize> {
        for snapshot_i in 0..self.snapshots.len() {
            if self.snapshots[snapshot_i].kind == kind {
                return Some(snapshot_i);
            }
        }
        None
    }

    pub fn find_checkpoint(&self) -> Option<usize> {
        let snapshot_i = self.find_snapshot(SnapshotKind::Checkpoint);
        if snapshot_i.is_some() {
            return snapshot_i;
        }
        self.find_snapshot(SnapshotKind::NewGame)
    }

    pub fn save_snapshot(
        &mut self,
        kind: SnapshotKind,
        name: String,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
    ) {
        let snapshot = Snapshot {
            kind,
            name,
            turn: unit_manager.turn_count,
            picture: self.encode_head_state(unit_manager.turn_count, unit_manager, level_manager),
        };
        let mut replace_i = None;
        if kind != SnapshotKind::Manual {
            for snapshot_i in 0..self.snapshots.len() {
                if self.snapshots[snapshot_i].kind == kind {
                    replace_i = Some(snapshot_i);
                    break;
                }
            }
        }
        match replace_i {
            Some(snapshot_i) => self.snapshots[snapshot_i] = snapshot,
            None => self.snapshots.push(snapshot),
        }
        self.storage
            .write_snapshots(encode_snapshots(&self.snapshots));
    }

    pub fn delete_snapshot(&mut self, snapshot_i: usize) {
        if snapshot_i >= self.snapshots.len() {
            return;
        }
        self.snapshots.remove(snapshot_i);
        self.storage
            .write_snapshots(encode_snapshots(&self.snapshots));
    }

    pub fn saves_left(&self) -> Option<usize> {
        #[cfg(target_arch = "wasm32")]
        {
            let free = self.storage.web_storage_free();
            if free < 0.0 {
                return None;
            }
            let mut picture_bytes = SNAPSHOT_BYTES_FLOOR;
            for snapshot in &self.snapshots {
                picture_bytes = picture_bytes.max(snapshot.picture.len());
            }
            Some((free / (2 * picture_bytes) as f64) as usize)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None
        }
    }

    pub fn can_save(&self) -> bool {
        if !self.recorder.is_enabled {
            return false;
        }
        match self.saves_left() {
            Some(count) => count > 0,
            None => true,
        }
    }

    pub fn describe_head_state_offset(bytes: &[u8], target: usize) -> String {
        describe_head_state_offset(bytes, target)
    }

    pub fn log_statistics(&self, debug_log: &mut DebugLog) {
        let mut total_frames = 0;
        for tier_index in 0..self.recorder.tiers.len() {
            let tier = &self.recorder.tiers[tier_index];
            let count = tier.frames.len();
            total_frames += count;
            let turn_range = if count == 0 {
                String::from("-")
            } else {
                format!("{}-{}", tier.turn_oldest, tier.turn_newest)
            };
            debug_log.log(format!(
                "history: tier{} frames={} turn_range={} sealed_chunks={}",
                tier_index, count, turn_range, tier.sealed_chunk_count
            ));
        }
        let mut total_file_bytes: u64 = 0;
        for file_ref in &self.storage.files {
            total_file_bytes += file_ref.byte_length as u64;
        }
        debug_log.log(format!(
            "history: total_frames={} head_turn={} files={} file_bytes={}",
            total_frames,
            self.recorder.head_turn,
            self.storage.files.len(),
            total_file_bytes
        ));
    }

    #[cfg(target_arch = "wasm32")]
    pub fn reset_game(&mut self) {
        self.storage.reset_web_game();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn reset_game(&mut self) {
        let mut args = env::args();
        let Some(program) = args.next() else {
            panic!("[History] reset: the game was started without a program path");
        };
        self.storage.shutdown();
        if let Err(error) = fs::remove_dir_all(SAVE_DIR_NAME) {
            if error.kind() != ErrorKind::NotFound {
                panic!("[History] reset: can't delete {}: {}", SAVE_DIR_NAME, error);
            }
        }
        if let Err(error) = Command::new(program).args(args).spawn() {
            panic!("[History] reset: can't relaunch the game: {}", error);
        }
        process::exit(0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn shutdown(&mut self, unit_manager: &UnitManager, level_manager: &LevelManager) {
        if !self.recorder.is_enabled {
            return;
        }
        self.recorder.flush_all_chunks(&self.storage);
        let head_bytes =
            self.encode_head_state(unit_manager.turn_count, unit_manager, level_manager);
        self.storage.write_head_state(head_bytes);
        self.storage.shutdown();
    }
}
