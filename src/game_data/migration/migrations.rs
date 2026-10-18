use crate::{
    deser_proxy,
    enums::Direction,
    game_data::proxy::{
        HashMapIVec2InteractableProxy, HashMapIVec2TileTypeProxy, HashMapIVec2UnitSpawnDataProxy,
        IVec2Proxy, VecIVec2Proxy,
    },
    impl_versioned_migration,
    level::{
        background::Background,
        interactable::{Gate, Interactable, LightSource, Rubble},
        level_manager::{Level, LevelSet},
        music_track::MusicTrack,
        tile::TileType,
    },
    unit_design::{default_profile, Reward, StructureSprite, UnitDesign},
    unit_manager::{UnitOwner, UnitSpawnData, UnitType},
};
use macroquad::math::IVec2;
use nanoserde::DeBin;
use std::collections::HashMap;

const MAIN_PATH_LEVELS: [&str; 8] = [
    "level14", "level24", "level34", "level44", "level54", "level64", "level74", "level84",
];

const OLD_BACKGROUND_DEFAULT: u16 = 0;
const OLD_BACKGROUND_DEFAULT_SCALED: u16 = 1;

const OLD_INTERACTABLE_GAS_CAN: u16 = 100;
const OLD_INTERACTABLE_LIGHT_SOURCE: u16 = 300;
const OLD_INTERACTABLE_RUBBLE: u16 = 500;
const OLD_INTERACTABLE_GATE: u16 = 600;

const OLD_TILE_BLOCKERS: [u16; 18] = [
    0, 10, 20, 30, 40, 50, 100, 110, 120, 130, 200, 210, 220, 230, 240, 300, 310, 410,
];
const OLD_TILE_ITEM_FLOOR: u16 = 430;
const OLD_TILE_GATE: u16 = 440;

const OLD_UNIT_TYPE_SOLDIER: u16 = 0;
const OLD_UNIT_TYPE_MACHINE: u16 = 10;
const OLD_UNIT_TYPE_WALL: u16 = 100;
const OLD_UNIT_TYPE_TOWER: u16 = 110;
const OLD_UNIT_TYPE_JAIL: u16 = 120;
const OLD_UNIT_TYPE_UNIVERSITY: u16 = 130;
const OLD_UNIT_TYPE_STOREHOUSE: u16 = 140;
const OLD_UNIT_TYPE_FORTRESS: u16 = 150;

fn old_unit_type(discriminant: u16) -> Option<(UnitType, StructureSprite)> {
    match discriminant {
        OLD_UNIT_TYPE_SOLDIER => Some((UnitType::Soldier, StructureSprite::WallWood)),
        OLD_UNIT_TYPE_MACHINE => Some((UnitType::Machine, StructureSprite::WallWood)),
        OLD_UNIT_TYPE_WALL => Some((UnitType::Structure, StructureSprite::WallWood)),
        OLD_UNIT_TYPE_TOWER => Some((UnitType::Structure, StructureSprite::TowerWood)),
        OLD_UNIT_TYPE_JAIL => Some((UnitType::Structure, StructureSprite::Jail)),
        OLD_UNIT_TYPE_UNIVERSITY => Some((UnitType::Structure, StructureSprite::University)),
        OLD_UNIT_TYPE_STOREHOUSE => Some((UnitType::Structure, StructureSprite::Storehouse)),
        OLD_UNIT_TYPE_FORTRESS => Some((UnitType::Structure, StructureSprite::Fortress)),
        _ => None,
    }
}

