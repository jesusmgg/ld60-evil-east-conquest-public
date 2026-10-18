use super::HistoryManager;
use crate::constants::SAVE_DIR_NAME;
use crate::debug::DebugLog;
use crate::history_manager::recorder::{decode_head_state, HistoryRecorder};
use crate::history_manager::service::{parse_manifest_bytes, HistoryFileRef};
use crate::history_manager::snapshots::{SnapshotKind, CHECKPOINT_NAME, NEW_GAME_NAME};
use crate::level::level_manager::LevelManager;
use crate::unit_manager::UnitManager;
use macroquad::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::history_manager::service::{
    build_bundle_container, build_manifest_bytes, build_sector_table_bytes, decode_bundled_level,
    decode_bundled_level_set, level_set_file_name, parse_boot_container, parse_bundle_container,
    parse_sector_table_bytes, BootProbe,
};
#[cfg(target_arch = "wasm32")]
use crate::history_manager::snapshots::decode_snapshots;
#[cfg(target_arch = "wasm32")]
use macroquad::miniquad;
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use crate::constants::HISTORY_REFRESH_BUNDLE_FROM_ASSETS;
#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
use crate::history_manager::provider::desktop::read_manifest;
#[cfg(not(target_arch = "wasm32"))]
use crate::history_manager::provider::desktop::{
    create_save_directory, finalize_save_directory, read_head_state, read_level_bundle,
    read_manifest_bytes, read_sector_table, read_snapshots, refresh_bundle_from_assets,
    scan_history_files, write_level_bundle, write_manifest, write_sector_table,
};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::Ordering;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

pub const NO_LEVEL_BUNDLE_WARNING: &str =
    "[History] Save has no readable level bundle, falling back to assets/ - sector indices may not match a later save";

#[cfg(target_arch = "wasm32")]
pub const OPFS_CORRUPT_ERROR: &str = "[History] OPFS error — save is corrupt";

