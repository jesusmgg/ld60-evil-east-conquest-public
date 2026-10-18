use crate::{
    assets::AssetManager,
    camera_manager::CameraManager,
    constants::*,
    debug::{DebugLog, DebugLogPtr},
    enums::Direction,
    game_data::{
        game_data::{load_from_assets, save_to_assets, ASSET_BIN_EXT},
        proxy::{
            HashMapIVec2InteractableProxy, HashMapIVec2StringProxy, HashMapIVec2TileTypeProxy,
            HashMapIVec2UnitSpawnDataProxy, IVec2Proxy,
        },
    },
    level::{
        background::{get_texture_for_background, Background},
        interactable::Gate,
        music_track::MusicTrack,
        tile::render_tile,
        Interactable, TileType,
    },
    lighting_manager::LIGHT_AMBIENT_LEVEL_DEFAULT,
    unit_manager::UnitSpawnData,
};
use macroquad::{
    experimental::coroutines::{start_coroutine, Coroutine},
    prelude::*,
};
use nanoserde::{DeBin, SerBin};
use std::collections::HashMap;

pub const LEVEL_SET_ASSET_NAME: &str = "level_set";
pub const LEVEL_DIR: &str = "levels";

pub fn sector_range_for_world_rect(world_rect: Rect) -> (IVec2, IVec2) {
    let min = IVec2::new(
        (world_rect.x / SECTOR_W_PX).floor() as i32,
        (world_rect.y / SECTOR_H_PX).floor() as i32,
    );
    let max = IVec2::new(
        (world_rect.right() / SECTOR_W_PX).floor() as i32,
        (world_rect.bottom() / SECTOR_H_PX).floor() as i32,
    );
    (min, max)
}

pub fn sector_in_range(pos: IVec2, min: IVec2, max: IVec2) -> bool {
    pos.x >= min.x && pos.x <= max.x && pos.y >= min.y && pos.y <= max.y
}

#[derive(Clone, SerBin, DeBin)]
pub struct Level {
    pub file_name: String,
    pub name: String,
    #[nserde(proxy = "IVec2Proxy")]
    pub grid_tiles: IVec2,
    #[nserde(proxy = "HashMapIVec2TileTypeProxy")]
    pub tile_layout: HashMap<IVec2, TileType>,
    #[nserde(proxy = "HashMapIVec2InteractableProxy")]
    pub interactables: HashMap<IVec2, Interactable>,
    #[nserde(proxy = "HashMapIVec2UnitSpawnDataProxy")]
    pub unit_spawns: HashMap<IVec2, UnitSpawnData>,
    pub ambient_light_level: f32,
    pub background_type: Background,
    pub music_track: MusicTrack,
}

impl Level {
    pub fn new(file_name: &str, name: &str, grid_tiles: IVec2) -> Self {
        let tile_layout = HashMap::new();
        let interactables = HashMap::new();
        let unit_spawns = HashMap::new();

        Self {
            file_name: file_name.to_string(),
            name: name.to_string(),
            grid_tiles,
            tile_layout,
            interactables,
            unit_spawns,
            ambient_light_level: LIGHT_AMBIENT_LEVEL_DEFAULT,
            background_type: Background::Default,
            music_track: MusicTrack::BattleMain,
        }
    }

    pub fn gate_at(&self, tile: IVec2) -> Option<&Gate> {
        let Some(Interactable::Gate(gate)) = self.interactables.get(&tile) else {
            return None;
        };
        Some(gate)
    }

    pub fn pos_world(grid_pos: IVec2) -> Vec2 {
        Vec2::new(
            grid_pos.x as f32 * SECTOR_W_PX,
            grid_pos.y as f32 * SECTOR_H_PX,
        )
    }

    pub fn grid_size_px(&self) -> Vec2 {
        Vec2::new(
            TILE_SIZE_X * self.grid_tiles.x as f32,
            TILE_SIZE_Y * self.grid_tiles.y as f32,
        )
    }

    pub fn grid_offset(&self) -> Vec2 {
        let grid_size_px = self.grid_size_px();

        Vec2::new(
            (SECTOR_W_PX - grid_size_px.x) / 2.0,
            (SECTOR_H_PX - grid_size_px.y) / 2.0,
        )
    }

    pub fn grid_origin(&self, grid_pos: IVec2) -> Vec2 {
        Level::pos_world(grid_pos) + self.grid_offset()
    }

