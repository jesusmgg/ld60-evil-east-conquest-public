use crate::assets::asset_path::ASSETS_DIR;
use crate::constants::{
    HISTORY_CHUNK_TURN_FIRST_OFFSET, HISTORY_CHUNK_TURN_LAST_OFFSET, HISTORY_FILE_BYTES,
    HISTORY_FILE_EXT, HISTORY_FILE_HEADER_BYTES, HISTORY_FILE_MAGIC, HISTORY_FILE_PATCH_OFFSET,
    HISTORY_FILE_TIER_OFFSET, HISTORY_FILE_TURN_FIRST_OFFSET, HISTORY_LEVELS_DIR_NAME,
    HISTORY_MANIFEST_NAME, HISTORY_SECTORS_NAME, HISTORY_SNAPSHOTS_NAME, HISTORY_STATE_MAGIC,
    HISTORY_STATE_NAME, HISTORY_STATE_PREV_SUFFIX, HISTORY_TIER_COUNT, HISTORY_TIER_RATIO,
};
use crate::debug::DebugLog;
use crate::game_data::game_data::DATA_VERSION;
use crate::history_manager::recorder::CHUNK_HEADER_BYTES;
use crate::history_manager::service::{
    build_manifest_bytes, build_sector_table_bytes, bundled_level_file_name, decode_bundled_level,
    decode_bundled_level_bytes, decode_bundled_level_set, encode_bundled_level,
    encode_bundled_level_set, head_state_prev_name, history_file_ref_from_header,
    level_set_file_name, parse_history_file_bytes, parse_manifest_bytes, parse_sector_table_bytes,
    Chunk, HistoryFileRef, LoadedFile,
};
use crate::history_manager::snapshots::{decode_snapshots, Snapshot};
use crate::level::level_manager::{Level, LevelManager, LevelSet};
use macroquad::math::IVec2;
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle, Thread};

struct WorkerConfig {
    queue: Arc<Mutex<VecDeque<WorkerCommand>>>,
    is_running: Arc<AtomicBool>,
    save_dir: PathBuf,
    loaded_mailbox: Arc<Mutex<Option<LoadedFile>>>,
    closed_files: Arc<Mutex<Vec<HistoryFileRef>>>,
    has_error: Arc<AtomicBool>,
    initial_sequences: [u32; HISTORY_TIER_COUNT],
}

static WORKER_CONFIG: OnceLock<WorkerConfig> = OnceLock::new();

fn worker_thread_entry() {
    let config = match WORKER_CONFIG.get() {
        Some(config) => config,
        None => panic!("worker config not set before spawn"),
    };
    worker_loop(
        Arc::clone(&config.queue),
        Arc::clone(&config.is_running),
        config.save_dir.clone(),
        Arc::clone(&config.loaded_mailbox),
        Arc::clone(&config.closed_files),
        Arc::clone(&config.has_error),
        config.initial_sequences,
    );
}

pub enum WorkerCommand {
    WriteChunk {
        tier: u8,
        bytes: Vec<u8>,
    },
    WriteHeadState {
        bytes: Vec<u8>,
    },
    WriteSnapshots {
        bytes: Vec<u8>,
    },
    RequestLoad {
        path: PathBuf,
        sequence: u32,
        tier: u8,
    },
    DeleteFile {
        path: PathBuf,
    },
    RepublishChunks {
        tier: u8,
        chunks: Vec<Chunk>,
    },
}

struct OpenFile {
    file: fs::File,
    path: PathBuf,
    sequence: u32,
    chunk_count: u32,
    turn_first: u32,
    turn_last: u32,
    bytes_written: usize,
}

pub struct DesktopHistoryProvider {
    queue: Arc<Mutex<VecDeque<WorkerCommand>>>,
    is_running: Arc<AtomicBool>,
    worker_thread: Option<Thread>,
    worker_handle: Option<JoinHandle<()>>,
    loaded_mailbox: Arc<Mutex<Option<LoadedFile>>>,
    closed_files: Arc<Mutex<Vec<HistoryFileRef>>>,
    pub has_error: Arc<AtomicBool>,
}