fn deserialize_old_unit_spawns(
    version: usize,
    offset: &mut usize,
    bytes: &[u8],
) -> Option<HashMap<IVec2, UnitSpawnData>> {
    let len = usize::de_bin(offset, bytes).ok()?;
    let mut unit_spawns = HashMap::new();
    for _ in 0..len {
        let key: IVec2 = IVec2Proxy::de_bin(offset, bytes).ok()?.into();
        let owner = UnitOwner::de_bin(offset, bytes).ok()?;
        let (unit_type, structure_sprite) = old_unit_type(u16::de_bin(offset, bytes).ok()?)?;
        let tile_pos = deser_proxy!(IVec2Proxy => IVec2, offset, bytes);
        let direction = Direction::de_bin(offset, bytes).ok()?;
        let mut profile = default_profile(unit_type, structure_sprite);
        let mut waypoints = Vec::new();
        match version {
            1 => {
                u8::de_bin(offset, bytes).ok()?;
            }
            2 => {
                profile.design = UnitDesign::de_bin(offset, bytes).ok()?;
                profile.aggro_range = u8::de_bin(offset, bytes).ok()?;
                profile.reward = Reward::de_bin(offset, bytes).ok()?;
                profile.is_final_fortress = bool::de_bin(offset, bytes).ok()?;
                waypoints = deser_proxy!(VecIVec2Proxy => Vec<IVec2>, offset, bytes);
            }
            _ => return None,
        }
        unit_spawns.insert(
            key,
            UnitSpawnData {
                owner,
                unit_type,
                tile_pos,
                direction,
                profile,
                waypoints,
            },
        );
    }
    Some(unit_spawns)
}

fn deserialize_old_interactables(
    offset: &mut usize,
    bytes: &[u8],
) -> Option<HashMap<IVec2, Interactable>> {
    let len = usize::de_bin(offset, bytes).ok()?;
    let mut interactables = HashMap::new();
    for _ in 0..len {
        let key: IVec2 = IVec2Proxy::de_bin(offset, bytes).ok()?.into();
        match u16::de_bin(offset, bytes).ok()? {
            OLD_INTERACTABLE_GAS_CAN => {}
            OLD_INTERACTABLE_LIGHT_SOURCE => {
                let light = LightSource::de_bin(offset, bytes).ok()?;
                interactables.insert(key, Interactable::LightSource(light));
            }
            _ => return None,
        }
    }
    Some(interactables)
}

fn deserialize_old_gate_interactables(
    offset: &mut usize,
    bytes: &[u8],
) -> Option<HashMap<IVec2, Interactable>> {
    let len = usize::de_bin(offset, bytes).ok()?;
    let mut interactables = HashMap::new();
    for _ in 0..len {
        let key: IVec2 = IVec2Proxy::de_bin(offset, bytes).ok()?.into();
        let interactable = match u16::de_bin(offset, bytes).ok()? {
            OLD_INTERACTABLE_LIGHT_SOURCE => {
                Interactable::LightSource(LightSource::de_bin(offset, bytes).ok()?)
            }
            OLD_INTERACTABLE_RUBBLE => Interactable::Rubble(Rubble::de_bin(offset, bytes).ok()?),
            OLD_INTERACTABLE_GATE => {
                let mut gate = Gate::default();
                gate.direction = Direction::de_bin(offset, bytes).ok()?;
                Interactable::Gate(gate)
            }
            _ => return None,
        };
        interactables.insert(key, interactable);
    }
    Some(interactables)
}

fn old_level_transfer_area(
    version: usize,
    mut offset: usize,
    bytes: &[u8],
    grid_tiles: IVec2,
) -> Option<(IVec2, IVec2)> {
    match version {
        1 => return Some((IVec2::ZERO, grid_tiles)),
        2 => {
            deserialize_old_unit_spawns(version, &mut offset, bytes)?;
        }
        3..=7 => {
            HashMapIVec2UnitSpawnDataProxy::de_bin(&mut offset, bytes).ok()?;
        }
        _ => return None,
    }
    f32::de_bin(&mut offset, bytes).ok()?;
    deserialize_old_background(&mut offset, bytes)?;
    let area_tile = deser_proxy!(IVec2Proxy => IVec2, &mut offset, bytes);
    let area_size = deser_proxy!(IVec2Proxy => IVec2, &mut offset, bytes);
    Some((area_tile, area_size))
}

