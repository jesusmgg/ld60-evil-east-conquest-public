use crate::constants::{
    HISTORY_CHUNK_TURN_FIRST_OFFSET, HISTORY_CHUNK_TURN_LAST_OFFSET, HISTORY_CLOSED_RECORD_BYTES,
    HISTORY_FILE_BYTES, HISTORY_FILE_EXT, HISTORY_FILE_HEADER_BYTES, HISTORY_FILE_MAGIC,
    HISTORY_FILE_PATCH_OFFSET, HISTORY_FILE_TIER_OFFSET, HISTORY_FILE_TURN_FIRST_OFFSET,
    HISTORY_LEVELS_DIR_NAME, HISTORY_MANIFEST_BYTES, HISTORY_MANIFEST_NAME,
    HISTORY_MANIFEST_SAVE_ID_OFFSET, HISTORY_SECTORS_NAME, HISTORY_SNAPSHOTS_NAME,
    HISTORY_STATE_MAGIC, HISTORY_STATE_NAME, HISTORY_STATE_PREV_SUFFIX, HISTORY_TIER_COUNT,
    HISTORY_TIER_RATIO,
};

extern "C" {
    fn jam_ld60_old_tech_history_init(
        format_version: u32,
        tier_count: u32,
        file_bytes: u32,
        ratio_0: u32,
        ratio_1: u32,
        ratio_2: u32,
        ratio_3: u32,
        ratio_4: u32,
        magic_0: u32,
        magic_1: u32,
        magic_2: u32,
        magic_3: u32,
        magic_4: u32,
        magic_5: u32,
        magic_6: u32,
        magic_7: u32,
        state_magic_0: u32,
        state_magic_1: u32,
        state_magic_2: u32,
        state_magic_3: u32,
        state_magic_4: u32,
        state_magic_5: u32,
        state_magic_6: u32,
        state_magic_7: u32,
        manifest_name_ptr: *const u8,
        manifest_name_len: u32,
        sectors_name_ptr: *const u8,
        sectors_name_len: u32,
        state_name_ptr: *const u8,
        state_name_len: u32,
        levels_dir_name_ptr: *const u8,
        levels_dir_name_len: u32,
        file_ext_ptr: *const u8,
        file_ext_len: u32,
        file_header_bytes: u32,
        closed_record_bytes: u32,
        file_patch_offset: u32,
        state_prev_suffix_ptr: *const u8,
        state_prev_suffix_len: u32,
        manifest_save_id_offset: u32,
        file_tier_offset: u32,
        file_turn_first_offset: u32,
        chunk_turn_first_offset: u32,
        chunk_turn_last_offset: u32,
        manifest_size: u32,
        snapshots_name_ptr: *const u8,
        snapshots_name_len: u32,
    );
    fn jam_ld60_old_tech_history_ready() -> i32;
    fn jam_ld60_old_tech_history_clear_failed_save();
    fn jam_ld60_old_tech_history_reset_game();
    fn jam_ld60_old_tech_history_persistent() -> i32;
    fn jam_ld60_old_tech_history_write(tier: u32, ptr: *const u8, len: u32);
    fn jam_ld60_old_tech_history_write_head_state(ptr: *const u8, len: u32);
    fn jam_ld60_old_tech_history_write_snapshots(ptr: *const u8, len: u32);
    fn jam_ld60_old_tech_history_storage_free() -> f64;
    fn jam_ld60_old_tech_history_mark_head_good();
    fn jam_ld60_old_tech_history_write_bundle(ptr: *const u8, len: u32);
    fn jam_ld60_old_tech_history_seed_sequences(ptr: *const u8, count: u32);

    fn jam_ld60_old_tech_history_boot_len() -> u32;
    fn jam_ld60_old_tech_history_boot_copy(ptr: *mut u8);
    fn jam_ld60_old_tech_history_commit_fresh(
        manifest_ptr: *const u8,
        manifest_len: u32,
        sectors_ptr: *const u8,
        sectors_len: u32,
        bundle_ptr: *const u8,
        bundle_len: u32,
        head_ptr: *const u8,
        head_len: u32,
    );
    fn jam_ld60_old_tech_history_request_load(tier: u32, turn_first: u32, sequence: u32);
    fn jam_ld60_old_tech_history_load_state() -> i32;
    fn jam_ld60_old_tech_history_load_tier() -> u32;
    fn jam_ld60_old_tech_history_load_sequence() -> u32;
    fn jam_ld60_old_tech_history_load_len() -> u32;
    fn jam_ld60_old_tech_history_load_copy(ptr: *mut u8);
    fn jam_ld60_old_tech_history_delete(tier: u32, turn_first: u32, sequence: u32);
    fn jam_ld60_old_tech_history_republish(tier: u32, ptr: *const u8, len: u32);
    fn jam_ld60_old_tech_history_closed_next(ptr: *mut u8) -> i32;
}