impl DesktopHistoryProvider {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            worker_thread: None,
            worker_handle: None,
            loaded_mailbox: Arc::new(Mutex::new(None)),
            closed_files: Arc::new(Mutex::new(Vec::new())),
            has_error: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn spawn_worker(
        &mut self,
        save_dir: PathBuf,
        initial_sequences: [u32; HISTORY_TIER_COUNT],
        debug_log: &mut DebugLog,
    ) {
        self.is_running.store(true, Ordering::Release);

        if WORKER_CONFIG
            .set(WorkerConfig {
                queue: Arc::clone(&self.queue),
                is_running: Arc::clone(&self.is_running),
                save_dir,
                loaded_mailbox: Arc::clone(&self.loaded_mailbox),
                closed_files: Arc::clone(&self.closed_files),
                has_error: Arc::clone(&self.has_error),
                initial_sequences,
            })
            .is_err()
        {
            panic!("worker already spawned");
        }

        let handle = thread::Builder::new()
            .name("history-writer".to_string())
            .spawn(worker_thread_entry)
            .expect("failed to spawn history writer thread");

        self.worker_thread = Some(handle.thread().clone());
        self.worker_handle = Some(handle);
        debug_log.log(String::from("[History] Desktop writer thread spawned"));
    }

    pub fn publish(&self, command: WorkerCommand) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(command);
        drop(queue);
        if let Some(thread) = &self.worker_thread {
            thread.unpark();
        }
    }

    pub fn shutdown(&mut self) {
        self.is_running.store(false, Ordering::Release);
        if let Some(thread) = &self.worker_thread {
            thread.unpark();
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        self.worker_thread = None;
    }

    pub fn request_load(&self, path: PathBuf, sequence: u32, tier: u8) {
        self.publish(WorkerCommand::RequestLoad {
            path,
            sequence,
            tier,
        });
    }

    pub fn poll_loaded(&self) -> Option<LoadedFile> {
        let mut mailbox = self.loaded_mailbox.lock().unwrap();
        mailbox.take()
    }

    pub fn poll_closed_files(&self) -> Vec<HistoryFileRef> {
        let mut closed = self.closed_files.lock().unwrap();
        mem::take(&mut closed)
    }

    pub fn delete_file(&self, path: PathBuf) {
        self.publish(WorkerCommand::DeleteFile { path });
    }
}