fn with_gate_transfer_areas(
    mut interactables: HashMap<IVec2, Interactable>,
    area: (IVec2, IVec2),
) -> HashMap<IVec2, Interactable> {
    for interactable in interactables.values_mut() {
        if let Interactable::Gate(gate) = interactable {
            gate.transfer_area_tile = area.0;
            gate.transfer_area_size = area.1;
        }
    }
    interactables
}

fn skip_old_transfer_area(offset: &mut usize, bytes: &[u8]) -> Option<()> {
    IVec2Proxy::de_bin(offset, bytes).ok()?;
    IVec2Proxy::de_bin(offset, bytes).ok()?;
    Some(())
}

fn deserialize_old_background(offset: &mut usize, bytes: &[u8]) -> Option<Background> {
    match u16::de_bin(offset, bytes).ok()? {
        OLD_BACKGROUND_DEFAULT | OLD_BACKGROUND_DEFAULT_SCALED => Some(Background::Default),
        _ => None,
    }
}

fn tree_for_tile(tile: IVec2) -> TileType {
    match (tile.x + tile.y).rem_euclid(3) {
        0 => TileType::Tree1,
        1 => TileType::Tree2,
        _ => TileType::Tree3,
    }
}

fn deserialize_old_tiles(offset: &mut usize, bytes: &[u8]) -> Option<HashMap<IVec2, TileType>> {
    let len = usize::de_bin(offset, bytes).ok()?;
    let mut tile_layout = HashMap::new();
    for _ in 0..len {
        let key: IVec2 = IVec2Proxy::de_bin(offset, bytes).ok()?.into();
        let discriminant_offset = *offset;
        let discriminant = u16::de_bin(offset, bytes).ok()?;
        if discriminant == OLD_TILE_GATE {
            continue;
        }
        if OLD_TILE_BLOCKERS.contains(&discriminant) {
            tile_layout.insert(key, tree_for_tile(key));
            continue;
        }
        if discriminant == OLD_TILE_ITEM_FLOOR {
            tile_layout.insert(key, TileType::Dirt);
            continue;
        }
        *offset = discriminant_offset;
        tile_layout.insert(key, TileType::de_bin(offset, bytes).ok()?);
    }
    Some(tile_layout)
}

fn deserialize_old_interactables_with_gates(
    version: usize,
    offset: &mut usize,
    bytes: &[u8],
) -> Option<HashMap<IVec2, Interactable>> {
    let len = usize::de_bin(offset, bytes).ok()?;
    let mut gates = Vec::new();
    for _ in 0..len {
        let key: IVec2 = IVec2Proxy::de_bin(offset, bytes).ok()?.into();
        if u16::de_bin(offset, bytes).ok()? == OLD_TILE_GATE {
            gates.push(key);
        }
    }
    let mut interactables = match version {
        1..=2 => deserialize_old_interactables(offset, bytes)?,
        3..=4 => deserialize_old_gate_interactables(offset, bytes)?,
        _ => return None,
    };
    for gate in gates {
        interactables.insert(gate, Interactable::Gate(Gate::default()));
    }
    Some(interactables)
}