pub struct WebHistoryProvider {
    pub is_initialized: bool,
    pub has_error: bool,
}

impl WebHistoryProvider {
    pub fn new() -> Self {
        Self {
            is_initialized: false,
            has_error: false,
        }
    }

    pub fn init(&mut self, format_version: u32) {
        unsafe {
            jam_ld60_old_tech_history_init(
                format_version,
                HISTORY_TIER_COUNT as u32,
                HISTORY_FILE_BYTES as u32,
                HISTORY_TIER_RATIO[0],
                HISTORY_TIER_RATIO[1],
                HISTORY_TIER_RATIO[2],
                HISTORY_TIER_RATIO[3],
                HISTORY_TIER_RATIO[4],
                HISTORY_FILE_MAGIC[0] as u32,
                HISTORY_FILE_MAGIC[1] as u32,
                HISTORY_FILE_MAGIC[2] as u32,
                HISTORY_FILE_MAGIC[3] as u32,
                HISTORY_FILE_MAGIC[4] as u32,
                HISTORY_FILE_MAGIC[5] as u32,
                HISTORY_FILE_MAGIC[6] as u32,
                HISTORY_FILE_MAGIC[7] as u32,
                HISTORY_STATE_MAGIC[0] as u32,
                HISTORY_STATE_MAGIC[1] as u32,
                HISTORY_STATE_MAGIC[2] as u32,
                HISTORY_STATE_MAGIC[3] as u32,
                HISTORY_STATE_MAGIC[4] as u32,
                HISTORY_STATE_MAGIC[5] as u32,
                HISTORY_STATE_MAGIC[6] as u32,
                HISTORY_STATE_MAGIC[7] as u32,
                HISTORY_MANIFEST_NAME.as_ptr(),
                HISTORY_MANIFEST_NAME.len() as u32,
                HISTORY_SECTORS_NAME.as_ptr(),
                HISTORY_SECTORS_NAME.len() as u32,
                HISTORY_STATE_NAME.as_ptr(),
                HISTORY_STATE_NAME.len() as u32,
                HISTORY_LEVELS_DIR_NAME.as_ptr(),
                HISTORY_LEVELS_DIR_NAME.len() as u32,
                HISTORY_FILE_EXT.as_ptr(),
                HISTORY_FILE_EXT.len() as u32,
                HISTORY_FILE_HEADER_BYTES as u32,
                HISTORY_CLOSED_RECORD_BYTES as u32,
                HISTORY_FILE_PATCH_OFFSET as u32,
                HISTORY_STATE_PREV_SUFFIX.as_ptr(),
                HISTORY_STATE_PREV_SUFFIX.len() as u32,
                HISTORY_MANIFEST_SAVE_ID_OFFSET as u32,
                HISTORY_FILE_TIER_OFFSET as u32,
                HISTORY_FILE_TURN_FIRST_OFFSET as u32,
                HISTORY_CHUNK_TURN_FIRST_OFFSET as u32,
                HISTORY_CHUNK_TURN_LAST_OFFSET as u32,
                HISTORY_MANIFEST_BYTES as u32,
                HISTORY_SNAPSHOTS_NAME.as_ptr(),
                HISTORY_SNAPSHOTS_NAME.len() as u32,
            );
        }
        self.is_initialized = true;
    }

    pub fn ready(&self) -> i32 {
        if !self.is_initialized {
            return 0;
        }
        unsafe { jam_ld60_old_tech_history_ready() }
    }