fn worker_loop(
    queue: Arc<Mutex<VecDeque<WorkerCommand>>>,
    is_running: Arc<AtomicBool>,
    save_dir: PathBuf,
    loaded_mailbox: Arc<Mutex<Option<LoadedFile>>>,
    closed_files: Arc<Mutex<Vec<HistoryFileRef>>>,
    has_error: Arc<AtomicBool>,
    initial_sequences: [u32; HISTORY_TIER_COUNT],
) {
    let mut open_files: Vec<Option<OpenFile>> = Vec::new();
    for _ in 0..HISTORY_TIER_COUNT {
        open_files.push(None);
    }
    let mut file_sequences: Vec<u32> = initial_sequences.to_vec();

    loop {
        thread::park();

        loop {
            let command = {
                let mut locked_queue = queue.lock().unwrap();
                locked_queue.pop_front()
            };

            let Some(command) = command else {
                break;
            };

            match command {
                WorkerCommand::WriteChunk { tier, bytes } => {
                    let tier_index = tier as usize;
                    if tier_index >= HISTORY_TIER_COUNT {
                        continue;
                    }
                    if bytes.len() < CHUNK_HEADER_BYTES {
                        has_error.store(true, Ordering::Release);
                        continue;
                    }
                    let turn_first = u32::from_le_bytes([
                        bytes[HISTORY_CHUNK_TURN_FIRST_OFFSET],
                        bytes[HISTORY_CHUNK_TURN_FIRST_OFFSET + 1],
                        bytes[HISTORY_CHUNK_TURN_FIRST_OFFSET + 2],
                        bytes[HISTORY_CHUNK_TURN_FIRST_OFFSET + 3],
                    ]);
                    let turn_last = u32::from_le_bytes([
                        bytes[HISTORY_CHUNK_TURN_LAST_OFFSET],
                        bytes[HISTORY_CHUNK_TURN_LAST_OFFSET + 1],
                        bytes[HISTORY_CHUNK_TURN_LAST_OFFSET + 2],
                        bytes[HISTORY_CHUNK_TURN_LAST_OFFSET + 3],
                    ]);

                    if open_files[tier_index].is_none() {
                        let sequence = file_sequences[tier_index];
                        file_sequences[tier_index] += 1;
                        let file_name = format!(
                            "t{}-{:010}-{:06}.{}.tmp",
                            tier, turn_first, sequence, HISTORY_FILE_EXT
                        );
                        let path = save_dir.join(&file_name);
                        match fs::File::create(&path) {
                            Ok(mut file) => {
                                let header = build_file_header(tier, turn_first);
                                if file.write_all(&header).is_err() {
                                    has_error.store(true, Ordering::Release);
                                    continue;
                                }
                                open_files[tier_index] = Some(OpenFile {
                                    file,
                                    path,
                                    sequence,
                                    chunk_count: 0,
                                    turn_first,
                                    turn_last,
                                    bytes_written: header.len(),
                                });
                            }
                            Err(_) => {
                                has_error.store(true, Ordering::Release);
                                continue;
                            }
                        }
                    }

                    if let Some(open_file) = &mut open_files[tier_index] {
                        let chunk_len = bytes.len() as u32;
                        if open_file.file.write_all(&chunk_len.to_le_bytes()).is_err() {
                            has_error.store(true, Ordering::Release);
                            continue;
                        }
                        if open_file.file.write_all(&bytes).is_err() {
                            has_error.store(true, Ordering::Release);
                            continue;
                        }
                        open_file.chunk_count += 1;
                        open_file.turn_last = turn_last;
                        open_file.bytes_written += 4 + bytes.len();

                        if open_file.bytes_written >= HISTORY_FILE_BYTES {
                            let open_file = open_files[tier_index].take().unwrap();
                            if let Some(closed_ref) = close_open_file(&open_file, tier, &has_error)
                            {
                                closed_files.lock().unwrap().push(closed_ref);
                            }
                        }
                    }
                }
                WorkerCommand::WriteHeadState { bytes } => {
                    write_head_state(&save_dir, &bytes, &has_error);
                }
                WorkerCommand::WriteSnapshots { bytes } => {
                    write_snapshots(&save_dir, &bytes, &has_error);
                }
                WorkerCommand::RequestLoad {
                    path,
                    sequence,
                    tier,
                } => {
                    let loaded = load_history_file(&path, sequence, tier);
                    if loaded.is_none() {
                        has_error.store(true, Ordering::Release);
                    }
                    let mut mailbox = loaded_mailbox.lock().unwrap();
                    *mailbox = loaded;
                    drop(mailbox);
                }
                WorkerCommand::DeleteFile { path } => {
                    if fs::remove_file(&path).is_err() {
                        has_error.store(true, Ordering::Release);
                    }
                }
                WorkerCommand::RepublishChunks { tier, chunks } => {
                    let tier_index = tier as usize;
                    if tier_index >= HISTORY_TIER_COUNT || chunks.is_empty() {
                        continue;
                    }
                    let sequence = file_sequences[tier_index];
                    file_sequences[tier_index] += 1;
                    let closed_ref =
                        write_republished_file(&save_dir, tier, sequence, chunks, &has_error);
                    if let Some(closed_ref) = closed_ref {
                        closed_files.lock().unwrap().push(closed_ref);
                    }
                }
            }
        }

        if !is_running.load(Ordering::Acquire) {
            let locked_queue = queue.lock().unwrap();
            if locked_queue.is_empty() {
                break;
            }
        }
    }

    for tier_index in 0..HISTORY_TIER_COUNT {
        if let Some(open_file) = open_files[tier_index].take() {
            if let Some(closed_ref) = close_open_file(&open_file, tier_index as u8, &has_error) {
                closed_files.lock().unwrap().push(closed_ref);
            }
        }
    }
}