impl_versioned_migration!(Level; version, offset, bytes, debug_log {
    1 => direct {
        file_name = String::de_bin(offset, bytes).ok()?,
        name = String::de_bin(offset, bytes).ok()?,
        grid_tiles = deser_proxy!(IVec2Proxy => IVec2, offset, bytes),
        tile_layout = deserialize_old_tiles(&mut offset.clone(), bytes)?,
        interactables = with_gate_transfer_areas(
            deserialize_old_interactables_with_gates(version, offset, bytes)?,
            old_level_transfer_area(version, *offset, bytes, grid_tiles)?,
        ),
        unit_spawns = deserialize_old_unit_spawns(version, offset, bytes)?,
        ambient_light_level = f32::de_bin(offset, bytes).ok()?,
        background_type = deserialize_old_background(offset, bytes)?,
        music_track = MusicTrack::BattleMain,
    },
    2 => direct {
        file_name = String::de_bin(offset, bytes).ok()?,
        name = String::de_bin(offset, bytes).ok()?,
        grid_tiles = deser_proxy!(IVec2Proxy => IVec2, offset, bytes),
        tile_layout = deserialize_old_tiles(&mut offset.clone(), bytes)?,
        interactables = with_gate_transfer_areas(
            deserialize_old_interactables_with_gates(version, offset, bytes)?,
            old_level_transfer_area(version, *offset, bytes, grid_tiles)?,
        ),
        unit_spawns = deserialize_old_unit_spawns(version, offset, bytes)?,
        ambient_light_level = f32::de_bin(offset, bytes).ok()?,
        background_type = deserialize_old_background(offset, bytes)?,
        music_track = {
            skip_old_transfer_area(offset, bytes)?;
            MusicTrack::BattleMain
        },
    },
    3..=4 => direct {
        file_name = String::de_bin(offset, bytes).ok()?,
        name = String::de_bin(offset, bytes).ok()?,
        grid_tiles = deser_proxy!(IVec2Proxy => IVec2, offset, bytes),
        tile_layout = deserialize_old_tiles(&mut offset.clone(), bytes)?,
        interactables = with_gate_transfer_areas(
            deserialize_old_interactables_with_gates(version, offset, bytes)?,
            old_level_transfer_area(version, *offset, bytes, grid_tiles)?,
        ),
        unit_spawns = deser_proxy!(
            HashMapIVec2UnitSpawnDataProxy => HashMap<IVec2, UnitSpawnData>, offset, bytes
        ),
        ambient_light_level = f32::de_bin(offset, bytes).ok()?,
        background_type = deserialize_old_background(offset, bytes)?,
        music_track = {
            skip_old_transfer_area(offset, bytes)?;
            MusicTrack::BattleMain
        },
    },
    5 => direct {
        file_name = String::de_bin(offset, bytes).ok()?,
        name = String::de_bin(offset, bytes).ok()?,
        grid_tiles = deser_proxy!(IVec2Proxy => IVec2, offset, bytes),
        tile_layout = deser_proxy!(
            HashMapIVec2TileTypeProxy => HashMap<IVec2, TileType>, offset, bytes
        ),
        interactables = with_gate_transfer_areas(
            deserialize_old_gate_interactables(offset, bytes)?,
            old_level_transfer_area(version, *offset, bytes, grid_tiles)?,
        ),
        unit_spawns = deser_proxy!(
            HashMapIVec2UnitSpawnDataProxy => HashMap<IVec2, UnitSpawnData>, offset, bytes
        ),
        ambient_light_level = f32::de_bin(offset, bytes).ok()?,
        background_type = deserialize_old_background(offset, bytes)?,
        music_track = {
            skip_old_transfer_area(offset, bytes)?;
            MusicTrack::BattleMain
        },
    },
    6..=7 => direct {
        file_name = String::de_bin(offset, bytes).ok()?,
        name = String::de_bin(offset, bytes).ok()?,
        grid_tiles = deser_proxy!(IVec2Proxy => IVec2, offset, bytes),
        tile_layout = deser_proxy!(
            HashMapIVec2TileTypeProxy => HashMap<IVec2, TileType>, offset, bytes
        ),
        interactables = with_gate_transfer_areas(
            deserialize_old_gate_interactables(offset, bytes)?,
            old_level_transfer_area(version, *offset, bytes, grid_tiles)?,
        ),
        unit_spawns = deser_proxy!(
            HashMapIVec2UnitSpawnDataProxy => HashMap<IVec2, UnitSpawnData>, offset, bytes
        ),
        ambient_light_level = f32::de_bin(offset, bytes).ok()?,
        background_type = deserialize_old_background(offset, bytes)?,
        music_track = {
            skip_old_transfer_area(offset, bytes)?;
            MusicTrack::de_bin(offset, bytes).ok()?
        },
    },
    8..=10 => direct {
        file_name = String::de_bin(offset, bytes).ok()?,
        name = String::de_bin(offset, bytes).ok()?,
        grid_tiles = deser_proxy!(IVec2Proxy => IVec2, offset, bytes),
        tile_layout = deser_proxy!(
            HashMapIVec2TileTypeProxy => HashMap<IVec2, TileType>, offset, bytes
        ),
        interactables = deser_proxy!(
            HashMapIVec2InteractableProxy => HashMap<IVec2, Interactable>, offset, bytes
        ),
        unit_spawns = deser_proxy!(
            HashMapIVec2UnitSpawnDataProxy => HashMap<IVec2, UnitSpawnData>, offset, bytes
        ),
        ambient_light_level = f32::de_bin(offset, bytes).ok()?,
        background_type = Background::de_bin(offset, bytes).ok()?,
        music_track = {
            let stored = MusicTrack::de_bin(offset, bytes).ok()?;
            if version == 9 && MAIN_PATH_LEVELS.contains(&file_name.as_str()) {
                MusicTrack::BattleMain
            } else {
                stored
            }
        },
    },
});