    pub fn tile_to_world_pos(&self, grid_pos: IVec2, tile_pos: IVec2) -> Vec2 {
        let grid_origin = self.grid_origin(grid_pos);
        Vec2::new(
            grid_origin.x + (tile_pos.x as f32 * TILE_SIZE_X),
            grid_origin.y + (tile_pos.y as f32 * TILE_SIZE_Y),
        )
    }

    pub fn world_to_tile_pos(&self, grid_pos: IVec2, world_pos: Vec2) -> Option<IVec2> {
        let grid_origin = self.grid_origin(grid_pos);
        let relative_x = world_pos.x - grid_origin.x;
        let relative_y = world_pos.y - grid_origin.y;

        let tile_x = (relative_x / TILE_SIZE_X).floor() as i32;
        let tile_y = (relative_y / TILE_SIZE_Y).floor() as i32;

        if tile_x >= 0 && tile_x < self.grid_tiles.x && tile_y >= 0 && tile_y < self.grid_tiles.y {
            Some(IVec2::new(tile_x, tile_y))
        } else {
            None
        }
    }

    pub fn render(&self, grid_pos: IVec2, assets: &AssetManager) {
        let background_texture = get_texture_for_background(assets, self.background_type);
        let pos_world = Level::pos_world(grid_pos);
        draw_texture(background_texture, pos_world.x, pos_world.y, WHITE);

        for (tile_pos, tile_type) in &self.tile_layout {
            let world_pos = self.tile_to_world_pos(grid_pos, *tile_pos);
            render_tile(assets, *tile_type, world_pos, WHITE);
        }

        for (interactable_pos, interactable) in &self.interactables {
            interactable.render(*interactable_pos, grid_pos, self, assets);
        }
    }
}

#[derive(SerBin, DeBin)]
pub struct LevelSet {
    #[nserde(proxy = "HashMapIVec2StringProxy")]
    pub level_grid: HashMap<IVec2, String>,
    pub initial_level_name: String,
}

pub struct LevelManager {
    pub level_set: Option<LevelSet>,
    pub levels: HashMap<IVec2, Level>,
    pub view_sector_pos: Option<IVec2>,

    level_set_coroutine: Option<Coroutine<Option<LevelSet>>>,
    load_coroutines: Vec<(IVec2, Coroutine<Option<Level>>)>,
    pub loading_complete: bool,
    pub is_initialized: bool,
}

impl LevelManager {
    pub fn first_sector_pos(&self) -> Option<IVec2> {
        let mut best: Option<IVec2> = None;
        for &pos in self.levels.keys() {
            let better = match best {
                None => true,
                Some(b) => (pos.y, pos.x) < (b.y, b.x),
            };
            if better {
                best = Some(pos);
            }
        }
        best
    }

    pub fn new() -> Self {
        let levels = HashMap::new();
        let view_sector_pos = None;

        Self {
            level_set: None,
            levels,
            view_sector_pos,
            level_set_coroutine: None,
            load_coroutines: Vec::new(),
            loading_complete: true,
            is_initialized: false,
        }
    }

    fn poll_level_coroutines(&mut self, debug_log: &mut DebugLog) -> bool {
        if self.load_coroutines.is_empty() {
            return true;
        }

        let mut remaining_coroutines = Vec::new();

        for (level_pos, coroutine) in self.load_coroutines.drain(..) {
            if !coroutine.is_done() {
                remaining_coroutines.push((level_pos, coroutine));
                continue;
            }
            let Some(loaded_level_opt) = coroutine.retrieve() else {
                continue;
            };
            let Some(loaded_level) = loaded_level_opt else {
                debug_log.warning(format!(
                    "[LevelManager] Skipping level at {:?} (failed to load)",
                    level_pos
                ));
                continue;
            };
            self.levels.insert(level_pos, loaded_level);
        }

        self.load_coroutines = remaining_coroutines;

        self.load_coroutines.is_empty()
    }