fn build_file_header(tier: u8, turn_first: u32) -> Vec<u8> {
    let mut header = Vec::with_capacity(HISTORY_FILE_HEADER_BYTES);
    header.extend_from_slice(&HISTORY_FILE_MAGIC);
    header.extend_from_slice(&(DATA_VERSION as u32).to_le_bytes());
    debug_assert_eq!(header.len(), HISTORY_FILE_TIER_OFFSET);
    header.extend_from_slice(&(tier as u32).to_le_bytes());
    header.extend_from_slice(&HISTORY_TIER_RATIO[tier as usize].to_le_bytes());
    debug_assert_eq!(header.len(), HISTORY_FILE_TURN_FIRST_OFFSET);
    header.extend_from_slice(&turn_first.to_le_bytes());
    debug_assert_eq!(header.len(), HISTORY_FILE_PATCH_OFFSET);
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header
}

fn write_republished_file(
    save_dir: &Path,
    tier: u8,
    sequence: u32,
    chunks: Vec<Chunk>,
    has_error: &Arc<AtomicBool>,
) -> Option<HistoryFileRef> {
    let turn_first = chunks[0].turn_first;
    let file_name = format!(
        "t{}-{:010}-{:06}.{}.tmp",
        tier, turn_first, sequence, HISTORY_FILE_EXT
    );
    let path = save_dir.join(&file_name);

    let mut file = match fs::File::create(&path) {
        Ok(file) => file,
        Err(_) => {
            has_error.store(true, Ordering::Release);
            return None;
        }
    };
    let header = build_file_header(tier, turn_first);
    if file.write_all(&header).is_err() {
        has_error.store(true, Ordering::Release);
        return None;
    }
    let mut bytes_written = header.len();
    let mut chunk_count: u32 = 0;
    let mut turn_last = turn_first;
    for chunk in &chunks {
        let chunk_len = chunk.bytes.len() as u32;
        if file.write_all(&chunk_len.to_le_bytes()).is_err() {
            has_error.store(true, Ordering::Release);
            return None;
        }
        if file.write_all(&chunk.bytes).is_err() {
            has_error.store(true, Ordering::Release);
            return None;
        }
        chunk_count += 1;
        turn_last = chunk.turn_last;
        bytes_written += 4 + chunk.bytes.len();
    }

    let open_file = OpenFile {
        file,
        path,
        sequence,
        chunk_count,
        turn_first,
        turn_last,
        bytes_written,
    };
    close_open_file(&open_file, tier, has_error)
}

fn load_history_file(path: &Path, sequence: u32, tier: u8) -> Option<LoadedFile> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return None,
    };
    parse_history_file_bytes(&bytes, sequence, tier)
}

fn close_open_file(
    open_file: &OpenFile,
    tier: u8,
    has_error: &Arc<AtomicBool>,
) -> Option<HistoryFileRef> {
    let mut patch = Vec::with_capacity(8);
    patch.extend_from_slice(&open_file.turn_last.to_le_bytes());
    patch.extend_from_slice(&open_file.chunk_count.to_le_bytes());

    let mut file = &open_file.file;
    let seek_ok = file
        .seek(SeekFrom::Start(HISTORY_FILE_PATCH_OFFSET as u64))
        .is_ok();
    let write_ok = seek_ok && file.write_all(&patch).is_ok();
    let patched = write_ok && file.flush().is_ok();
    if !patched {
        has_error.store(true, Ordering::Release);
        return None;
    }

    let final_name = match open_file.path.to_str() {
        Some(s) => match s.strip_suffix(".tmp") {
            Some(stripped) => stripped.to_string(),
            None => s.to_string(),
        },
        None => {
            has_error.store(true, Ordering::Release);
            return None;
        }
    };
    if fs::rename(&open_file.path, &final_name).is_err() {
        has_error.store(true, Ordering::Release);
        return None;
    }

    Some(HistoryFileRef {
        tier,
        sequence: open_file.sequence,
        turn_first: open_file.turn_first,
        turn_last: open_file.turn_last,
        byte_length: open_file.bytes_written as u32,
    })
}