impl_versioned_migration!(LevelSet; version, offset, bytes, debug_log {
    1..=10 => default,
});

#[cfg(test)]
mod tests {
    use super::{
        deserialize_old_interactables, deserialize_old_interactables_with_gates,
        deserialize_old_tiles, deserialize_old_unit_spawns, OLD_BACKGROUND_DEFAULT,
        OLD_BACKGROUND_DEFAULT_SCALED, OLD_INTERACTABLE_GAS_CAN, OLD_INTERACTABLE_GATE,
        OLD_INTERACTABLE_LIGHT_SOURCE, OLD_TILE_BLOCKERS, OLD_TILE_GATE, OLD_TILE_ITEM_FLOOR,
        OLD_UNIT_TYPE_TOWER,
    };
    use crate::debug::DebugLog;
    use crate::enums::Direction;
    use crate::game_data::game_data::{Versioned, ASSET_BIN_EXT};
    use crate::game_data::proxy::{IVec2Proxy, VecIVec2Proxy};
    use crate::level::background::Background;
    use crate::level::interactable::{Interactable, LightIntensity, LightSource};
    use crate::level::level_manager::{Level, LevelSet, LEVEL_DIR, LEVEL_SET_ASSET_NAME};
    use crate::level::music_track::MusicTrack;
    use crate::level::tile::TileType;
    use crate::unit_design::{Reward, StructureSprite, UnitDesign, UnitKind, Weapon};
    use crate::unit_manager::{UnitOwner, UnitType};
    use macroquad::math::IVec2;
    use nanoserde::{DeBin, SerBin};
    use std::fs;

    #[test]
    fn version_2_tower_becomes_a_structure() {
        let tile = IVec2Proxy { x: 3, y: 4 };
        let mut design = UnitDesign::default_for(UnitKind::Soldier);
        design.weapon = Weapon::Bow;
        let mut bytes = Vec::new();
        1usize.ser_bin(&mut bytes);
        tile.ser_bin(&mut bytes);
        UnitOwner::P1.ser_bin(&mut bytes);
        OLD_UNIT_TYPE_TOWER.ser_bin(&mut bytes);
        tile.ser_bin(&mut bytes);
        Direction::Down.ser_bin(&mut bytes);
        design.ser_bin(&mut bytes);
        5u8.ser_bin(&mut bytes);
        Reward::default().ser_bin(&mut bytes);
        true.ser_bin(&mut bytes);
        VecIVec2Proxy(Vec::new()).ser_bin(&mut bytes);

        let mut offset = 0;
        let unit_spawns = deserialize_old_unit_spawns(2, &mut offset, &bytes).unwrap();
        assert_eq!(offset, bytes.len());
        let spawn = &unit_spawns[&IVec2::new(3, 4)];
        assert!(spawn.unit_type == UnitType::Structure);
        assert!(spawn.profile.structure_sprite == StructureSprite::TowerWood);
        assert_eq!(spawn.profile.structure_hp, 200);
        assert!(spawn.profile.design.weapon == Weapon::Bow);
        assert_eq!(spawn.profile.aggro_range, 5);
        assert!(spawn.profile.is_final_fortress);
    }

