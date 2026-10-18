use macroquad::file::load_file;
use nanoserde::{DeBin, SerBin};
use std::fs;
use std::path::Path;

use crate::assets::asset_path::ASSETS_DIR;
use crate::debug::DebugLog;

pub const DATA_VERSION: usize = 11;
pub const OLDEST_READABLE_SAVE_VERSION: usize = 9;
pub const ASSET_BIN_EXT: &str = "asb";

pub fn is_readable_save_version(version: u32) -> bool {
    version >= OLDEST_READABLE_SAVE_VERSION as u32 && version <= DATA_VERSION as u32
}

pub trait Versioned: SerBin + DeBin + Sized {
    fn migrate_from(
        version: usize,
        offset: &mut usize,
        bytes: &[u8],
        path: &str,
        debug_log: &mut DebugLog,
    ) -> Option<Self> {
        if version == DATA_VERSION {
            Self::de_bin(offset, bytes).ok()
        } else {
            debug_log.warning(format!(
                "Failed to load '{}': {} version {} (current: {})",
                path,
                std::any::type_name::<Self>(),
                version,
                DATA_VERSION
            ));
            None
        }
    }
}

pub fn save_to_assets<T: SerBin>(data: &T, path: &str, debug_log: &mut DebugLog) -> bool {
    let full_path = format!("{}/{}", ASSETS_DIR, path);

    if let Some(parent) = Path::new(&full_path).parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            debug_log.warning(format!(
                "[GameData] Failed to create directory for {}: {}",
                full_path, e
            ));
            return false;
        }
    }

    let mut bytes = Vec::new();
    DATA_VERSION.ser_bin(&mut bytes);
    data.ser_bin(&mut bytes);

    if let Err(e) = fs::write(&full_path, &bytes) {
        debug_log.warning(format!("[GameData] Failed to save {}: {}", full_path, e));
        return false;
    }

    debug_log.log(format!("[GameData] Saved to {}", full_path));
    true
}

pub async fn load_from_assets<T: Versioned>(path: &str, debug_log: &mut DebugLog) -> Option<T> {
    let full_path = format!("{}/{}", ASSETS_DIR, path);

    let bytes = match load_file(&full_path).await {
        Ok(b) => b,
        Err(e) => {
            debug_log.log(format!("[GameData] No asset data at {}: {}", full_path, e));
            return None;
        }
    };

    let mut offset = 0;

    let version = match usize::de_bin(&mut offset, &bytes) {
        Ok(v) => v,
        Err(_) => {
            debug_log.warning(format!("[GameData] Failed to load {}", full_path));
            return None;
        }
    };

    let data = T::migrate_from(version, &mut offset, &bytes, path, debug_log)?;
    debug_log.log(format!("[GameData] Loaded from {}", full_path));
    Some(data)
}