fn write_head_state(save_dir: &Path, bytes: &[u8], has_error: &Arc<AtomicBool>) {
    write_with_prev(
        save_dir,
        HISTORY_STATE_NAME,
        &head_state_prev_name(),
        bytes,
        has_error,
    );
}

fn snapshots_prev_name() -> String {
    format!("{}{}", HISTORY_SNAPSHOTS_NAME, HISTORY_STATE_PREV_SUFFIX)
}

fn write_snapshots(save_dir: &Path, bytes: &[u8], has_error: &Arc<AtomicBool>) {
    write_with_prev(
        save_dir,
        HISTORY_SNAPSHOTS_NAME,
        &snapshots_prev_name(),
        bytes,
        has_error,
    );
}

fn write_with_prev(
    save_dir: &Path,
    name: &str,
    prev_name: &str,
    bytes: &[u8],
    has_error: &Arc<AtomicBool>,
) {
    let path = save_dir.join(name);
    let prev_path = save_dir.join(prev_name);
    let tmp_path = save_dir.join(format!("{}.tmp", name));
    if fs::write(&tmp_path, bytes).is_err() {
        has_error.store(true, Ordering::Release);
        return;
    }
    if path.exists() {
        if fs::rename(&path, &prev_path).is_err() {
            has_error.store(true, Ordering::Release);
            return;
        }
    }
    if fs::rename(&tmp_path, &path).is_err() {
        has_error.store(true, Ordering::Release);
    }
}

pub fn create_save_directory(base_dir: &Path, save_id: u128) -> Option<PathBuf> {
    if fs::create_dir_all(base_dir).is_err() {
        return None;
    }
    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(".tmp") {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    let temp_dir = base_dir.join(format!("{}.tmp", save_id));
    if fs::create_dir_all(&temp_dir).is_err() {
        return None;
    }
    let levels_dir = temp_dir.join(HISTORY_LEVELS_DIR_NAME);
    if fs::create_dir_all(&levels_dir).is_err() {
        return None;
    }
    Some(temp_dir)
}

pub fn finalize_save_directory(base_dir: &Path, save_id: u128) -> Option<PathBuf> {
    let temp_dir = base_dir.join(format!("{}.tmp", save_id));
    let save_dir = base_dir.join(save_id.to_string());
    match fs::rename(&temp_dir, &save_dir) {
        Ok(()) => Some(save_dir),
        Err(_) => None,
    }
}

pub fn write_manifest(save_dir: &Path, save_id: u128, sector_count: u32) -> bool {
    let manifest_path = save_dir.join(HISTORY_MANIFEST_NAME);
    fs::write(&manifest_path, build_manifest_bytes(save_id, sector_count)).is_ok()
}

pub fn read_manifest_bytes(save_dir: &Path) -> Option<Vec<u8>> {
    fs::read(save_dir.join(HISTORY_MANIFEST_NAME)).ok()
}

#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
pub fn read_manifest(save_dir: &Path) -> Option<(u128, u32)> {
    let bytes = read_manifest_bytes(save_dir)?;
    parse_manifest_bytes(&bytes)
}

pub fn write_sector_table(save_dir: &Path, table: &[IVec2]) -> bool {
    let path = save_dir.join(HISTORY_SECTORS_NAME);
    fs::write(&path, build_sector_table_bytes(table)).is_ok()
}

pub fn read_sector_table(save_dir: &Path) -> Option<Vec<IVec2>> {
    let bytes = fs::read(save_dir.join(HISTORY_SECTORS_NAME)).ok()?;
    parse_sector_table_bytes(&bytes)
}

pub fn sweep_tmp_files(save_dir: &Path) -> u32 {
    let mut count = 0;
    let entries = match fs::read_dir(save_dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with(".tmp") {
            if fs::remove_file(entry.path()).is_ok() {
                count += 1;
            }
        }
    }
    count
}

pub fn read_head_state(save_dir: &Path) -> Option<Vec<u8>> {
    let head_path = save_dir.join(HISTORY_STATE_NAME);
    if let Ok(bytes) = fs::read(&head_path) {
        if bytes.len() >= 8 && bytes[0..8] == HISTORY_STATE_MAGIC {
            return Some(bytes);
        }
    }
    let prev_path = save_dir.join(head_state_prev_name());
    if let Ok(bytes) = fs::read(&prev_path) {
        if bytes.len() >= 8 && bytes[0..8] == HISTORY_STATE_MAGIC {
            return Some(bytes);
        }
    }
    None
}

pub fn read_snapshots(save_dir: &Path) -> Vec<Snapshot> {
    if let Ok(bytes) = fs::read(save_dir.join(HISTORY_SNAPSHOTS_NAME)) {
        if let Some(snapshots) = decode_snapshots(&bytes) {
            return snapshots;
        }
    }
    if let Ok(bytes) = fs::read(save_dir.join(snapshots_prev_name())) {
        if let Some(snapshots) = decode_snapshots(&bytes) {
            return snapshots;
        }
    }
    Vec::new()
}

pub fn scan_history_files(save_dir: &Path) -> Vec<HistoryFileRef> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(save_dir) {
        Ok(entries) => entries,
        Err(_) => return files,
    };
    let ext_suffix = format!(".{}", HISTORY_FILE_EXT);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.ends_with(&ext_suffix) {
            continue;
        }

        let path = entry.path();
        let byte_length = match entry.metadata() {
            Ok(metadata) => metadata.len() as u32,
            Err(_) => continue,
        };

        let mut header = [0u8; HISTORY_FILE_HEADER_BYTES];
        let read_ok = match fs::File::open(&path) {
            Ok(mut file) => file.read_exact(&mut header).is_ok(),
            Err(_) => false,
        };
        if !read_ok {
            continue;
        }

        if let Some(file_ref) = history_file_ref_from_header(&name_str, byte_length, &header) {
            files.push(file_ref);
        }
    }

    files.sort_by(history_file_ref_order);
    files
}