    #[test]
    fn old_gas_cans_are_dropped() {
        let mut bytes = Vec::new();
        2usize.ser_bin(&mut bytes);
        IVec2Proxy { x: 1, y: 1 }.ser_bin(&mut bytes);
        OLD_INTERACTABLE_GAS_CAN.ser_bin(&mut bytes);
        IVec2Proxy { x: 2, y: 1 }.ser_bin(&mut bytes);
        OLD_INTERACTABLE_LIGHT_SOURCE.ser_bin(&mut bytes);
        LightSource::new(LightIntensity::Medium).ser_bin(&mut bytes);

        let mut offset = 0;
        let interactables = deserialize_old_interactables(&mut offset, &bytes).unwrap();
        assert_eq!(offset, bytes.len());
        assert_eq!(interactables.len(), 1);
        assert!(matches!(
            interactables[&IVec2::new(2, 1)],
            Interactable::LightSource(_)
        ));
    }

    #[test]
    fn version_4_gate_tile_becomes_a_gate_interactable() {
        let mut bytes = Vec::new();
        2usize.ser_bin(&mut bytes);
        IVec2Proxy { x: 0, y: 5 }.ser_bin(&mut bytes);
        OLD_TILE_GATE.ser_bin(&mut bytes);
        IVec2Proxy { x: 1, y: 5 }.ser_bin(&mut bytes);
        TileType::Water.ser_bin(&mut bytes);
        1usize.ser_bin(&mut bytes);
        IVec2Proxy { x: 0, y: 5 }.ser_bin(&mut bytes);
        Interactable::LightSource(LightSource::new(LightIntensity::Medium)).ser_bin(&mut bytes);

        let tile_layout = deserialize_old_tiles(&mut 0, &bytes).unwrap();
        let mut offset = 0;
        let interactables =
            deserialize_old_interactables_with_gates(4, &mut offset, &bytes).unwrap();
        assert_eq!(offset, bytes.len());
        assert_eq!(tile_layout.len(), 1);
        assert!(tile_layout[&IVec2::new(1, 5)] == TileType::Water);
        assert_eq!(interactables.len(), 1);
        assert!(matches!(
            interactables[&IVec2::new(0, 5)],
            Interactable::Gate(_)
        ));
    }

    #[test]
    fn version_4_base_game_tiles_become_this_games_tiles() {
        let mut bytes = Vec::new();
        3usize.ser_bin(&mut bytes);
        IVec2Proxy { x: 2, y: 3 }.ser_bin(&mut bytes);
        OLD_TILE_BLOCKERS[0].ser_bin(&mut bytes);
        IVec2Proxy { x: 3, y: 3 }.ser_bin(&mut bytes);
        OLD_TILE_BLOCKERS[17].ser_bin(&mut bytes);
        IVec2Proxy { x: 4, y: 3 }.ser_bin(&mut bytes);
        OLD_TILE_ITEM_FLOOR.ser_bin(&mut bytes);

        let mut offset = 0;
        let tile_layout = deserialize_old_tiles(&mut offset, &bytes).unwrap();
        assert_eq!(offset, bytes.len());
        assert!(tile_layout[&IVec2::new(2, 3)] == TileType::Tree3);
        assert!(tile_layout[&IVec2::new(3, 3)] == TileType::Tree1);
        assert!(tile_layout[&IVec2::new(4, 3)] == TileType::Dirt);
    }

    const OLD_GATE_TILE: IVec2 = IVec2::new(31, 4);
    const OLD_AREA_TILE: IVec2Proxy = IVec2Proxy { x: 2, y: 3 };
    const OLD_AREA_SIZE: IVec2Proxy = IVec2Proxy { x: 4, y: 5 };