    pub fn clear_failed_save(&self) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_clear_failed_save();
        }
    }

    pub fn reset_game(&self) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_reset_game();
        }
    }

    pub fn persistent(&self) -> i32 {
        if !self.is_initialized {
            return -1;
        }
        unsafe { jam_ld60_old_tech_history_persistent() }
    }

    pub fn write_chunk(&self, tier: u8, bytes: &[u8]) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_write(tier as u32, bytes.as_ptr(), bytes.len() as u32);
        }
    }

    pub fn write_head_state(&self, bytes: &[u8]) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_write_head_state(bytes.as_ptr(), bytes.len() as u32);
        }
    }

    pub fn write_snapshots(&self, bytes: &[u8]) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_write_snapshots(bytes.as_ptr(), bytes.len() as u32);
        }
    }

    pub fn storage_free(&self) -> f64 {
        if !self.is_initialized {
            return -1.0;
        }
        unsafe { jam_ld60_old_tech_history_storage_free() }
    }

    pub fn mark_inherited_head_good(&self) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_mark_head_good();
        }
    }

    pub fn write_bundle(&self, bundle: &[u8]) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_write_bundle(bundle.as_ptr(), bundle.len() as u32);
        }
    }

    pub fn seed_sequences(&self, seeds: &[u32]) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_seed_sequences(
                seeds.as_ptr() as *const u8,
                seeds.len() as u32,
            );
        }
    }

    pub fn boot_len(&self) -> u32 {
        if !self.is_initialized {
            return 0;
        }
        unsafe { jam_ld60_old_tech_history_boot_len() }
    }

    pub fn boot_copy(&mut self, output: &mut Vec<u8>) {
        if !self.is_initialized {
            return;
        }
        let len = unsafe { jam_ld60_old_tech_history_boot_len() } as usize;
        output.resize(len, 0);
        unsafe { jam_ld60_old_tech_history_boot_copy(output.as_mut_ptr()) };
    }

    pub fn commit_fresh(&self, manifest: &[u8], sectors: &[u8], bundle: &[u8], head: &[u8]) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_commit_fresh(
                manifest.as_ptr(),
                manifest.len() as u32,
                sectors.as_ptr(),
                sectors.len() as u32,
                bundle.as_ptr(),
                bundle.len() as u32,
                head.as_ptr(),
                head.len() as u32,
            );
        }
    }

    pub fn request_load(&self, tier: u8, turn_first: u32, sequence: u32) {
        if !self.is_initialized {
            return;
        }
        unsafe { jam_ld60_old_tech_history_request_load(tier as u32, turn_first, sequence) };
    }

    pub fn load_state(&self) -> i32 {
        if !self.is_initialized {
            return 0;
        }
        unsafe { jam_ld60_old_tech_history_load_state() }
    }

    pub fn load_identity(&self) -> (u8, u32) {
        if !self.is_initialized {
            return (0, 0);
        }
        unsafe {
            (
                jam_ld60_old_tech_history_load_tier() as u8,
                jam_ld60_old_tech_history_load_sequence(),
            )
        }
    }

    pub fn load_copy(&mut self, output: &mut Vec<u8>) {
        if !self.is_initialized {
            return;
        }
        let len = unsafe { jam_ld60_old_tech_history_load_len() } as usize;
        output.resize(len, 0);
        unsafe { jam_ld60_old_tech_history_load_copy(output.as_mut_ptr()) };
    }

    pub fn delete_file(&self, tier: u8, turn_first: u32, sequence: u32) {
        if !self.is_initialized {
            return;
        }
        unsafe { jam_ld60_old_tech_history_delete(tier as u32, turn_first, sequence) };
    }

    pub fn republish(&self, tier: u8, container: &[u8]) {
        if !self.is_initialized {
            return;
        }
        unsafe {
            jam_ld60_old_tech_history_republish(
                tier as u32,
                container.as_ptr(),
                container.len() as u32,
            )
        };
    }

    pub fn closed_next(&mut self, record: &mut [u8; HISTORY_CLOSED_RECORD_BYTES]) -> bool {
        if !self.is_initialized {
            return false;
        }
        unsafe { jam_ld60_old_tech_history_closed_next(record.as_mut_ptr()) == 1 }
    }
}
