#[cfg(target_arch = "wasm32")]
use crate::constants::HISTORY_CLOSED_RECORD_BYTES;
#[cfg(not(target_arch = "wasm32"))]
use crate::constants::HISTORY_FILE_EXT;
use crate::constants::{
    HISTORY_FILE_HEADER_BYTES, HISTORY_FILE_MAGIC, HISTORY_FILE_PATCH_OFFSET,
    HISTORY_FILE_TIER_OFFSET, HISTORY_FILE_TURN_FIRST_OFFSET, HISTORY_MANIFEST_BYTES,
    HISTORY_MANIFEST_MAGIC, HISTORY_MANIFEST_SAVE_ID_OFFSET, HISTORY_SECTORS_MAGIC,
    HISTORY_SECTOR_TABLE_HEADER_BYTES, HISTORY_STATE_NAME, HISTORY_STATE_PREV_SUFFIX,
    HISTORY_TIER_COUNT, HISTORY_TIER_RATIO,
};
use crate::debug::DebugLog;
use crate::game_data::game_data::{
    is_readable_save_version, Versioned, ASSET_BIN_EXT, DATA_VERSION,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::history_manager::provider::desktop::{
    sweep_tmp_files, DesktopHistoryProvider, WorkerCommand,
};
#[cfg(target_arch = "wasm32")]
use crate::history_manager::provider::web::WebHistoryProvider;
use crate::history_manager::recorder::{read_chunk_header, CHUNK_HEADER_BYTES, FRAME_FIXED_BYTES};
use crate::level::level_manager::{Level, LevelSet, LEVEL_SET_ASSET_NAME};
use macroquad::math::IVec2;
use nanoserde::{DeBin, SerBin};
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

pub struct Chunk {
    pub bytes: Vec<u8>,
    pub turn_first: u32,
    pub turn_last: u32,
    pub used_bytes: u32,
    pub frame_count: u32,
}

pub struct LoadedFile {
    pub sequence: u32,
    pub tier: u8,
    pub chunks: Vec<Chunk>,
}

pub struct HistoryFileRef {
    pub tier: u8,
    pub sequence: u32,
    pub turn_first: u32,
    pub turn_last: u32,
    pub byte_length: u32,
}

pub struct HistoryStorageService {
    #[cfg(not(target_arch = "wasm32"))]
    pub provider_desktop: DesktopHistoryProvider,
    #[cfg(target_arch = "wasm32")]
    pub provider_web: WebHistoryProvider,

    pub files: Vec<HistoryFileRef>,
    #[cfg(not(target_arch = "wasm32"))]
    pub save_dir: Option<PathBuf>,
    pub is_started: bool,
}

impl HistoryStorageService {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            provider_desktop: DesktopHistoryProvider::new(),
            #[cfg(target_arch = "wasm32")]
            provider_web: WebHistoryProvider::new(),

            files: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            save_dir: None,
            is_started: false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn init(
        &mut self,
        debug_log: &mut DebugLog,
        save_dir: PathBuf,
        files: Vec<HistoryFileRef>,
    ) {
        let tmp_count = sweep_tmp_files(&save_dir);
        if tmp_count > 0 {
            debug_log.log(format!("[History] Swept {} stale .tmp files", tmp_count));
        }
        self.files = files;
        let initial_sequences = next_sequences(&self.files);
        self.save_dir = Some(save_dir.clone());
        self.provider_desktop
            .spawn_worker(save_dir, initial_sequences, debug_log);
        self.is_started = true;
    }

    pub fn publish_chunk(&self, tier: u8, bytes: Vec<u8>) {
        if !self.is_started {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.provider_desktop
            .publish(WorkerCommand::WriteChunk { tier, bytes });
        #[cfg(target_arch = "wasm32")]
        self.provider_web.write_chunk(tier, &bytes);
    }

    pub fn write_head_state(&self, bytes: Vec<u8>) {
        if !self.is_started {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.provider_desktop
            .publish(WorkerCommand::WriteHeadState { bytes });
        #[cfg(target_arch = "wasm32")]
        self.provider_web.write_head_state(&bytes);
    }

    pub fn write_snapshots(&self, bytes: Vec<u8>) {
        if !self.is_started {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.provider_desktop
            .publish(WorkerCommand::WriteSnapshots { bytes });
        #[cfg(target_arch = "wasm32")]
        self.provider_web.write_snapshots(&bytes);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn mark_web_inherited_head_good(&self) {
        self.provider_web.mark_inherited_head_good();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn shutdown(&mut self) {
        if !self.is_started {
            return;
        }
        self.provider_desktop.shutdown();
        self.is_started = false;
    }

    fn find_file_index(&self, tier: u8, sequence: u32) -> Option<usize> {
        for index in 0..self.files.len() {
            if self.files[index].tier == tier && self.files[index].sequence == sequence {
                return Some(index);
            }
        }
        None
    }

    pub fn request_load(&self, tier: u8, sequence: u32) -> bool {
        if !self.is_started {
            return false;
        }
        let Some(file_index) = self.find_file_index(tier, sequence) else {
            return false;
        };
        let file_ref = &self.files[file_index];
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.file_path(file_ref);
            self.provider_desktop.request_load(path, sequence, tier);
        }
        #[cfg(target_arch = "wasm32")]
        self.provider_web
            .request_load(tier, file_ref.turn_first, sequence);
        true
    }

    pub fn poll_loaded(&mut self) -> Option<LoadedFile> {
        if !self.is_started {
            return None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            return self.provider_desktop.poll_loaded();
        }
        #[cfg(target_arch = "wasm32")]
        {
            if self.provider_web.load_state() != 1 {
                return None;
            }
            let (tier, sequence) = self.provider_web.load_identity();
            let mut bytes = Vec::new();
            self.provider_web.load_copy(&mut bytes);
            let Some(loaded) = parse_history_file_bytes(&bytes, sequence, tier) else {
                self.provider_web.has_error = true;
                return None;
            };
            Some(loaded)
        }
    }

    pub fn poll_closed_files(&mut self) -> Vec<HistoryFileRef> {
        if !self.is_started {
            return Vec::new();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            return self.provider_desktop.poll_closed_files();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let mut files = Vec::new();
            let mut record = [0u8; HISTORY_CLOSED_RECORD_BYTES];
            while self.provider_web.closed_next(&mut record) {
                let tier = u32::from_le_bytes([record[0], record[1], record[2], record[3]]) as u8;
                let sequence = u32::from_le_bytes([record[4], record[5], record[6], record[7]]);
                let turn_first = u32::from_le_bytes([record[8], record[9], record[10], record[11]]);
                let turn_last =
                    u32::from_le_bytes([record[12], record[13], record[14], record[15]]);
                let byte_length =
                    u32::from_le_bytes([record[16], record[17], record[18], record[19]]);
                files.push(HistoryFileRef {
                    tier,
                    sequence,
                    turn_first,
                    turn_last,
                    byte_length,
                });
            }
            files
        }
    }

    pub fn delete_file(&self, tier: u8, sequence: u32) {
        if !self.is_started {
            return;
        }
        let Some(file_index) = self.find_file_index(tier, sequence) else {
            return;
        };
        let file_ref = &self.files[file_index];
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.file_path(file_ref);
            self.provider_desktop.delete_file(path);
        }
        #[cfg(target_arch = "wasm32")]
        self.provider_web
            .delete_file(tier, file_ref.turn_first, sequence);
    }

    pub fn remove_file_ref(&mut self, tier: u8, sequence: u32) {
        if let Some(file_index) = self.find_file_index(tier, sequence) {
            self.files.remove(file_index);
        }
    }

    pub fn republish_chunks(&self, tier: u8, chunks: Vec<Chunk>) {
        if !self.is_started {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.provider_desktop
            .publish(WorkerCommand::RepublishChunks { tier, chunks });
        #[cfg(target_arch = "wasm32")]
        self.provider_web
            .republish(tier, &build_republish_container(&chunks));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn file_path(&self, file_ref: &HistoryFileRef) -> PathBuf {
        let save_dir = match &self.save_dir {
            Some(dir) => dir,
            None => panic!("[History] file_path called before init resolved a save_dir"),
        };
        let file_name = format!(
            "t{}-{:010}-{:06}.{}",
            file_ref.tier, file_ref.turn_first, file_ref.sequence, HISTORY_FILE_EXT
        );
        save_dir.join(file_name)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn init_web_probe(&mut self) {
        self.provider_web.init(DATA_VERSION as u32);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn web_boot_state(&self) -> i32 {
        self.provider_web.ready()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn clear_web_failed_save(&self) {
        self.provider_web.clear_failed_save();
    }

    #[cfg(target_arch = "wasm32")]
    pub fn reset_web_game(&self) {
        self.provider_web.reset_game();
    }

    #[cfg(target_arch = "wasm32")]
    pub fn take_web_boot_blob(&mut self) -> Option<Vec<u8>> {
        if self.provider_web.boot_len() == 0 {
            return None;
        }
        let mut bytes = Vec::new();
        self.provider_web.boot_copy(&mut bytes);
        Some(bytes)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn commit_web_existing(&mut self, files: Vec<HistoryFileRef>) {
        self.files = files;
        let seeds = next_sequences(&self.files);
        self.provider_web.seed_sequences(&seeds);
        self.is_started = true;
    }

    #[cfg(target_arch = "wasm32")]
    pub fn write_web_bundle(&self, bundle: &[u8]) {
        if !self.is_started {
            return;
        }
        self.provider_web.write_bundle(bundle);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn commit_web_fresh(
        &mut self,
        manifest: &[u8],
        sectors: &[u8],
        bundle: &[u8],
        head: &[u8],
    ) {
        self.provider_web
            .commit_fresh(manifest, sectors, bundle, head);
        self.is_started = true;
    }

    #[cfg(target_arch = "wasm32")]
    pub fn web_persistent(&self) -> i32 {
        self.provider_web.persistent()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn web_storage_free(&self) -> f64 {
        self.provider_web.storage_free()
    }
}

pub(crate) fn read_u32_at(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    if *offset + 4 > bytes.len() {
        return None;
    }
    let value = u32::from_le_bytes([
        bytes[*offset],
        bytes[*offset + 1],
        bytes[*offset + 2],
        bytes[*offset + 3],
    ]);
    *offset += 4;
    Some(value)
}

pub(crate) fn read_u16_at(bytes: &[u8], offset: &mut usize) -> Option<u16> {
    if *offset + 2 > bytes.len() {
        return None;
    }
    let value = u16::from_le_bytes([bytes[*offset], bytes[*offset + 1]]);
    *offset += 2;
    Some(value)
}

pub fn build_manifest_bytes(save_id: u128, sector_count: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HISTORY_MANIFEST_BYTES);
    bytes.extend_from_slice(&HISTORY_MANIFEST_MAGIC);
    bytes.extend_from_slice(&(DATA_VERSION as u32).to_le_bytes());
    debug_assert_eq!(bytes.len(), HISTORY_MANIFEST_SAVE_ID_OFFSET);
    bytes.extend_from_slice(&save_id.to_le_bytes());
    bytes.extend_from_slice(&sector_count.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes
}

pub fn parse_manifest_bytes(bytes: &[u8]) -> Option<(u128, u32)> {
    if bytes.len() < HISTORY_MANIFEST_BYTES {
        return None;
    }
    if bytes[0..8] != HISTORY_MANIFEST_MAGIC {
        return None;
    }
    let mut offset = 8;
    let format_version = read_u32_at(bytes, &mut offset)?;
    if !is_readable_save_version(format_version) {
        return None;
    }
    debug_assert_eq!(offset, HISTORY_MANIFEST_SAVE_ID_OFFSET);
    let mut id_bytes = [0u8; 16];
    id_bytes.copy_from_slice(&bytes[offset..offset + 16]);
    let save_id = u128::from_le_bytes(id_bytes);
    offset += 16;
    let sector_count = read_u32_at(bytes, &mut offset)?;
    Some((save_id, sector_count))
}

pub fn build_sector_table_bytes(table: &[IVec2]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HISTORY_SECTOR_TABLE_HEADER_BYTES + table.len() * 8);
    bytes.extend_from_slice(&HISTORY_SECTORS_MAGIC);
    bytes.extend_from_slice(&(table.len() as u32).to_le_bytes());
    for pos in table {
        bytes.extend_from_slice(&pos.x.to_le_bytes());
        bytes.extend_from_slice(&pos.y.to_le_bytes());
    }
    bytes
}

pub fn parse_sector_table_bytes(bytes: &[u8]) -> Option<Vec<IVec2>> {
    if bytes.len() < HISTORY_SECTOR_TABLE_HEADER_BYTES {
        return None;
    }
    if bytes[0..8] != HISTORY_SECTORS_MAGIC {
        return None;
    }
    let mut offset = 8;
    let count = read_u32_at(bytes, &mut offset)? as usize;
    if count > (bytes.len() - offset) / 8 {
        return None;
    }
    let mut table = Vec::with_capacity(count);
    for _ in 0..count {
        let x = read_u32_at(bytes, &mut offset)? as i32;
        let y = read_u32_at(bytes, &mut offset)? as i32;
        table.push(IVec2::new(x, y));
    }
    Some(table)
}

pub fn parse_history_file_bytes(bytes: &[u8], sequence: u32, tier: u8) -> Option<LoadedFile> {
    if bytes.len() < HISTORY_FILE_HEADER_BYTES {
        return None;
    }
    if bytes[0..8] != HISTORY_FILE_MAGIC {
        return None;
    }

    let mut offset = 8;
    let format_version = read_u32_at(bytes, &mut offset)?;
    if !is_readable_save_version(format_version) {
        return None;
    }
    let file_tier = read_u32_at(bytes, &mut offset)? as u8;
    let tier_ratio = read_u32_at(bytes, &mut offset)?;
    let _turn_first = read_u32_at(bytes, &mut offset)?;
    debug_assert_eq!(offset, HISTORY_FILE_PATCH_OFFSET);
    let _turn_last = read_u32_at(bytes, &mut offset)?;
    let chunk_count = read_u32_at(bytes, &mut offset)? as usize;
    let _flags = read_u32_at(bytes, &mut offset)?;

    if file_tier != tier {
        return None;
    }
    if file_tier as usize >= HISTORY_TIER_COUNT
        || tier_ratio != HISTORY_TIER_RATIO[file_tier as usize]
    {
        return None;
    }

    if chunk_count > (bytes.len() - offset) / (4 + CHUNK_HEADER_BYTES) {
        return None;
    }
    let mut chunks = Vec::with_capacity(chunk_count);
    for _ in 0..chunk_count {
        let chunk_len = read_u32_at(bytes, &mut offset)? as usize;
        if chunk_len > bytes.len() - offset {
            return None;
        }
        let chunk_bytes = bytes[offset..offset + chunk_len].to_vec();
        offset += chunk_len;

        if chunk_bytes.len() < CHUNK_HEADER_BYTES {
            return None;
        }
        let mut header_offset = 0;
        let header = match read_chunk_header(&chunk_bytes, &mut header_offset) {
            Some(header) => header,
            None => return None,
        };
        if header.tier != tier as u32 {
            return None;
        }
        if chunk_bytes.len() - CHUNK_HEADER_BYTES != header.used_bytes as usize {
            return None;
        }
        if header.frame_count as usize > header.used_bytes as usize / FRAME_FIXED_BYTES {
            return None;
        }

        chunks.push(Chunk {
            bytes: chunk_bytes,
            turn_first: header.turn_first,
            turn_last: header.turn_last,
            used_bytes: header.used_bytes,
            frame_count: header.frame_count,
        });
    }

    Some(LoadedFile {
        sequence,
        tier,
        chunks,
    })
}

pub fn history_file_ref_from_header(
    name: &str,
    byte_length: u32,
    header: &[u8; HISTORY_FILE_HEADER_BYTES],
) -> Option<HistoryFileRef> {
    if header[0..8] != HISTORY_FILE_MAGIC {
        return None;
    }
    let tier_raw = u32::from_le_bytes([
        header[HISTORY_FILE_TIER_OFFSET],
        header[HISTORY_FILE_TIER_OFFSET + 1],
        header[HISTORY_FILE_TIER_OFFSET + 2],
        header[HISTORY_FILE_TIER_OFFSET + 3],
    ]);
    if tier_raw as usize >= HISTORY_TIER_COUNT {
        return None;
    }
    let tier = tier_raw as u8;
    let turn_first = u32::from_le_bytes([
        header[HISTORY_FILE_TURN_FIRST_OFFSET],
        header[HISTORY_FILE_TURN_FIRST_OFFSET + 1],
        header[HISTORY_FILE_TURN_FIRST_OFFSET + 2],
        header[HISTORY_FILE_TURN_FIRST_OFFSET + 3],
    ]);
    let turn_last = u32::from_le_bytes([
        header[HISTORY_FILE_PATCH_OFFSET],
        header[HISTORY_FILE_PATCH_OFFSET + 1],
        header[HISTORY_FILE_PATCH_OFFSET + 2],
        header[HISTORY_FILE_PATCH_OFFSET + 3],
    ]);
    let sequence = parse_sequence_from_name(name)?;
    Some(HistoryFileRef {
        tier,
        sequence,
        turn_first,
        turn_last,
        byte_length,
    })
}

fn parse_sequence_from_name(name: &str) -> Option<u32> {
    let without_ext = match name.rfind('.') {
        Some(pos) => &name[..pos],
        None => name,
    };
    let seq_part = without_ext.split('-').nth(2)?;
    seq_part.parse::<u32>().ok()
}

fn next_sequences(existing: &[HistoryFileRef]) -> [u32; HISTORY_TIER_COUNT] {
    let mut next = [0u32; HISTORY_TIER_COUNT];
    for file_ref in existing {
        let tier_index = file_ref.tier as usize;
        if tier_index < HISTORY_TIER_COUNT && file_ref.sequence + 1 > next[tier_index] {
            next[tier_index] = file_ref.sequence + 1;
        }
    }
    next
}

fn push_bundle_entry(output: &mut Vec<u8>, name: &str, data: &[u8]) {
    let name_bytes = name.as_bytes();
    output.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    output.extend_from_slice(name_bytes);
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(data);
}

pub fn level_set_file_name() -> String {
    format!("{}.{}", LEVEL_SET_ASSET_NAME, ASSET_BIN_EXT)
}

pub fn bundled_level_file_name(pos: IVec2) -> String {
    format!("{}_{}.{}", pos.x, pos.y, ASSET_BIN_EXT)
}

pub fn head_state_prev_name() -> String {
    format!("{}{}", HISTORY_STATE_NAME, HISTORY_STATE_PREV_SUFFIX)
}

pub fn encode_bundled_level(level: &Level) -> Vec<u8> {
    let mut bytes = Vec::new();
    DATA_VERSION.ser_bin(&mut bytes);
    level.ser_bin(&mut bytes);
    bytes
}

pub fn encode_bundled_level_set(level_set: &LevelSet) -> Vec<u8> {
    let mut bytes = Vec::new();
    DATA_VERSION.ser_bin(&mut bytes);
    level_set.ser_bin(&mut bytes);
    bytes
}

pub fn build_bundle_container(
    levels: &HashMap<IVec2, Level>,
    level_set: &Option<LevelSet>,
) -> Vec<u8> {
    let mut file_count = levels.len() as u32;
    if level_set.is_some() {
        file_count += 1;
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&file_count.to_le_bytes());
    for (pos, level) in levels {
        let name = bundled_level_file_name(*pos);
        let data = encode_bundled_level(level);
        push_bundle_entry(&mut bytes, &name, &data);
    }
    if let Some(level_set) = level_set {
        let data = encode_bundled_level_set(level_set);
        let level_set_name = level_set_file_name();
        push_bundle_entry(&mut bytes, &level_set_name, &data);
    }
    bytes
}

pub fn parse_bundle_container(bytes: &[u8]) -> Option<Vec<(&str, &[u8])>> {
    let mut offset = 0;
    let file_count = read_u32_at(bytes, &mut offset)? as usize;
    if file_count > (bytes.len() - offset) / 6 {
        return None;
    }
    let mut entries = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let name_len = read_u16_at(bytes, &mut offset)? as usize;
        if name_len > bytes.len() - offset {
            return None;
        }
        let name = match std::str::from_utf8(&bytes[offset..offset + name_len]) {
            Ok(name) => name,
            Err(_) => return None,
        };
        offset += name_len;
        let data_len = read_u32_at(bytes, &mut offset)? as usize;
        if data_len > bytes.len() - offset {
            return None;
        }
        let data = &bytes[offset..offset + data_len];
        offset += data_len;
        entries.push((name, data));
    }
    Some(entries)
}

pub fn decode_bundled_level(
    name: &str,
    bytes: &[u8],
    debug_log: &mut DebugLog,
) -> Option<(IVec2, Level)> {
    let stem = name.strip_suffix(ASSET_BIN_EXT)?.strip_suffix('.')?;
    let mut parts = stem.split('_');
    let x_part = parts.next()?;
    let y_part = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let x = x_part.parse::<i32>().ok()?;
    let y = y_part.parse::<i32>().ok()?;
    let pos = IVec2::new(x, y);
    let level = decode_bundled_level_bytes(bytes, debug_log)?;
    Some((pos, level))
}

pub fn decode_bundled_level_bytes(bytes: &[u8], debug_log: &mut DebugLog) -> Option<Level> {
    let mut offset = 0;
    let version = usize::de_bin(&mut offset, bytes).ok()?;
    Level::migrate_from(version, &mut offset, bytes, "", debug_log)
}

pub fn decode_bundled_level_set(bytes: &[u8], debug_log: &mut DebugLog) -> Option<LevelSet> {
    let mut offset = 0;
    let version = usize::de_bin(&mut offset, bytes).ok()?;
    let level_set_name = level_set_file_name();
    LevelSet::migrate_from(version, &mut offset, bytes, &level_set_name, debug_log)
}

pub fn build_republish_container(chunks: &[Chunk]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for chunk in chunks {
        bytes.extend_from_slice(&(chunk.bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&chunk.bytes);
    }
    bytes
}

pub struct BootProbe {
    pub has_save: bool,
    pub manifest_bytes: Vec<u8>,
    pub sectors_bytes: Vec<u8>,
    pub head_bytes: Vec<u8>,
    pub head_prev_bytes: Vec<u8>,
    pub bundle_bytes: Vec<u8>,
    pub files: Vec<HistoryFileRef>,
    pub snapshots_bytes: Vec<u8>,
    pub snapshots_prev_bytes: Vec<u8>,
}

fn read_section(bytes: &[u8], offset: &mut usize) -> Option<Vec<u8>> {
    Some(read_section_slice(bytes, offset)?.to_vec())
}

fn read_section_slice<'a>(bytes: &'a [u8], offset: &mut usize) -> Option<&'a [u8]> {
    let len = read_u32_at(bytes, offset)? as usize;
    if len > bytes.len() - *offset {
        return None;
    }
    let section = &bytes[*offset..*offset + len];
    *offset += len;
    Some(section)
}

pub fn parse_boot_container(bytes: &[u8]) -> Option<BootProbe> {
    if bytes.is_empty() {
        return None;
    }
    let has_save = bytes[0] == 1;
    if !has_save {
        return Some(BootProbe {
            has_save: false,
            manifest_bytes: Vec::new(),
            sectors_bytes: Vec::new(),
            head_bytes: Vec::new(),
            head_prev_bytes: Vec::new(),
            bundle_bytes: Vec::new(),
            files: Vec::new(),
            snapshots_bytes: Vec::new(),
            snapshots_prev_bytes: Vec::new(),
        });
    }
    let mut offset = 1;
    let manifest_bytes = read_section(bytes, &mut offset)?;
    let sectors_bytes = read_section(bytes, &mut offset)?;
    let head_bytes = read_section(bytes, &mut offset)?;
    let head_prev_bytes = read_section(bytes, &mut offset)?;
    let bundle_bytes = read_section(bytes, &mut offset)?;
    let files_section = read_section_slice(bytes, &mut offset)?;
    let snapshots_bytes = read_section(bytes, &mut offset)?;
    let snapshots_prev_bytes = read_section(bytes, &mut offset)?;
    Some(BootProbe {
        has_save: true,
        manifest_bytes,
        sectors_bytes,
        head_bytes,
        head_prev_bytes,
        bundle_bytes,
        files: parse_files_container(files_section)?,
        snapshots_bytes,
        snapshots_prev_bytes,
    })
}

pub fn parse_files_container(bytes: &[u8]) -> Option<Vec<HistoryFileRef>> {
    let mut offset = 0;
    let Some(file_count) = read_u32_at(bytes, &mut offset) else {
        return if bytes.is_empty() {
            Some(Vec::new())
        } else {
            None
        };
    };
    let file_count = file_count as usize;
    let min_entry_bytes = 2 + 4 + HISTORY_FILE_HEADER_BYTES;
    if file_count > (bytes.len() - offset) / min_entry_bytes {
        return None;
    }
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let Some(name_len) = read_u16_at(bytes, &mut offset) else {
            return None;
        };
        let name_len = name_len as usize;
        if name_len > bytes.len() - offset {
            return None;
        }
        let name = match std::str::from_utf8(&bytes[offset..offset + name_len]) {
            Ok(name) => name,
            Err(_) => return None,
        };
        offset += name_len;
        let Some(byte_length) = read_u32_at(bytes, &mut offset) else {
            return None;
        };
        if offset + HISTORY_FILE_HEADER_BYTES > bytes.len() {
            return None;
        }
        let mut header = [0u8; HISTORY_FILE_HEADER_BYTES];
        header.copy_from_slice(&bytes[offset..offset + HISTORY_FILE_HEADER_BYTES]);
        offset += HISTORY_FILE_HEADER_BYTES;
        if let Some(file_ref) = history_file_ref_from_header(name, byte_length, &header) {
            files.push(file_ref);
        }
    }
    Some(files)
}