    fn update_init(&mut self, debug_log: &mut DebugLog) {
        if self.is_initialized {
            return;
        }

        if self.loading_complete {
            return;
        }

        if let Some(coroutine) = self.level_set_coroutine.take() {
            if coroutine.is_done() {
                if let Some(loaded_level_set_opt) = coroutine.retrieve() {
                    if let Some(loaded_level_set) = loaded_level_set_opt {
                        debug_log.log(format!(
                            "[LevelManager] Loaded level_set - initial level: {}",
                            loaded_level_set.initial_level_name
                        ));

                        for (level_pos, file_name) in &loaded_level_set.level_grid {
                            self.spawn_level_load_coroutine(
                                *level_pos,
                                file_name,
                                DebugLogPtr::new(debug_log),
                            );
                        }

                        self.level_set = Some(loaded_level_set);
                    } else {
                        self.ensure_level_set(debug_log);
                        self.create_initial_level(debug_log);
                    }
                } else {
                    self.ensure_level_set(debug_log);
                    self.create_initial_level(debug_log);
                }
            } else {
                self.level_set_coroutine = Some(coroutine);
            }
            return;
        }

        let all_complete = self.poll_level_coroutines(debug_log);

        if all_complete {
            self.loading_complete = true;
            self.is_initialized = true;

            if let Some(pos) = self.initial_level_pos() {
                self.view_sector_pos = Some(pos);
            }

            if self.view_sector_pos.is_none() && !self.levels.is_empty() {
                self.view_sector_pos = self.first_sector_pos();
                debug_log.warning(String::from(
                    "[LevelManager] Initial level not found, using first available level",
                ));
            }
        }
    }

    pub fn initial_level_pos(&self) -> Option<IVec2> {
        let Some(ref level_set) = self.level_set else {
            return None;
        };
        for (pos, level) in &self.levels {
            if level.name == level_set.initial_level_name {
                return Some(*pos);
            }
        }
        None
    }

    pub fn update(&mut self, debug_log: &mut DebugLog) {
        self.update_init(debug_log);

        if self.is_initialized {
            self.poll_level_coroutines(debug_log);
        }
    }

    pub fn render(&self, assets: &AssetManager, camera_manager: &mut CameraManager) {
        if let Some(active_pos) = self.view_sector_pos {
            let pos_world = Level::pos_world(active_pos);

            camera_manager.target_pos = f32::vec2(
                pos_world.x + SECTOR_W_PX / 2.0,
                pos_world.y + SECTOR_H_PX / 2.0,
            );

            let view_rect = camera_manager.world_view_rect(CULL_MARGIN_PX);
            let (sector_min, sector_max) = sector_range_for_world_rect(view_rect);

            for gy in sector_min.y..=sector_max.y {
                for gx in sector_min.x..=sector_max.x {
                    let level_pos = IVec2::new(gx, gy);

                    if let Some(level) = self.levels.get(&level_pos) {
                        level.render(level_pos, assets);
                    }
                }
            }
        }
    }

    pub fn current_level(&self) -> Option<&Level> {
        let Some(pos) = self.view_sector_pos else {
            return None;
        };
        self.levels.get(&pos)
    }

    pub fn get_level_at_pos(&self, pos: IVec2) -> Option<&Level> {
        self.levels.get(&pos)
    }

    pub fn get_level_at_pos_mut(&mut self, pos: IVec2) -> Option<&mut Level> {
        self.levels.get_mut(&pos)
    }

    pub fn get_gate(&self, level_pos: IVec2, tile: IVec2) -> Option<&Gate> {
        self.get_level_at_pos(level_pos)?.gate_at(tile)
    }

    pub fn tile_under_mouse(&self, camera_manager: &CameraManager) -> Option<IVec2> {
        let view_sector = self.view_sector_pos?;
        let level = self.get_level_at_pos(view_sector)?;
        level.world_to_tile_pos(view_sector, camera_manager.mouse_world_pos)
    }

    pub fn gate_arrival(&self, level_pos: IVec2, tile: IVec2) -> Option<(IVec2, IVec2)> {
        let level = self.get_level_at_pos(level_pos)?;
        level.gate_at(tile)?;
        let offset = if tile.x == 0 {
            IVec2::new(-1, 0)
        } else if tile.x == level.grid_tiles.x - 1 {
            IVec2::new(1, 0)
        } else if tile.y == 0 {
            IVec2::new(0, -1)
        } else if tile.y == level.grid_tiles.y - 1 {
            IVec2::new(0, 1)
        } else {
            return None;
        };
        let (destination, arrival_tile, _) =
            self.calculate_level_transition(level_pos, tile + offset)?;
        let is_across = if offset.x != 0 {
            arrival_tile.y == tile.y
        } else {
            arrival_tile.x == tile.x
        };
        if !is_across {
            return None;
        }
        self.get_gate(destination, arrival_tile)?;
        Some((destination, arrival_tile))
    }