    fn old_level_bytes(background: u16, music_track: Option<MusicTrack>) -> Vec<u8> {
        let mut bytes = Vec::new();
        String::from("old_level").ser_bin(&mut bytes);
        String::from("Old level").ser_bin(&mut bytes);
        IVec2Proxy { x: 32, y: 18 }.ser_bin(&mut bytes);
        0usize.ser_bin(&mut bytes);
        1usize.ser_bin(&mut bytes);
        IVec2Proxy {
            x: OLD_GATE_TILE.x,
            y: OLD_GATE_TILE.y,
        }
        .ser_bin(&mut bytes);
        OLD_INTERACTABLE_GATE.ser_bin(&mut bytes);
        Direction::Right.ser_bin(&mut bytes);
        0usize.ser_bin(&mut bytes);
        0.5f32.ser_bin(&mut bytes);
        background.ser_bin(&mut bytes);
        OLD_AREA_TILE.ser_bin(&mut bytes);
        OLD_AREA_SIZE.ser_bin(&mut bytes);
        if let Some(music_track) = music_track {
            music_track.ser_bin(&mut bytes);
        }
        bytes
    }

    fn assert_gate_has_old_area(level: &Level) {
        let Some(Interactable::Gate(gate)) = level.interactables.get(&OLD_GATE_TILE) else {
            panic!("the gate did not load");
        };
        assert!(gate.direction == Direction::Right);
        assert_eq!(gate.transfer_area_tile, OLD_AREA_TILE.into());
        assert_eq!(gate.transfer_area_size, OLD_AREA_SIZE.into());
    }

    fn version_1_or_2_level_bytes(version: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        String::from("old_level").ser_bin(&mut bytes);
        String::from("Old level").ser_bin(&mut bytes);
        IVec2Proxy { x: 32, y: 18 }.ser_bin(&mut bytes);
        1usize.ser_bin(&mut bytes);
        IVec2Proxy {
            x: OLD_GATE_TILE.x,
            y: OLD_GATE_TILE.y,
        }
        .ser_bin(&mut bytes);
        OLD_TILE_GATE.ser_bin(&mut bytes);
        0usize.ser_bin(&mut bytes);
        let spawn_tile = IVec2Proxy { x: 3, y: 4 };
        1usize.ser_bin(&mut bytes);
        spawn_tile.ser_bin(&mut bytes);
        UnitOwner::P1.ser_bin(&mut bytes);
        OLD_UNIT_TYPE_TOWER.ser_bin(&mut bytes);
        spawn_tile.ser_bin(&mut bytes);
        Direction::Down.ser_bin(&mut bytes);
        if version == 1 {
            3u8.ser_bin(&mut bytes);
        } else {
            UnitDesign::default_for(UnitKind::Soldier).ser_bin(&mut bytes);
            5u8.ser_bin(&mut bytes);
            Reward::default().ser_bin(&mut bytes);
            false.ser_bin(&mut bytes);
            VecIVec2Proxy(vec![spawn_tile]).ser_bin(&mut bytes);
        }
        0.5f32.ser_bin(&mut bytes);
        OLD_BACKGROUND_DEFAULT.ser_bin(&mut bytes);
        if version == 2 {
            OLD_AREA_TILE.ser_bin(&mut bytes);
            OLD_AREA_SIZE.ser_bin(&mut bytes);
        }
        bytes
    }

    #[test]
    fn version_1_and_2_gate_tiles_get_the_level_transfer_area() {
        for (version, area_tile, area_size) in [
            (1, IVec2::ZERO, IVec2::new(32, 18)),
            (2, OLD_AREA_TILE.into(), OLD_AREA_SIZE.into()),
        ] {
            let bytes = version_1_or_2_level_bytes(version);
            let mut debug_log = DebugLog::new();
            let mut offset = 0;
            let loaded =
                Level::migrate_from(version, &mut offset, &bytes, "old_level", &mut debug_log)
                    .unwrap();
            assert_eq!(offset, bytes.len());
            assert!(loaded.tile_layout.is_empty());
            assert_eq!(loaded.unit_spawns.len(), 1);
            let Some(Interactable::Gate(gate)) = loaded.interactables.get(&OLD_GATE_TILE) else {
                panic!("the gate tile did not become a gate");
            };
            assert_eq!(gate.transfer_area_tile, area_tile);
            assert_eq!(gate.transfer_area_size, area_size);
        }
    }