fn history_file_ref_order(a: &HistoryFileRef, b: &HistoryFileRef) -> CmpOrdering {
    (a.tier, a.turn_first, a.sequence).cmp(&(b.tier, b.turn_first, b.sequence))
}

pub fn write_level_bundle(
    save_dir: &Path,
    levels: &HashMap<IVec2, Level>,
    level_set: &Option<LevelSet>,
) -> bool {
    let levels_dir = save_dir.join(HISTORY_LEVELS_DIR_NAME);
    let mut all_ok = true;
    for (pos, level) in levels {
        let file_name = bundled_level_file_name(*pos);
        let bytes = encode_bundled_level(level);
        if fs::write(levels_dir.join(file_name), &bytes).is_err() {
            all_ok = false;
        }
    }
    if let Some(level_set) = level_set {
        let bytes = encode_bundled_level_set(level_set);
        if fs::write(levels_dir.join(level_set_file_name()), &bytes).is_err() {
            all_ok = false;
        }
    }
    all_ok
}

pub fn read_level_bundle(
    save_dir: &Path,
    debug_log: &mut DebugLog,
) -> Option<(HashMap<IVec2, Level>, Option<LevelSet>)> {
    let levels_dir = save_dir.join(HISTORY_LEVELS_DIR_NAME);
    let entries = fs::read_dir(&levels_dir).ok()?;
    let mut levels = HashMap::new();
    let level_set_name = level_set_file_name();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == level_set_name.as_str() {
            continue;
        }
        let bytes = match fs::read(entry.path()) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        if let Some((pos, level)) = decode_bundled_level(&name_str, &bytes, debug_log) {
            levels.insert(pos, level);
        }
    }
    if levels.is_empty() {
        return None;
    }

    let level_set = match fs::read(levels_dir.join(level_set_name)) {
        Ok(bytes) => decode_bundled_level_set(&bytes, debug_log),
        Err(_) => None,
    };

    Some((levels, level_set))
}