    pub fn tile_to_world(&self, level_pos: IVec2, tile_pos: IVec2) -> Vec2 {
        if let Some(level) = self.get_level_at_pos(level_pos) {
            level.tile_to_world_pos(level_pos, tile_pos)
        } else {
            Vec2::ZERO
        }
    }

    pub fn calculate_level_transition(
        &self,
        current_level_pos: IVec2,
        exit_tile_pos: IVec2,
    ) -> Option<(IVec2, IVec2, Direction)> {
        let current_level = self.get_level_at_pos(current_level_pos)?;
        let grid_tiles = current_level.grid_tiles;

        let (neighbor_offset, entry_direction) = if exit_tile_pos.x < 0 {
            (IVec2::new(-1, 0), Direction::Left)
        } else if exit_tile_pos.x >= grid_tiles.x {
            (IVec2::new(1, 0), Direction::Right)
        } else if exit_tile_pos.y < 0 {
            (IVec2::new(0, -1), Direction::Up)
        } else if exit_tile_pos.y >= grid_tiles.y {
            (IVec2::new(0, 1), Direction::Down)
        } else {
            return None;
        };

        let target_level_pos = current_level_pos + neighbor_offset;

        let neighbor_level = self.get_level_at_pos(target_level_pos)?;

        let entry_tile_pos = match entry_direction {
            Direction::Left => {
                let clamped_y = exit_tile_pos.y.clamp(0, neighbor_level.grid_tiles.y - 1);
                IVec2::new(neighbor_level.grid_tiles.x - 1, clamped_y)
            }
            Direction::Right => {
                let clamped_y = exit_tile_pos.y.clamp(0, neighbor_level.grid_tiles.y - 1);
                IVec2::new(0, clamped_y)
            }
            Direction::Up => {
                let clamped_x = exit_tile_pos.x.clamp(0, neighbor_level.grid_tiles.x - 1);
                IVec2::new(clamped_x, neighbor_level.grid_tiles.y - 1)
            }
            Direction::Down => {
                let clamped_x = exit_tile_pos.x.clamp(0, neighbor_level.grid_tiles.x - 1);
                IVec2::new(clamped_x, 0)
            }
        };

        Some((target_level_pos, entry_tile_pos, entry_direction))
    }

    pub fn level_file_path(level: &Level) -> String {
        Self::level_file_path_from_name(&level.file_name)
    }

    pub fn level_file_path_from_name(file_name: &str) -> String {
        format!("{}/{}.{}", LEVEL_DIR, file_name, ASSET_BIN_EXT)
    }

    fn spawn_level_load_coroutine(
        &mut self,
        level_pos: IVec2,
        file_name: &str,
        debug_log: DebugLogPtr,
    ) {
        let path = Self::level_file_path_from_name(file_name);
        let handle = start_coroutine(async move {
            let debug_log = unsafe { debug_log.as_mut() };
            load_from_assets::<Level>(&path, debug_log).await
        });
        self.load_coroutines.push((level_pos, handle));
    }

    pub fn level_set_file_path() -> String {
        format!("{}/{}.{}", LEVEL_DIR, LEVEL_SET_ASSET_NAME, ASSET_BIN_EXT)
    }

    pub fn load_single_level_async(
        &mut self,
        level_pos: IVec2,
        file_name: &str,
        debug_log: &mut DebugLog,
    ) {
        self.load_coroutines.clear();
        self.loading_complete = false;

        self.spawn_level_load_coroutine(level_pos, file_name, DebugLogPtr::new(debug_log));
        debug_log.log(format!("[LevelManager] Loading level {}...", file_name));
    }

    pub fn load_all_levels_async(&mut self, debug_log: &mut DebugLog) {
        self.load_coroutines.clear();
        self.loading_complete = false;

        let level_set_path = Self::level_set_file_path();
        let debug_log_ptr = DebugLogPtr::new(debug_log);
        let handle = start_coroutine(async move {
            let debug_log = unsafe { debug_log_ptr.as_mut() };
            load_from_assets::<LevelSet>(&level_set_path, debug_log).await
        });

        self.level_set_coroutine = Some(handle);

        debug_log.log(String::from(
            "[LevelManager] Loading level_set from disk...",
        ));
    }