impl HistoryManager {
    pub fn pre_update(&mut self, level_manager: &mut LevelManager, debug_log: &mut DebugLog) {
        #[cfg(not(target_arch = "wasm32"))]
        if !self.is_level_bundle_checked && level_manager.is_initialized {
            self.is_level_bundle_checked = true;
            self.existing_save = if self.is_history_disabled {
                None
            } else {
                self.find_existing_save()
            };
            if let Some((save_dir, _manifest_bytes)) = &self.existing_save {
                match read_level_bundle(save_dir, debug_log) {
                    Some((mut bundle_levels, mut bundle_level_set)) => {
                        let mut spawn_drifted = false;
                        if HISTORY_REFRESH_BUNDLE_FROM_ASSETS {
                            spawn_drifted = refresh_bundle_from_assets(
                                &mut bundle_levels,
                                &mut bundle_level_set,
                                debug_log,
                            );
                        }
                        if spawn_drifted {
                            let save_dir = match self.existing_save.take() {
                                Some((dir, _)) => dir,
                                None => unreachable!(),
                            };
                            debug_log.warning(format!(
                                "[History] Unit spawns changed under save {} - starting a fresh timeline",
                                save_dir.display()
                            ));
                            for (pos, level) in bundle_levels {
                                level_manager.levels.entry(pos).or_insert(level);
                            }
                        } else {
                            level_manager.levels = bundle_levels;
                            if bundle_level_set.is_some() {
                                level_manager.level_set = bundle_level_set;
                            }
                        }
                    }
                    None => debug_log.warning(String::from(NO_LEVEL_BUNDLE_WARNING)),
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        if !self.is_web_probe_started && level_manager.is_initialized {
            self.is_web_probe_started = true;
            if !self.is_history_disabled {
                self.storage.init_web_probe();
            }
        }
    }

    pub fn post_update(
        &mut self,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        debug_log: &mut DebugLog,
    ) {
        if self.input_reset_game {
            self.input_reset_game = false;
            self.reset_game();
            return;
        }
        self.recorder.head_turn = unit_manager.turn_count;
        let chunk_sealed = self.is_chunk_sealed;
        self.is_chunk_sealed = false;

        let manual_save = self.input_save_game;
        if manual_save {
            self.input_save_game = false;
            self.save_game(unit_manager, level_manager, debug_log);
        }

        if chunk_sealed && !manual_save {
            let head_bytes =
                self.encode_head_state(unit_manager.turn_count, unit_manager, level_manager);
            self.storage.write_head_state(head_bytes);
        }

        let save_request = self.input_save_snapshot.take();
        let delete_request = self.input_delete_snapshot.take();
        if self.recorder.is_enabled {
            if let Some(snapshot_i) = delete_request {
                self.delete_snapshot(snapshot_i);
            }
            if unit_manager.transfer_arrival.is_some() {
                self.save_snapshot(
                    SnapshotKind::Checkpoint,
                    String::from(CHECKPOINT_NAME),
                    unit_manager,
                    level_manager,
                );
            }
            if let Some(name) = save_request {
                if self.can_save() {
                    debug_log.log(format!(
                        "[History] Saved \"{}\" at turn {}",
                        name, unit_manager.turn_count
                    ));
                    self.save_snapshot(SnapshotKind::Manual, name, unit_manager, level_manager);
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if self
                .storage
                .provider_desktop
                .has_error
                .load(Ordering::Acquire)
            {
                let reason = "[History] Worker IO error — save is corrupt";
                debug_log.warning(String::from(reason));
                panic!("{}", reason);
            }
        }

        for closed in self.storage.poll_closed_files() {
            self.storage.files.push(closed);
        }
        if let Some(loaded) = self.storage.poll_loaded() {
            let tier = loaded.tier;
            let sequence = loaded.sequence;
            let was_rewind = self.recorder.pending_load_is_rewind;
            let installed = self.recorder.install_loaded_file(loaded, &self.storage);
            if was_rewind || !installed {
                self.storage.delete_file(tier, sequence);
                self.storage.remove_file_ref(tier, sequence);
            }
        }
        self.recorder.check_prefetch(&self.storage);

        for file_index in self.recorder.files_fully_promoted(&self.storage) {
            let file_ref = &self.storage.files[file_index];
            let tier = file_ref.tier;
            let sequence = file_ref.sequence;
            self.storage.delete_file(tier, sequence);
            self.storage.files.remove(file_index);
        }

        #[cfg(target_arch = "wasm32")]
        if self.recorder.is_enabled {
            let ready = self.storage.web_boot_state();
            if ready == -1 || self.storage.provider_web.has_error {
                debug_log.warning(String::from(OPFS_CORRUPT_ERROR));
                panic!("{}", OPFS_CORRUPT_ERROR);
            }
            let persistent = self.storage.web_persistent();
            if persistent != -1 && !self.is_web_persistence_logged {
                self.is_web_persistence_logged = true;
                debug_log.log(format!("[History] OPFS persist() answer: {}", persistent));
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn check_web_boot(&mut self, debug_log: &mut DebugLog) {
        if !self.is_web_probe_started || self.is_history_disabled || self.web_boot_blob.is_some() {
            return;
        }
        let reason = match self.storage.web_boot_state() {
            0 => return,
            -2 => {
                debug_log.warning(String::from(OPFS_CORRUPT_ERROR));
                panic!("{}", OPFS_CORRUPT_ERROR);
            }
            -1 => OPFS_CORRUPT_ERROR,
            _ => match self.storage.take_web_boot_blob() {
                Some(blob) => match validate_web_boot_blob(&blob) {
                    Some(reason) => reason,
                    None => {
                        self.web_boot_blob = Some(blob);
                        return;
                    }
                },
                None => "[History] OPFS boot probe reported ready with no blob",
            },
        };
        debug_log.warning(format!("{} - deleting the save", reason));
        self.is_save_cleared = true;
        self.storage.clear_web_failed_save();
    }

    pub fn is_boot_ready(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            self.is_history_disabled || self.web_boot_blob.is_some()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            true
        }
    }

    pub fn finish_initialization(
        &mut self,
        level_manager: &mut LevelManager,
        unit_manager: &mut UnitManager,
        debug_log: &mut DebugLog,
    ) {
        #[cfg(target_arch = "wasm32")]
        let web_probe = self.take_web_boot_probe(level_manager, debug_log);

        self.recorder = HistoryRecorder::new(&level_manager.levels);
        self.recorder.is_enabled = !self.is_history_disabled;
        let mut is_save_resumed = false;
        #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
        {
            self.persisted_sector_count = self.recorder.sector_table.len();
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.recorder.is_enabled {
            match self.existing_save.take() {
                Some((save_dir, manifest_bytes)) => {
                    if let Some(table) = read_sector_table(&save_dir) {
                        #[cfg(debug_assertions)]
                        {
                            self.persisted_sector_count = table.len();
                        }
                        self.recorder.restore_sector_table(table);
                    }
                    let files = scan_history_files(&save_dir);
                    let head_restored = self.restore_from_save(
                        &save_dir,
                        &manifest_bytes,
                        &files,
                        unit_manager,
                        level_manager,
                        debug_log,
                    );
                    self.snapshots = read_snapshots(&save_dir);
                    self.storage.init(debug_log, save_dir, files);
                    is_save_resumed = true;
                    if !head_restored {
                        self.save_snapshot(
                            SnapshotKind::NewGame,
                            String::from(NEW_GAME_NAME),
                            unit_manager,
                            level_manager,
                        );
                    }
                }
                None => {
                    let save_id = match SystemTime::now().duration_since(UNIX_EPOCH) {
                        Ok(d) => d.as_millis(),
                        Err(_) => 0,
                    };
                    let temp_dir = match create_save_directory(Path::new(SAVE_DIR_NAME), save_id) {
                        Some(dir) => dir,
                        None => panic!(
                            "[History] Could not create save directory for save {}",
                            save_id
                        ),
                    };
                    if !write_manifest(&temp_dir, save_id, self.recorder.sector_table.len() as u32)
                    {
                        panic!("[History] Could not write manifest for save {}", save_id);
                    }
                    if !write_level_bundle(
                        &temp_dir,
                        &level_manager.levels,
                        &level_manager.level_set,
                    ) {
                        panic!(
                            "[History] Could not write level bundle for save {}",
                            save_id
                        );
                    }
                    if !write_sector_table(&temp_dir, &self.recorder.sector_table) {
                        panic!(
                            "[History] Could not write sector table for save {}",
                            save_id
                        );
                    }
                    let save_dir = match finalize_save_directory(Path::new(SAVE_DIR_NAME), save_id)
                    {
                        Some(dir) => dir,
                        None => panic!(
                            "[History] Could not finalize save directory for save {}",
                            save_id
                        ),
                    };
                    self.storage.init(debug_log, save_dir, Vec::new());
                    self.save_snapshot(
                        SnapshotKind::NewGame,
                        String::from(NEW_GAME_NAME),
                        unit_manager,
                        level_manager,
                    );
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        if self.recorder.is_enabled {
            let web_probe = match web_probe {
                Some(probe) => probe,
                None => {
                    let reason = "[History] OPFS boot probe had no blob at init despite history being enabled";
                    debug_log.warning(String::from(reason));
                    panic!("{}", reason);
                }
            };
            if web_probe.has_save {
                if !web_probe.sectors_bytes.is_empty() {
                    match parse_sector_table_bytes(&web_probe.sectors_bytes) {
                        Some(table) => self.recorder.restore_sector_table(table),
                        None => {
                            let reason = "[History] save has no readable sector table";
                            debug_log.warning(String::from(reason));
                            panic!("{}", reason);
                        }
                    }
                }
                let head = if web_probe.head_bytes.is_empty() {
                    None
                } else {
                    Some(web_probe.head_bytes)
                };
                let head_prev = if web_probe.head_prev_bytes.is_empty() {
                    None
                } else {
                    Some(web_probe.head_prev_bytes)
                };
                let bundle_missing = web_probe.bundle_bytes.is_empty();
                let (primary_head_decoded, head_restored) = self.restore_from_head_bytes(
                    &web_probe.manifest_bytes,
                    head,
                    head_prev,
                    &web_probe.files,
                    unit_manager,
                    level_manager,
                    debug_log,
                );
                if primary_head_decoded {
                    self.storage.mark_web_inherited_head_good();
                }
                self.snapshots = match decode_snapshots(&web_probe.snapshots_bytes) {
                    Some(snapshots) => snapshots,
                    None => match decode_snapshots(&web_probe.snapshots_prev_bytes) {
                        Some(snapshots) => snapshots,
                        None => Vec::new(),
                    },
                };
                self.storage.commit_web_existing(web_probe.files);
                is_save_resumed = true;
                if !head_restored {
                    let head = self.encode_head_state(
                        unit_manager.turn_count,
                        unit_manager,
                        level_manager,
                    );
                    self.storage.write_head_state(head);
                    self.save_snapshot(
                        SnapshotKind::NewGame,
                        String::from(NEW_GAME_NAME),
                        unit_manager,
                        level_manager,
                    );
                }
                if bundle_missing {
                    let bundle =
                        build_bundle_container(&level_manager.levels, &level_manager.level_set);
                    self.storage.write_web_bundle(&bundle);
                }
            } else {
                let save_id = (miniquad::date::now() * 1000.0) as u128;
                let manifest =
                    build_manifest_bytes(save_id, self.recorder.sector_table.len() as u32);
                let sectors = build_sector_table_bytes(&self.recorder.sector_table);
                let bundle =
                    build_bundle_container(&level_manager.levels, &level_manager.level_set);
                let head =
                    self.encode_head_state(unit_manager.turn_count, unit_manager, level_manager);
                self.storage
                    .commit_web_fresh(&manifest, &sectors, &bundle, &head);
                self.save_snapshot(
                    SnapshotKind::NewGame,
                    String::from(NEW_GAME_NAME),
                    unit_manager,
                    level_manager,
                );
            }
        }

        if is_save_resumed {
            self.input_load_snapshot = self.find_checkpoint();
        }
    }

    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    pub fn persist_sector_growth(
        &mut self,
        level_manager: &LevelManager,
        debug_log: &mut DebugLog,
    ) {
        if !self.recorder.is_enabled {
            return;
        }
        if self.recorder.sector_table.len() <= self.persisted_sector_count {
            return;
        }
        let save_dir = match &self.storage.save_dir {
            Some(dir) => dir.clone(),
            None => return,
        };
        let save_id = match read_manifest(&save_dir) {
            Some((save_id, _)) => save_id,
            None => return,
        };
        let count = self.recorder.sector_table.len() as u32;
        let ok = write_level_bundle(&save_dir, &level_manager.levels, &level_manager.level_set)
            && write_manifest(&save_dir, save_id, count)
            && write_sector_table(&save_dir, &self.recorder.sector_table);
        if !ok {
            let reason = "[History] could not persist an editor-created sector to the save";
            debug_log.warning(String::from(reason));
            panic!("{}", reason);
        }
        self.persisted_sector_count = self.recorder.sector_table.len();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn find_existing_save(&self) -> Option<(PathBuf, Vec<u8>)> {
        let base = Path::new(SAVE_DIR_NAME);
        let entries = match fs::read_dir(base) {
            Ok(entries) => entries,
            Err(_) => return None,
        };
        let mut best: Option<(PathBuf, u128, Vec<u8>)> = None;
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            if entry.file_name().to_string_lossy().ends_with(".tmp") {
                continue;
            }
            let manifest_bytes = match read_manifest_bytes(&entry.path()) {
                Some(bytes) => bytes,
                None => panic!(
                    "[History] save directory {} could not read its manifest file",
                    entry.path().display()
                ),
            };
            let (save_id, _sector_count) = match parse_manifest_bytes(&manifest_bytes) {
                Some(manifest) => manifest,
                None => panic!(
                    "[History] save directory {} has a corrupt manifest (bad magic or a version it can't read)",
                    entry.path().display()
                ),
            };
            let is_newer = match &best {
                Some((existing_path, existing_id, _)) => {
                    save_id > *existing_id
                        || (save_id == *existing_id && entry.path() > *existing_path)
                }
                None => true,
            };
            if is_newer {
                best = Some((entry.path(), save_id, manifest_bytes));
            }
        }
        match best {
            Some((path, _save_id, manifest_bytes)) => Some((path, manifest_bytes)),
            None => None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn restore_from_save(
        &mut self,
        save_dir: &Path,
        manifest_bytes: &[u8],
        history_files: &[HistoryFileRef],
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
        debug_log: &mut DebugLog,
    ) -> bool {
        let head_bytes = read_head_state(save_dir);
        let (_, head_restored) = self.restore_from_head_bytes(
            manifest_bytes,
            head_bytes,
            None,
            history_files,
            unit_manager,
            level_manager,
            debug_log,
        );
        head_restored
    }

    fn restore_from_head_bytes(
        &mut self,
        manifest_bytes: &[u8],
        head_bytes: Option<Vec<u8>>,
        head_prev_bytes: Option<Vec<u8>>,
        history_files: &[HistoryFileRef],
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
        debug_log: &mut DebugLog,
    ) -> (bool, bool) {
        let Some((_, sector_count)) = parse_manifest_bytes(manifest_bytes) else {
            let reason = "[History] restore fed an unparseable manifest";
            debug_log.warning(String::from(reason));
            panic!("{}", reason);
        };
        if sector_count as usize != self.recorder.sector_table.len() {
            let reason = format!(
                "[History] Manifest sector_count {} != derived {} - save made against a different level set",
                sector_count,
                self.recorder.sector_table.len()
            );
            debug_log.warning(reason.clone());
            panic!("{}", reason);
        }

        let primary_state = match &head_bytes {
            Some(bytes) => decode_head_state(bytes),
            None => None,
        };
        let primary_decoded = primary_state.is_some();
        let state = match primary_state {
            Some(state) => state,
            None => {
                let fallback_state = match &head_prev_bytes {
                    Some(bytes) => decode_head_state(bytes),
                    None => None,
                };
                match fallback_state {
                    Some(state) => {
                        debug_log.log(String::from(
                            "[History] head.ths unreadable, restored from head.ths.prev",
                        ));
                        state
                    }
                    None => {
                        if !history_files.is_empty() {
                            let reason =
                                "[History] save has history files but no readable head.ths";
                            debug_log.warning(String::from(reason));
                            panic!("{}", reason);
                        }
                        debug_log.log(String::from("[History] No head.ths found, starting fresh"));
                        return (false, false);
                    }
                }
            }
        };

        self.recorder
            .apply_head_state(&state, unit_manager, level_manager);
        unit_manager.turn_count = state.turn;
        self.recorder.head_turn = state.turn;
        self.player_state = state.player_state;
        self.recorder.rewind_floor = state.rewind_floor;

        let source = if primary_decoded {
            "head.ths"
        } else {
            "head.ths.prev"
        };
        debug_log.log(format!(
            "[History] Restored from {} at turn {}",
            source, state.turn
        ));
        (primary_decoded, true)
    }

    #[cfg(target_arch = "wasm32")]
    fn replace_levels_from_bundle(
        &mut self,
        bundle_bytes: &[u8],
        level_manager: &mut LevelManager,
        debug_log: &mut DebugLog,
    ) -> bool {
        let mut bundle_levels = HashMap::new();
        let mut bundle_level_set = None;
        let level_set_name = level_set_file_name();
        if let Some(entries) = parse_bundle_container(bundle_bytes) {
            for (name, data) in entries {
                if name == level_set_name.as_str() {
                    bundle_level_set = decode_bundled_level_set(data, debug_log);
                } else if let Some((pos, level)) = decode_bundled_level(name, data, debug_log) {
                    bundle_levels.insert(pos, level);
                }
            }
        }
        if bundle_levels.is_empty() {
            if !bundle_bytes.is_empty() {
                debug_log.warning(String::from(NO_LEVEL_BUNDLE_WARNING));
            }
            return false;
        }
        level_manager.levels = bundle_levels;
        if bundle_level_set.is_some() {
            level_manager.level_set = bundle_level_set;
        }
        true
    }

    #[cfg(target_arch = "wasm32")]
    fn take_web_boot_probe(
        &mut self,
        level_manager: &mut LevelManager,
        debug_log: &mut DebugLog,
    ) -> Option<BootProbe> {
        let blob = self.web_boot_blob.take()?;
        let probe = match parse_boot_container(&blob) {
            Some(probe) => probe,
            None => {
                let reason = "[History] OPFS boot probe reported ready with an undecodable blob";
                debug_log.warning(String::from(reason));
                panic!("{}", reason);
            }
        };

        if probe.has_save {
            if parse_manifest_bytes(&probe.manifest_bytes).is_none() {
                let reason = "[History] save has no readable manifest";
                debug_log.warning(String::from(reason));
                panic!("{}", reason);
            }
            let bundle_applied =
                self.replace_levels_from_bundle(&probe.bundle_bytes, level_manager, debug_log);
            debug_assert!(
                !bundle_applied
                    || !probe.head_bytes.is_empty()
                    || !probe.head_prev_bytes.is_empty(),
                "level bundle applied with no head.ths or head.ths.prev bytes present"
            );
        }

        Some(probe)
    }
}

#[cfg(target_arch = "wasm32")]
fn validate_web_boot_blob(blob: &[u8]) -> Option<&'static str> {
    let Some(probe) = parse_boot_container(blob) else {
        return Some("[History] OPFS boot probe reported ready with an undecodable blob");
    };
    if !probe.has_save {
        return None;
    }
    let Some((_, sector_count)) = parse_manifest_bytes(&probe.manifest_bytes) else {
        return Some("[History] save has no readable manifest");
    };
    if !probe.sectors_bytes.is_empty() {
        match parse_sector_table_bytes(&probe.sectors_bytes) {
            Some(table) => {
                if table.len() != sector_count as usize {
                    return Some("[History] save sector table does not match its manifest");
                }
            }
            None => return Some("[History] save has no readable sector table"),
        }
    }
    if !probe.files.is_empty()
        && decode_head_state(&probe.head_bytes).is_none()
        && decode_head_state(&probe.head_prev_bytes).is_none()
    {
        return Some("[History] save has history files but no readable head.ths");
    }
    None
}