pub fn refresh_bundle_from_assets(
    bundle_levels: &mut HashMap<IVec2, Level>,
    bundle_level_set: &mut Option<LevelSet>,
    debug_log: &mut DebugLog,
) -> bool {
    refresh_bundle_from_assets_in(
        bundle_levels,
        bundle_level_set,
        Path::new(ASSETS_DIR),
        debug_log,
    )
}

fn refresh_bundle_from_assets_in(
    bundle_levels: &mut HashMap<IVec2, Level>,
    bundle_level_set: &mut Option<LevelSet>,
    assets_dir: &Path,
    debug_log: &mut DebugLog,
) -> bool {
    let mut refreshed = 0;
    let mut spawn_drifted = false;
    for level in bundle_levels.values_mut() {
        let path = assets_dir.join(LevelManager::level_file_path_from_name(&level.file_name));
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        match decode_bundled_level_bytes(&bytes, debug_log) {
            Some(asset_level) => {
                if asset_level.unit_spawns != level.unit_spawns {
                    spawn_drifted = true;
                }
                *level = asset_level;
                refreshed += 1;
            }
            None => debug_log.warning(format!(
                "[History] assets version of level '{}' failed to decode, keeping save copy",
                level.file_name
            )),
        }
    }
    let level_set_path = assets_dir.join(LevelManager::level_set_file_path());
    if let Ok(bytes) = fs::read(&level_set_path) {
        match decode_bundled_level_set(&bytes, debug_log) {
            Some(mut level_set) => {
                let mut drop_positions = Vec::new();
                for pos in level_set.level_grid.keys() {
                    if !bundle_levels.contains_key(pos) {
                        drop_positions.push(*pos);
                    }
                }
                for pos in drop_positions {
                    level_set.level_grid.remove(&pos);
                }
                *bundle_level_set = Some(level_set);
                debug_log.log(String::from("[History] Refreshed level_set from assets/"));
            }
            None => debug_log.warning(String::from(
                "[History] assets level_set.asb failed to decode, keeping save copy",
            )),
        }
    }
    debug_log.log(format!(
        "[History] Refreshed {} of {} bundled levels from assets/",
        refreshed,
        bundle_levels.len()
    ));
    spawn_drifted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y};
    use crate::enums::Direction;
    use crate::unit_design::{UnitDesign, UnitKind, UnitProfile};
    use crate::unit_manager::{UnitOwner, UnitSpawnData, UnitType};

    fn test_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bundle-refresh-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("levels")).unwrap();
        dir
    }

    fn write_asset_level(dir: &Path, file_name: &str, ambient: f32) {
        let mut level = Level::new(
            file_name,
            "Test",
            IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
        );
        level.ambient_light_level = ambient;
        let bytes = encode_bundled_level(&level);
        fs::write(dir.join("levels").join(format!("{}.asb", file_name)), bytes).unwrap();
    }

    #[test]
    fn asset_version_wins_over_bundle_copy() {
        let dir = test_dir("wins");
        write_asset_level(&dir, "level00", 1.0);
        let mut bundle_level = Level::new(
            "level00",
            "Test",
            IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
        );
        bundle_level.ambient_light_level = 0.05;
        let mut levels = HashMap::new();
        levels.insert(IVec2::new(0, 0), bundle_level);
        let mut level_set = None;
        let mut debug_log = DebugLog::new();
        let drifted =
            refresh_bundle_from_assets_in(&mut levels, &mut level_set, &dir, &mut debug_log);
        assert!(!drifted);
        assert_eq!(levels[&IVec2::new(0, 0)].ambient_light_level, 1.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn spawn_drift_detected_when_spawns_differ() {
        let dir = test_dir("drift");
        write_asset_level(&dir, "level00", 1.0);
        let mut bundle_level = Level::new(
            "level00",
            "Test",
            IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
        );
        bundle_level.unit_spawns.insert(
            IVec2::new(2, 1),
            UnitSpawnData {
                owner: UnitOwner::P0,
                unit_type: UnitType::Soldier,
                tile_pos: IVec2::new(2, 1),
                direction: Direction::Right,
                profile: UnitProfile::new(UnitDesign::default_for(UnitKind::Soldier)),
                waypoints: Vec::new(),
            },
        );
        let mut levels = HashMap::new();
        levels.insert(IVec2::new(0, 0), bundle_level);
        let mut level_set = None;
        let mut debug_log = DebugLog::new();
        let drifted =
            refresh_bundle_from_assets_in(&mut levels, &mut level_set, &dir, &mut debug_log);
        assert!(drifted);
        assert_eq!(levels[&IVec2::new(0, 0)].ambient_light_level, 1.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundle_only_entry_survives_without_asset_file() {
        let dir = test_dir("kept");
        let mut bundle_level = Level::new(
            "unsaved",
            "Test",
            IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
        );
        bundle_level.ambient_light_level = 0.05;
        let mut levels = HashMap::new();
        levels.insert(IVec2::new(3, 3), bundle_level);
        let mut level_set = None;
        let mut debug_log = DebugLog::new();
        let drifted =
            refresh_bundle_from_assets_in(&mut levels, &mut level_set, &dir, &mut debug_log);
        assert!(!drifted);
        assert_eq!(levels[&IVec2::new(3, 3)].ambient_light_level, 0.05);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_asset_file_keeps_bundle_copy() {
        let dir = test_dir("corrupt");
        fs::write(dir.join("levels").join("level00.asb"), b"not a level").unwrap();
        let mut bundle_level = Level::new(
            "level00",
            "Test",
            IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
        );
        bundle_level.ambient_light_level = 0.05;
        let mut levels = HashMap::new();
        levels.insert(IVec2::new(0, 0), bundle_level);
        let mut level_set = None;
        let mut debug_log = DebugLog::new();
        let drifted =
            refresh_bundle_from_assets_in(&mut levels, &mut level_set, &dir, &mut debug_log);
        assert!(!drifted);
        assert_eq!(levels[&IVec2::new(0, 0)].ambient_light_level, 0.05);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn level_set_grid_prunes_sectors_outside_bundle() {
        let dir = test_dir("prune");
        let mut grid = HashMap::new();
        grid.insert(IVec2::new(0, 0), String::from("level00"));
        grid.insert(IVec2::new(9, 9), String::from("level99"));
        let level_set = LevelSet {
            level_grid: grid,
            initial_level_name: String::from("fresh"),
        };
        let bytes = encode_bundled_level_set(&level_set);
        fs::write(dir.join("levels").join("level_set.asb"), bytes).unwrap();
        let mut levels = HashMap::new();
        levels.insert(
            IVec2::new(0, 0),
            Level::new(
                "level00",
                "Test",
                IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
            ),
        );
        let mut level_set = None;
        let mut debug_log = DebugLog::new();
        let drifted =
            refresh_bundle_from_assets_in(&mut levels, &mut level_set, &dir, &mut debug_log);
        assert!(!drifted);
        let grid = &level_set.unwrap().level_grid;
        assert_eq!(grid.len(), 1);
        assert!(grid.contains_key(&IVec2::new(0, 0)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn level_set_refreshes_from_assets() {
        let dir = test_dir("set");
        let level_set = LevelSet {
            level_grid: HashMap::new(),
            initial_level_name: String::from("fresh"),
        };
        let bytes = encode_bundled_level_set(&level_set);
        fs::write(dir.join("levels").join("level_set.asb"), bytes).unwrap();
        let mut levels = HashMap::new();
        let stale_set = LevelSet {
            level_grid: HashMap::new(),
            initial_level_name: String::from("stale"),
        };
        let mut level_set = Some(stale_set);
        let mut debug_log = DebugLog::new();
        let drifted =
            refresh_bundle_from_assets_in(&mut levels, &mut level_set, &dir, &mut debug_log);
        assert!(!drifted);
        assert_eq!(level_set.unwrap().initial_level_name, String::from("fresh"));
        let _ = fs::remove_dir_all(&dir);
    }
}