    #[test]
    fn version_3_and_5_levels_load_with_the_battle_track() {
        let bytes = old_level_bytes(OLD_BACKGROUND_DEFAULT, None);
        for version in [3, 5] {
            let mut debug_log = DebugLog::new();
            let mut offset = 0;
            let loaded =
                Level::migrate_from(version, &mut offset, &bytes, "old_level", &mut debug_log)
                    .unwrap();
            assert_eq!(offset, bytes.len());
            assert_eq!(loaded.file_name, "old_level");
            assert_eq!(loaded.grid_tiles, IVec2::new(32, 18));
            assert_gate_has_old_area(&loaded);
            assert!(loaded.music_track == MusicTrack::BattleMain);
        }
    }

    #[test]
    fn version_6_scaled_background_becomes_the_default() {
        let bytes = old_level_bytes(OLD_BACKGROUND_DEFAULT_SCALED, Some(MusicTrack::NoMusic));
        let mut debug_log = DebugLog::new();
        let mut offset = 0;
        let loaded =
            Level::migrate_from(6, &mut offset, &bytes, "old_level", &mut debug_log).unwrap();
        assert_eq!(offset, bytes.len());
        assert!(loaded.background_type == Background::Default);
        assert!(loaded.music_track == MusicTrack::NoMusic);
    }

    #[test]
    fn version_7_gates_get_the_level_transfer_area() {
        let bytes = old_level_bytes(OLD_BACKGROUND_DEFAULT, Some(MusicTrack::NoMusic));
        let mut debug_log = DebugLog::new();
        let mut offset = 0;
        let loaded =
            Level::migrate_from(7, &mut offset, &bytes, "old_level", &mut debug_log).unwrap();
        assert_eq!(offset, bytes.len());
        assert_gate_has_old_area(&loaded);
        assert!(loaded.music_track == MusicTrack::NoMusic);
    }

    #[test]
    fn version_9_main_path_levels_get_the_battle_track() {
        for (file_name, music_track) in [
            ("level04", MusicTrack::NoMusic),
            ("level24", MusicTrack::BattleMain),
            ("level84", MusicTrack::BattleMain),
            ("level13", MusicTrack::NoMusic),
        ] {
            let mut level = Level::new(file_name, "Level", IVec2::new(40, 23));
            level.music_track = MusicTrack::NoMusic;
            let mut bytes = Vec::new();
            level.ser_bin(&mut bytes);
            let mut debug_log = DebugLog::new();
            let mut offset = 0;
            let loaded =
                Level::migrate_from(9, &mut offset, &bytes, file_name, &mut debug_log).unwrap();
            assert_eq!(offset, bytes.len());
            assert!(loaded.music_track == music_track, "{}", file_name);
        }
    }

    #[test]
    fn every_authored_level_loads() {
        let directory = format!("assets/{}", LEVEL_DIR);
        let mut debug_log = DebugLog::new();
        let mut level_count = 0;
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            let Some(extension) = path.extension() else {
                continue;
            };
            if extension != ASSET_BIN_EXT {
                continue;
            }
            let bytes = fs::read(&path).unwrap();
            let mut offset = 0;
            let version = usize::de_bin(&mut offset, &bytes).unwrap();
            let name = path.display().to_string();
            let is_level_set = path.file_stem().unwrap() == LEVEL_SET_ASSET_NAME;
            if is_level_set {
                let level_set =
                    LevelSet::migrate_from(version, &mut offset, &bytes, &name, &mut debug_log);
                assert!(level_set.is_some(), "{} did not load", name);
                continue;
            }
            let level = Level::migrate_from(version, &mut offset, &bytes, &name, &mut debug_log);
            assert!(level.is_some(), "{} did not load", name);
            assert_eq!(offset, bytes.len(), "{} has trailing bytes", name);
            level_count += 1;
        }
        assert!(level_count > 0);
    }
}