    fn save_authored_level(level: &Level, path: &str, debug_log: &mut DebugLog) -> bool {
        let mut combat_rubble = Vec::new();
        for (tile, interactable) in &level.interactables {
            if let Interactable::Rubble(rubble) = interactable {
                if rubble.is_from_combat {
                    combat_rubble.push(*tile);
                }
            }
        }
        if combat_rubble.is_empty() {
            return save_to_assets(level, path, debug_log);
        }
        let mut authored = level.clone();
        for tile in combat_rubble {
            authored.interactables.remove(&tile);
        }
        save_to_assets(&authored, path, debug_log)
    }

    pub fn save_single_level(&self, level_pos: IVec2, debug_log: &mut DebugLog) -> bool {
        if let Some(level) = self.levels.get(&level_pos) {
            let path = Self::level_file_path(level);

            if Self::save_authored_level(level, &path, debug_log) {
                debug_log.log(format!("[LevelManager] Saved {}", level.name));
                true
            } else {
                false
            }
        } else {
            debug_log.warning(format!(
                "[LevelManager] Invalid level position {:?} for single save",
                level_pos
            ));
            false
        }
    }

    pub fn save_all_levels(&mut self, debug_log: &mut DebugLog) -> bool {
        let mut all_success = true;

        if !self.save_level_set(debug_log) {
            all_success = false;
        }

        for level in self.levels.values() {
            let path = Self::level_file_path(level);
            if !Self::save_authored_level(level, &path, debug_log) {
                all_success = false;
            }
        }

        if all_success {
            debug_log.log(String::from(
                "[LevelManager] Saved all levels and level_set",
            ));
        } else {
            debug_log.warning(String::from("[LevelManager] Some levels failed to save"));
        }

        all_success
    }

    pub fn save_level_set(&mut self, debug_log: &mut DebugLog) -> bool {
        let mut level_grid = HashMap::new();
        for (pos, level) in &self.levels {
            level_grid.insert(*pos, level.file_name.clone());
        }

        let initial_level_name = if let Some(ref level_set) = self.level_set {
            level_set.initial_level_name.clone()
        } else {
            let mut name = String::from("level00");
            if let Some(pos) = self.first_sector_pos() {
                if let Some(level) = self.levels.get(&pos) {
                    name = level.name.clone();
                }
            }
            name
        };

        let level_set = LevelSet {
            level_grid,
            initial_level_name,
        };

        let level_set_path = Self::level_set_file_path();
        let success = save_to_assets(&level_set, &level_set_path, debug_log);

        if success {
            self.level_set = Some(level_set);
            debug_log.log(String::from("[LevelManager] Saved level_set"));
        } else {
            debug_log.warning(String::from("[LevelManager] Failed to save level_set"));
        }

        success
    }

    pub fn init(&mut self, debug_log: &mut DebugLog) {
        self.load_all_levels_async(debug_log);
    }

    pub fn ensure_level_set(&mut self, debug_log: &mut DebugLog) {
        if self.level_set.is_none() {
            self.level_set = Some(LevelSet {
                level_grid: HashMap::new(),
                initial_level_name: String::new(),
            });
            debug_log.log(String::from("[LevelManager] Created new level_set"));
        }
    }

    pub fn create_level_at(&mut self, pos: IVec2, debug_log: &mut DebugLog) {
        if self.levels.contains_key(&pos) {
            return;
        }

        let file_name = format!("level{}{}", pos.x, pos.y);
        let name = format!("Level {}-{}", pos.x + 1, pos.y + 1);

        let level = Level::new(
            &file_name,
            &name,
            IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
        );
        self.levels.insert(pos, level);

        self.ensure_level_set(debug_log);
        if let Some(ref mut level_set) = self.level_set {
            level_set.level_grid.insert(pos, file_name.clone());
        }

        debug_log.log(format!(
            "Created level '{}' at ({}, {})",
            file_name, pos.x, pos.y
        ));
    }

    fn create_initial_level(&mut self, debug_log: &mut DebugLog) {
        let pos = IVec2::new(0, 0);
        self.create_level_at(pos, debug_log);

        if let Some(ref mut level_set) = self.level_set {
            if let Some(level) = self.levels.get(&pos) {
                level_set.initial_level_name = level.name.clone();
            }
        }
        self.view_sector_pos = Some(pos);
    }
}
