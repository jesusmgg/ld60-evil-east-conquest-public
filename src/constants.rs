pub const WINDOW_TITLE: &str = "Evil East Conquest";

pub const SAVE_CLEARED_MESSAGE: &str =
    "Your save could not be loaded\nand was deleted.\n\nA new game has started.";

pub const TILE_SIZE_PX: f32 = 16.0;

pub const TILE_SIZE_X: f32 = TILE_SIZE_PX;
pub const TILE_SIZE_Y: f32 = TILE_SIZE_PX;

pub const VIEW_W: f32 = 640.0;
pub const VIEW_H: f32 = 360.0;

pub const CULL_MARGIN_PX: f32 = TILE_SIZE_PX * 2.0;

pub const CAMERA_TRANSITION_SPEED: f32 = 12.0;

pub const TILE_HIGHLIGHT_LERP_SPEED: f32 = 50.0;

pub const TURN_PERIOD: f32 = 0.4;

pub const DEFAULT_LEVEL_TILES_X: i32 = 40;
pub const DEFAULT_LEVEL_TILES_Y: i32 = 23;
pub const SECTOR_W_PX: f32 = DEFAULT_LEVEL_TILES_X as f32 * TILE_SIZE_PX;
pub const SECTOR_H_PX: f32 = DEFAULT_LEVEL_TILES_Y as f32 * TILE_SIZE_PX;
pub const HISTORY_TILES_PER_SECTOR: u32 = (DEFAULT_LEVEL_TILES_X * DEFAULT_LEVEL_TILES_Y) as u32;
const _: () = assert!(HISTORY_TILES_PER_SECTOR <= 1 << 14);

pub const HISTORY_TIER_COUNT: usize = 5;
pub const HISTORY_TIER_RATIO: [u32; HISTORY_TIER_COUNT] = [1, 10, 20, 40, 80];
pub const HISTORY_TIER_MERGE: [u32; HISTORY_TIER_COUNT] = [1, 10, 2, 2, 2];
pub const HISTORY_TIER_SPAN_TURNS: [u32; HISTORY_TIER_COUNT] =
    [18_000, 54_000, 126_000, 252_000, u32::MAX];

pub const HISTORY_CHUNK_BYTES: usize = 256 * 1024;
pub const HISTORY_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const HISTORY_FILE_HEADER_BYTES: usize = 8 + 7 * 4;
pub const HISTORY_CLOSED_RECORD_BYTES: usize = 5 * 4;
pub const HISTORY_FILE_TIER_OFFSET: usize = 12;
pub const HISTORY_FILE_TURN_FIRST_OFFSET: usize = 20;
pub const HISTORY_FILE_PATCH_OFFSET: usize = 24;
pub const HISTORY_CHUNK_TURN_FIRST_OFFSET: usize = 4;
pub const HISTORY_CHUNK_TURN_LAST_OFFSET: usize = 8;
pub const HISTORY_CHUNK_USED_BYTES_OFFSET: usize = 12;
pub const HISTORY_FILE_EXT: &str = "thf";
pub const HISTORY_STATE_NAME: &str = "head.ths";
pub const HISTORY_STATE_PREV_SUFFIX: &str = ".prev";
pub const HISTORY_MANIFEST_NAME: &str = "save.thm";
pub const HISTORY_MANIFEST_SAVE_ID_OFFSET: usize = 12;
pub const HISTORY_MANIFEST_BYTES: usize = 8 + 4 + 16 + 4 + 4;
pub const HISTORY_LEVELS_DIR_NAME: &str = "levels";
pub const SAVE_DIR_NAME: &str = "save";
pub const HISTORY_REFRESH_BUNDLE_FROM_ASSETS: bool = false;
pub const HISTORY_SECTORS_NAME: &str = "sectors.tht";
pub const HISTORY_SECTORS_MAGIC: [u8; 8] = *b"EVILECST";
pub const HISTORY_SECTOR_TABLE_HEADER_BYTES: usize = 8 + 4;
pub const HISTORY_FILE_MAGIC: [u8; 8] = *b"EVILECHF";
pub const HISTORY_STATE_MAGIC: [u8; 8] = *b"EVILECHS";
pub const HISTORY_SNAPSHOTS_NAME: &str = "snapshots.thn";
pub const HISTORY_SNAPSHOTS_MAGIC: [u8; 8] = *b"EVILECSN";
pub const HISTORY_MANIFEST_MAGIC: [u8; 8] = *b"EVILECHM";
pub const HISTORY_CHUNK_MAGIC: u32 = 0x4B48_4345;
pub const HISTORY_PREFETCH_FRAMES: u32 = 256;

pub const HISTORY_AUTO_CHECK_INTERVAL_TURNS: u32 = HISTORY_TIER_SPAN_TURNS[0] / 8;
pub const HISTORY_AUTO_CHECK_TURNS: u32 = 10;

const _: () = assert!(HISTORY_TIER_RATIO[0] == 1 && HISTORY_TIER_MERGE[0] == 1);
const _: () = assert!(HISTORY_TIER_RATIO[1] == HISTORY_TIER_RATIO[0] * HISTORY_TIER_MERGE[1]);
const _: () = assert!(HISTORY_TIER_RATIO[2] == HISTORY_TIER_RATIO[1] * HISTORY_TIER_MERGE[2]);
const _: () = assert!(HISTORY_TIER_RATIO[3] == HISTORY_TIER_RATIO[2] * HISTORY_TIER_MERGE[3]);
const _: () = assert!(HISTORY_TIER_RATIO[4] == HISTORY_TIER_RATIO[3] * HISTORY_TIER_MERGE[4]);
