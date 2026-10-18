use crate::constants::{
    DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y, HISTORY_CHUNK_BYTES, HISTORY_CHUNK_MAGIC,
    HISTORY_CHUNK_TURN_FIRST_OFFSET, HISTORY_CHUNK_TURN_LAST_OFFSET,
    HISTORY_CHUNK_USED_BYTES_OFFSET, HISTORY_PREFETCH_FRAMES, HISTORY_STATE_MAGIC,
    HISTORY_TIER_COUNT, HISTORY_TIER_MERGE, HISTORY_TIER_SPAN_TURNS, HISTORY_TILES_PER_SECTOR,
};
use crate::enums::Direction;
use crate::game_data::game_data::{is_readable_save_version, DATA_VERSION};
use crate::history_manager::service::{
    read_u16_at, read_u32_at, Chunk, HistoryFileRef, HistoryStorageService, LoadedFile,
};
use crate::level::interactable::Interactable;
use crate::level::level_manager::{Level, LevelManager};
use crate::unit_design::{Economy, Resources, UnitProfile, MEN_PER_UNIT};
use crate::unit_manager::{GoalNode, UnitManager, UnitOwner, UnitType};
use macroquad::prelude::IVec2;
use nanoserde::{DeBin, SerBin};
use std::cmp::Ordering;
use std::collections::HashMap;

pub const EVENT_SPAWN: u8 = 0;
pub const EVENT_DEATH: u8 = 1;
pub const EVENT_REVIVE: u8 = 2;
pub const EVENT_GOAL_PUSH: u8 = 3;
pub const EVENT_GOAL_POP_FRONT: u8 = 4;
pub const EVENT_GOAL_CLEAR: u8 = 5;
pub const EVENT_MOVE_PERIOD: u8 = 7;
pub const EVENT_OWNER: u8 = 8;
pub const EVENT_UNIT_TYPE: u8 = 9;
pub const EVENT_UNREACHABLE_STREAK: u8 = 10;
pub const EVENT_INTERACTABLE: u8 = 11;
pub const EVENT_HP: u8 = 12;
pub const EVENT_TARGET: u8 = 13;
pub const EVENT_RESOURCES: u8 = 14;
pub const EVENT_TECHNOLOGY: u8 = 15;
pub const EVENT_KILLS: u8 = 16;

pub const TARGET_NONE: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MoveRecord {
    pub unit_index: u32,
    pub sector_index: u16,
    pub tile_index: u16,
    pub facing: u8,
}

pub const MOVE_RECORD_BYTES: usize = 8;

impl MoveRecord {
    pub fn write_to(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.unit_index.to_le_bytes());
        output.extend_from_slice(&self.sector_index.to_le_bytes());
        let tile_and_facing = self.tile_index | ((self.facing as u16) << 14);
        output.extend_from_slice(&tile_and_facing.to_le_bytes());
    }

    pub fn read_from(bytes: &[u8], offset: &mut usize) -> Option<MoveRecord> {
        if *offset + MOVE_RECORD_BYTES > bytes.len() {
            return None;
        }
        let unit_index = read_u32_at(bytes, offset)?;
        let sector_index = read_u16_at(bytes, offset)?;
        let tile_and_facing = read_u16_at(bytes, offset)?;
        let tile_index = tile_and_facing & 0x3FFF;
        let facing = (tile_and_facing >> 14) as u8;
        if tile_index as u32 >= HISTORY_TILES_PER_SECTOR {
            return None;
        }
        Some(MoveRecord {
            unit_index,
            sector_index,
            tile_index,
            facing,
        })
    }
}

pub fn direction_to_facing(direction: Direction) -> u8 {
    match direction {
        Direction::Up => 0,
        Direction::Right => 1,
        Direction::Down => 2,
        Direction::Left => 3,
    }
}

pub fn facing_to_direction(facing: u8) -> Option<Direction> {
    match facing {
        0 => Some(Direction::Up),
        1 => Some(Direction::Right),
        2 => Some(Direction::Down),
        3 => Some(Direction::Left),
        _ => None,
    }
}

pub fn tile_to_index(tile: IVec2) -> u16 {
    assert!(
        tile.x >= 0
            && tile.x < DEFAULT_LEVEL_TILES_X
            && tile.y >= 0
            && tile.y < DEFAULT_LEVEL_TILES_Y,
        "[History] tile_to_index: tile {:?} out of range",
        tile
    );
    (tile.y * DEFAULT_LEVEL_TILES_X + tile.x) as u16
}

pub fn index_to_tile(index: u16) -> IVec2 {
    IVec2::new(
        (index % DEFAULT_LEVEL_TILES_X as u16) as i32,
        (index / DEFAULT_LEVEL_TILES_X as u16) as i32,
    )
}

fn read_u8(bytes: &[u8], offset: &mut usize) -> Option<u8> {
    if *offset >= bytes.len() {
        return None;
    }
    let value = bytes[*offset];
    *offset += 1;
    Some(value)
}

fn write_resources(output: &mut Vec<u8>, resources: &Resources) {
    output.extend_from_slice(&resources.men.to_le_bytes());
    output.extend_from_slice(&resources.iron.to_le_bytes());
    output.extend_from_slice(&resources.wood.to_le_bytes());
    output.extend_from_slice(&resources.mercury.to_le_bytes());
    output.extend_from_slice(&resources.sulfur.to_le_bytes());
}

fn read_resources(bytes: &[u8], offset: &mut usize) -> Option<Resources> {
    let men = read_u32_at(bytes, offset)? as i32;
    let iron = read_u32_at(bytes, offset)? as i32;
    let wood = read_u32_at(bytes, offset)? as i32;
    let mercury = read_u32_at(bytes, offset)? as i32;
    let sulfur = read_u32_at(bytes, offset)? as i32;
    Some(Resources::new(men, iron, wood, mercury, sulfur))
}

fn target_to_wire(target: Option<usize>) -> u32 {
    match target {
        Some(unit_i) => unit_i as u32,
        None => TARGET_NONE,
    }
}

fn wire_to_target(value: u32) -> Option<usize> {
    if value == TARGET_NONE {
        return None;
    }
    Some(value as usize)
}

fn encode_event_spawn(output: &mut Vec<u8>, unit_index: u32) {
    output.push(EVENT_SPAWN);
    output.extend_from_slice(&unit_index.to_le_bytes());
}

fn encode_event_goal_push(output: &mut Vec<u8>, unit_index: u32) {
    output.push(EVENT_GOAL_PUSH);
    output.extend_from_slice(&unit_index.to_le_bytes());
}

fn encode_event_goal_pop_front(
    output: &mut Vec<u8>,
    unit_index: u32,
    sector_index: u16,
    tile_index: u16,
) {
    output.push(EVENT_GOAL_POP_FRONT);
    output.extend_from_slice(&unit_index.to_le_bytes());
    output.extend_from_slice(&sector_index.to_le_bytes());
    output.extend_from_slice(&tile_index.to_le_bytes());
}

fn encode_event_unreachable_streak(output: &mut Vec<u8>, unit_index: u32, streak: u8) {
    output.push(EVENT_UNREACHABLE_STREAK);
    output.extend_from_slice(&unit_index.to_le_bytes());
    output.push(streak);
}

fn encode_event_death(output: &mut Vec<u8>, unit_index: u32) {
    output.push(EVENT_DEATH);
    output.extend_from_slice(&unit_index.to_le_bytes());
}

fn encode_event_hp(output: &mut Vec<u8>, unit_index: u32, hp: u16) {
    output.push(EVENT_HP);
    output.extend_from_slice(&unit_index.to_le_bytes());
    output.extend_from_slice(&hp.to_le_bytes());
}

fn encode_event_kills(output: &mut Vec<u8>, unit_index: u32, kills: u16) {
    output.push(EVENT_KILLS);
    output.extend_from_slice(&unit_index.to_le_bytes());
    output.extend_from_slice(&kills.to_le_bytes());
}

fn encode_event_target(output: &mut Vec<u8>, unit_index: u32, target: u32) {
    output.push(EVENT_TARGET);
    output.extend_from_slice(&unit_index.to_le_bytes());
    output.extend_from_slice(&target.to_le_bytes());
}

fn encode_event_resources(output: &mut Vec<u8>, resources: &Resources) {
    output.push(EVENT_RESOURCES);
    write_resources(output, resources);
}

fn encode_event_technology(output: &mut Vec<u8>, technology: u32) {
    output.push(EVENT_TECHNOLOGY);
    output.extend_from_slice(&technology.to_le_bytes());
}

fn encode_event_interactable(
    output: &mut Vec<u8>,
    sector_index: u16,
    tile_index: u16,
    interactable: Option<&Interactable>,
) {
    output.push(EVENT_INTERACTABLE);
    output.extend_from_slice(&sector_index.to_le_bytes());
    output.extend_from_slice(&tile_index.to_le_bytes());
    match interactable {
        Some(interactable) => {
            output.push(1);
            interactable.ser_bin(output);
        }
        None => output.push(0),
    }
}

fn encode_event_goal_clear(output: &mut Vec<u8>, unit_index: u32, nodes: &[(u16, u16)]) {
    output.push(EVENT_GOAL_CLEAR);
    output.extend_from_slice(&unit_index.to_le_bytes());
    output.extend_from_slice(&(nodes.len() as u16).to_le_bytes());
    for (sector_index, tile_index) in nodes {
        output.extend_from_slice(&sector_index.to_le_bytes());
        output.extend_from_slice(&tile_index.to_le_bytes());
    }
}

pub enum HistoryEvent {
    Spawn {
        unit_index: u32,
    },
    GoalPush {
        unit_index: u32,
    },
    GoalPopFront {
        unit_index: u32,
        sector_index: u16,
        tile_index: u16,
    },
    GoalClear {
        unit_index: u32,
        nodes: Vec<(u16, u16)>,
    },
    UnreachableStreak {
        unit_index: u32,
        streak: u8,
    },
    Interactable {
        sector_index: u16,
        tile_index: u16,
        present: u8,
        data: Option<Interactable>,
    },
    Death {
        unit_index: u32,
    },
    Revive {
        unit_index: u32,
    },
    MovePeriod {
        unit_index: u32,
        move_period: u8,
    },
    Owner {
        unit_index: u32,
        owner: u8,
    },
    UnitType {
        unit_index: u32,
        unit_type: u8,
    },
    Hp {
        unit_index: u32,
        hp: u16,
    },
    Target {
        unit_index: u32,
        target: u32,
    },
    Resources {
        resources: Resources,
    },
    Technology {
        technology: u32,
    },
    Kills {
        unit_index: u32,
        kills: u16,
    },
}

pub fn decode_events(bytes: &[u8]) -> Option<Vec<HistoryEvent>> {
    let mut offset = 0;
    let mut events = Vec::new();
    while offset < bytes.len() {
        let tag = read_u8(bytes, &mut offset)?;
        match tag {
            EVENT_SPAWN => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                events.push(HistoryEvent::Spawn { unit_index });
            }
            EVENT_GOAL_PUSH => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                events.push(HistoryEvent::GoalPush { unit_index });
            }
            EVENT_GOAL_POP_FRONT => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let sector_index = read_u16_at(bytes, &mut offset)?;
                let tile_index = read_u16_at(bytes, &mut offset)?;
                events.push(HistoryEvent::GoalPopFront {
                    unit_index,
                    sector_index,
                    tile_index,
                });
            }
            EVENT_GOAL_CLEAR => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let node_count = read_u16_at(bytes, &mut offset)? as usize;
                let mut nodes = Vec::with_capacity(node_count);
                for _ in 0..node_count {
                    let sector_index = read_u16_at(bytes, &mut offset)?;
                    let tile_index = read_u16_at(bytes, &mut offset)?;
                    nodes.push((sector_index, tile_index));
                }
                events.push(HistoryEvent::GoalClear { unit_index, nodes });
            }
            EVENT_UNREACHABLE_STREAK => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let streak = read_u8(bytes, &mut offset)?;
                events.push(HistoryEvent::UnreachableStreak { unit_index, streak });
            }
            EVENT_INTERACTABLE => {
                let sector_index = read_u16_at(bytes, &mut offset)?;
                let tile_index = read_u16_at(bytes, &mut offset)?;
                let present = read_u8(bytes, &mut offset)?;
                let data = if present == 1 {
                    Some(Interactable::de_bin(&mut offset, bytes).ok()?)
                } else {
                    None
                };
                events.push(HistoryEvent::Interactable {
                    sector_index,
                    tile_index,
                    present,
                    data,
                });
            }
            EVENT_DEATH => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                events.push(HistoryEvent::Death { unit_index });
            }
            EVENT_REVIVE => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                events.push(HistoryEvent::Revive { unit_index });
            }
            EVENT_MOVE_PERIOD => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let move_period = read_u8(bytes, &mut offset)?;
                events.push(HistoryEvent::MovePeriod {
                    unit_index,
                    move_period,
                });
            }
            EVENT_OWNER => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let owner = read_u8(bytes, &mut offset)?;
                events.push(HistoryEvent::Owner { unit_index, owner });
            }
            EVENT_UNIT_TYPE => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let unit_type = read_u8(bytes, &mut offset)?;
                events.push(HistoryEvent::UnitType {
                    unit_index,
                    unit_type,
                });
            }
            EVENT_HP => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let hp = read_u16_at(bytes, &mut offset)?;
                events.push(HistoryEvent::Hp { unit_index, hp });
            }
            EVENT_TARGET => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let target = read_u32_at(bytes, &mut offset)?;
                events.push(HistoryEvent::Target { unit_index, target });
            }
            EVENT_RESOURCES => {
                let resources = read_resources(bytes, &mut offset)?;
                events.push(HistoryEvent::Resources { resources });
            }
            EVENT_TECHNOLOGY => {
                let technology = read_u32_at(bytes, &mut offset)?;
                events.push(HistoryEvent::Technology { technology });
            }
            EVENT_KILLS => {
                let unit_index = read_u32_at(bytes, &mut offset)?;
                let kills = read_u16_at(bytes, &mut offset)?;
                events.push(HistoryEvent::Kills { unit_index, kills });
            }
            _ => return None,
        }
    }
    Some(events)
}

#[derive(Clone)]
pub struct HistoryFrame {
    pub turn: u32,
    pub moves: Vec<MoveRecord>,
    pub events: Vec<u8>,
}

pub const FRAME_FIXED_BYTES: usize = 16;

pub fn frame_byte_size(move_count: usize, event_bytes: usize) -> usize {
    FRAME_FIXED_BYTES + move_count * MOVE_RECORD_BYTES + event_bytes
}

fn round_up(value: usize, multiple: usize) -> usize {
    (value + multiple - 1) / multiple * multiple
}

pub fn write_frame(output: &mut Vec<u8>, turn: u32, moves: &[MoveRecord], events: &[u8]) {
    output.extend_from_slice(&turn.to_le_bytes());
    output.extend_from_slice(&(moves.len() as u32).to_le_bytes());
    output.extend_from_slice(&(events.len() as u32).to_le_bytes());
    for i in 0..moves.len() {
        moves[i].write_to(output);
    }
    output.extend_from_slice(events);
    let total = frame_byte_size(moves.len(), events.len()) as u32;
    output.extend_from_slice(&total.to_le_bytes());
}

pub fn read_frame_forward(bytes: &[u8], offset: &mut usize) -> Option<HistoryFrame> {
    let start = *offset;
    let turn = read_u32_at(bytes, offset)?;
    let move_count = read_u32_at(bytes, offset)? as usize;
    let event_bytes = read_u32_at(bytes, offset)? as usize;
    if move_count > (bytes.len() - *offset) / MOVE_RECORD_BYTES {
        return None;
    }
    let mut moves = Vec::with_capacity(move_count);
    for _ in 0..move_count {
        moves.push(MoveRecord::read_from(bytes, offset)?);
    }
    if event_bytes > bytes.len() - *offset {
        return None;
    }
    let events = bytes[*offset..*offset + event_bytes].to_vec();
    *offset += event_bytes;
    let trailing = read_u32_at(bytes, offset)?;
    let expected = frame_byte_size(move_count, event_bytes) as u32;
    if trailing != expected {
        return None;
    }
    if *offset - start != expected as usize {
        return None;
    }
    Some(HistoryFrame {
        turn,
        moves,
        events,
    })
}

#[cfg(test)]
pub fn read_frame_backward(bytes: &[u8], end: usize) -> Option<(HistoryFrame, usize)> {
    if end < 4 {
        return None;
    }
    let trailing = u32::from_le_bytes([
        bytes[end - 4],
        bytes[end - 3],
        bytes[end - 2],
        bytes[end - 1],
    ]) as usize;
    if trailing < FRAME_FIXED_BYTES {
        return None;
    }
    if end < trailing {
        return None;
    }
    let start = end - trailing;
    let mut offset = start;
    let frame = read_frame_forward(bytes, &mut offset)?;
    if offset != end {
        return None;
    }
    Some((frame, start))
}

pub const CHUNK_HEADER_BYTES: usize = 24;

pub struct ChunkHeader {
    pub turn_first: u32,
    pub turn_last: u32,
    pub used_bytes: u32,
    pub frame_count: u32,
    pub tier: u32,
}

pub fn write_chunk_header(
    output: &mut Vec<u8>,
    turn_first: u32,
    turn_last: u32,
    used_bytes: u32,
    frame_count: u32,
    tier: u32,
) {
    let start = output.len();
    output.extend_from_slice(&HISTORY_CHUNK_MAGIC.to_le_bytes());
    debug_assert_eq!(output.len() - start, HISTORY_CHUNK_TURN_FIRST_OFFSET);
    output.extend_from_slice(&turn_first.to_le_bytes());
    debug_assert_eq!(output.len() - start, HISTORY_CHUNK_TURN_LAST_OFFSET);
    output.extend_from_slice(&turn_last.to_le_bytes());
    debug_assert_eq!(output.len() - start, HISTORY_CHUNK_USED_BYTES_OFFSET);
    output.extend_from_slice(&used_bytes.to_le_bytes());
    output.extend_from_slice(&frame_count.to_le_bytes());
    output.extend_from_slice(&tier.to_le_bytes());
}

pub fn read_chunk_header(bytes: &[u8], offset: &mut usize) -> Option<ChunkHeader> {
    let magic = read_u32_at(bytes, offset)?;
    if magic != HISTORY_CHUNK_MAGIC {
        return None;
    }
    let turn_first = read_u32_at(bytes, offset)?;
    let turn_last = read_u32_at(bytes, offset)?;
    let used_bytes = read_u32_at(bytes, offset)?;
    let frame_count = read_u32_at(bytes, offset)?;
    let tier = read_u32_at(bytes, offset)?;
    Some(ChunkHeader {
        turn_first,
        turn_last,
        used_bytes,
        frame_count,
        tier,
    })
}

fn sector_position_order(a: &IVec2, b: &IVec2) -> Ordering {
    (a.y, a.x).cmp(&(b.y, b.x))
}

fn interactable_entry_order(
    a: &(u16, u16, &Interactable),
    b: &(u16, u16, &Interactable),
) -> Ordering {
    (a.0, a.1).cmp(&(b.0, b.1))
}

pub fn derive_sector_table(levels: &HashMap<IVec2, Level>) -> (Vec<IVec2>, HashMap<IVec2, u16>) {
    let mut positions: Vec<IVec2> = levels.keys().copied().collect();
    positions.sort_by(sector_position_order);
    assert!(
        positions.len() <= u16::MAX as usize,
        "[History] sector count {} exceeds u16::MAX",
        positions.len()
    );
    let mut lookup = HashMap::new();
    for i in 0..positions.len() {
        lookup.insert(positions[i], i as u16);
    }
    (positions, lookup)
}

fn find_file_covering(files: &[HistoryFileRef], target_turn: u32) -> Option<(u8, u32)> {
    let mut best_tier: u8 = 0;
    let mut best_sequence: u32 = 0;
    let mut found = false;
    for file_ref in files {
        if file_ref.turn_first <= target_turn && target_turn <= file_ref.turn_last {
            if !found
                || file_ref.tier > best_tier
                || (file_ref.tier == best_tier && file_ref.sequence > best_sequence)
            {
                best_tier = file_ref.tier;
                best_sequence = file_ref.sequence;
                found = true;
            }
        }
    }
    if found {
        Some((best_tier, best_sequence))
    } else {
        None
    }
}

pub struct HeadStateUnit {
    pub sector_index: u16,
    pub tile_index: u16,
    pub facing: u8,
    pub owner: u8,
    pub unit_type: u8,
    pub move_period: u8,
    pub is_dead: u8,
    pub goal_unreachable_streak: u8,
    pub hp: u16,
    pub target: u32,
    pub goal_nodes: Vec<(u16, u16)>,
    pub waypoint_nodes: Vec<(u16, u16)>,
    pub profile: UnitProfile,
    pub kills: u16,
}

pub struct HeadStateInteractable {
    pub sector_index: u16,
    pub tile_index: u16,
    pub interactable: Interactable,
}

pub struct HeadState {
    pub turn: u32,
    pub units: Vec<HeadStateUnit>,
    pub economy: Economy,
    pub interactables: Vec<HeadStateInteractable>,
    pub player_state: Vec<u8>,
    pub rewind_floor: u32,
}

pub fn encode_head_state(
    turn: u32,
    unit_manager: &UnitManager,
    sector_lookup: &HashMap<IVec2, u16>,
    levels: &HashMap<IVec2, Level>,
    player_state: &[u8],
    rewind_floor: u32,
) -> Vec<u8> {
    let mut output = Vec::with_capacity(20 + unit_manager.len() * 60);
    output.extend_from_slice(&HISTORY_STATE_MAGIC);
    output.extend_from_slice(&(DATA_VERSION as u32).to_le_bytes());
    output.extend_from_slice(&turn.to_le_bytes());
    output.extend_from_slice(&(unit_manager.len() as u32).to_le_bytes());

    for unit_index in 0..unit_manager.len() {
        let sector_index = match sector_lookup.get(&unit_manager.level_pos[unit_index]) {
            Some(&index) => index,
            None => panic!(
                "[History] encode_head_state: unit {} at unknown sector {:?}",
                unit_index, unit_manager.level_pos[unit_index]
            ),
        };
        let tile_index = tile_to_index(unit_manager.tile_position[unit_index]);
        let facing = direction_to_facing(unit_manager.direction[unit_index]);
        let owner = unit_manager.owner[unit_index].to_index() as u8;
        let unit_type = unit_manager.unit_type[unit_index].to_index() as u8;
        let move_period = unit_manager.move_period[unit_index];
        let is_dead = if unit_manager.is_dead[unit_index] {
            1u8
        } else {
            0u8
        };
        let goal_unreachable_streak = unit_manager.goal_unreachable_streak[unit_index];

        output.extend_from_slice(&sector_index.to_le_bytes());
        output.extend_from_slice(&tile_index.to_le_bytes());
        output.push(facing);
        output.push(owner);
        output.push(unit_type);
        output.push(move_period);
        output.push(is_dead);
        output.push(goal_unreachable_streak);
        output.extend_from_slice(&unit_manager.hp[unit_index].to_le_bytes());
        output.extend_from_slice(&target_to_wire(unit_manager.target[unit_index]).to_le_bytes());

        let goal_node_count = unit_manager.goal_queue[unit_index].len() as u16;
        output.extend_from_slice(&goal_node_count.to_le_bytes());
        for node in &unit_manager.goal_queue[unit_index] {
            let node_sector_index = match sector_lookup.get(&node.level_pos) {
                Some(&index) => index,
                None => panic!(
                    "[History] encode_head_state: goal node at unknown sector {:?}",
                    node.level_pos
                ),
            };
            let node_tile_index = tile_to_index(node.tile);
            output.extend_from_slice(&node_sector_index.to_le_bytes());
            output.extend_from_slice(&node_tile_index.to_le_bytes());
        }

        let waypoint_count = unit_manager.waypoints[unit_index].len() as u16;
        output.extend_from_slice(&waypoint_count.to_le_bytes());
        for node in &unit_manager.waypoints[unit_index] {
            let node_sector_index = match sector_lookup.get(&node.level_pos) {
                Some(&index) => index,
                None => panic!(
                    "[History] encode_head_state: waypoint at unknown sector {:?}",
                    node.level_pos
                ),
            };
            output.extend_from_slice(&node_sector_index.to_le_bytes());
            output.extend_from_slice(&tile_to_index(node.tile).to_le_bytes());
        }
        unit_manager.profile[unit_index].ser_bin(&mut output);
        output.extend_from_slice(&unit_manager.kills[unit_index].to_le_bytes());
    }

    write_resources(&mut output, &unit_manager.economy.resources);
    output.extend_from_slice(&unit_manager.economy.technology.to_le_bytes());

    let mut interactable_total = 0;
    for (_, level) in levels {
        interactable_total += level.interactables.len();
    }
    let mut interactable_entries: Vec<(u16, u16, &Interactable)> =
        Vec::with_capacity(interactable_total);
    for (level_pos, level) in levels {
        let sector_index = sector_lookup[level_pos];
        for (tile_pos, interactable) in &level.interactables {
            let tile_index = tile_to_index(*tile_pos);
            interactable_entries.push((sector_index, tile_index, interactable));
        }
    }
    interactable_entries.sort_by(interactable_entry_order);

    output.extend_from_slice(&(interactable_entries.len() as u32).to_le_bytes());
    for entry in &interactable_entries {
        output.extend_from_slice(&entry.0.to_le_bytes());
        output.extend_from_slice(&entry.1.to_le_bytes());
        entry.2.ser_bin(&mut output);
    }

    output.extend_from_slice(&(player_state.len() as u32).to_le_bytes());
    output.extend_from_slice(player_state);
    output.extend_from_slice(&rewind_floor.to_le_bytes());

    output
}

pub fn decode_head_state(bytes: &[u8]) -> Option<HeadState> {
    if bytes.len() < 8 {
        return None;
    }
    if &bytes[0..8] != &HISTORY_STATE_MAGIC {
        return None;
    }
    let mut offset = 8;

    let format_version = read_u32_at(bytes, &mut offset)?;
    if !is_readable_save_version(format_version) {
        return None;
    }
    let turn = read_u32_at(bytes, &mut offset)?;
    let unit_count = read_u32_at(bytes, &mut offset)? as usize;
    const MIN_UNIT_BYTES: usize = 20;
    if unit_count > (bytes.len() - offset) / MIN_UNIT_BYTES {
        return None;
    }

    let mut units = Vec::with_capacity(unit_count);
    for _ in 0..unit_count {
        let sector_index = read_u16_at(bytes, &mut offset)?;
        let tile_index = read_u16_at(bytes, &mut offset)?;
        let facing = read_u8(bytes, &mut offset)?;
        let owner = read_u8(bytes, &mut offset)?;
        let unit_type = read_u8(bytes, &mut offset)?;
        let move_period = read_u8(bytes, &mut offset)?;
        let is_dead = read_u8(bytes, &mut offset)?;
        let goal_unreachable_streak = read_u8(bytes, &mut offset)?;
        let hp = read_u16_at(bytes, &mut offset)?;
        let target = read_u32_at(bytes, &mut offset)?;

        if tile_index as u32 >= HISTORY_TILES_PER_SECTOR {
            return None;
        }
        if facing > 3 {
            return None;
        }

        let goal_node_count = read_u16_at(bytes, &mut offset)? as usize;
        let mut goal_nodes = Vec::with_capacity(goal_node_count);
        for _ in 0..goal_node_count {
            let node_sector_index = read_u16_at(bytes, &mut offset)?;
            let node_tile_index = read_u16_at(bytes, &mut offset)?;
            if node_tile_index as u32 >= HISTORY_TILES_PER_SECTOR {
                return None;
            }
            goal_nodes.push((node_sector_index, node_tile_index));
        }

        let waypoint_count = read_u16_at(bytes, &mut offset)? as usize;
        let mut waypoint_nodes = Vec::with_capacity(waypoint_count);
        for _ in 0..waypoint_count {
            let node_sector_index = read_u16_at(bytes, &mut offset)?;
            let node_tile_index = read_u16_at(bytes, &mut offset)?;
            if node_tile_index as u32 >= HISTORY_TILES_PER_SECTOR {
                return None;
            }
            waypoint_nodes.push((node_sector_index, node_tile_index));
        }
        let profile = UnitProfile::de_bin(&mut offset, bytes).ok()?;
        let kills = match format_version {
            9..=10 => 0,
            11 => read_u16_at(bytes, &mut offset)?,
            _ => return None,
        };

        units.push(HeadStateUnit {
            sector_index,
            tile_index,
            facing,
            owner,
            unit_type,
            move_period,
            is_dead,
            goal_unreachable_streak,
            hp,
            target,
            goal_nodes,
            waypoint_nodes,
            profile,
            kills,
        });
    }

    let resources = read_resources(bytes, &mut offset)?;
    let technology = read_u32_at(bytes, &mut offset)?;

    let interactable_count = read_u32_at(bytes, &mut offset)? as usize;
    const MIN_INTERACTABLE_BYTES: usize = 7;
    if interactable_count > (bytes.len() - offset) / MIN_INTERACTABLE_BYTES {
        return None;
    }
    let mut interactables = Vec::with_capacity(interactable_count);
    for _ in 0..interactable_count {
        let sector_index = read_u16_at(bytes, &mut offset)?;
        let tile_index = read_u16_at(bytes, &mut offset)?;
        if tile_index as u32 >= HISTORY_TILES_PER_SECTOR {
            return None;
        }
        let interactable = Interactable::de_bin(&mut offset, bytes).ok()?;
        interactables.push(HeadStateInteractable {
            sector_index,
            tile_index,
            interactable,
        });
    }

    let player_state_length = read_u32_at(bytes, &mut offset)? as usize;
    if player_state_length > bytes.len() - offset {
        return None;
    }
    let player_state = bytes[offset..offset + player_state_length].to_vec();
    offset += player_state_length;
    let rewind_floor = read_u32_at(bytes, &mut offset)?;

    Some(HeadState {
        turn,
        units,
        economy: Economy {
            resources,
            technology,
        },
        interactables,
        player_state,
        rewind_floor,
    })
}

pub fn describe_head_state_offset(bytes: &[u8], target: usize) -> String {
    if target < 8 {
        return String::from("header magic");
    }
    if target < 12 {
        return String::from("header format_version");
    }
    if target < 16 {
        return String::from("header turn");
    }
    if target < 20 {
        return String::from("header unit_count");
    }
    let mut count_offset = 16;
    let unit_count = match read_u32_at(bytes, &mut count_offset) {
        Some(count) => count as usize,
        None => return String::from("header (truncated)"),
    };

    let mut offset = 20;
    for unit_index in 0..unit_count {
        let unit_start = offset;
        if target < unit_start + 18 {
            let field = match target - unit_start {
                0 | 1 => "sector_index",
                2 | 3 => "tile_index",
                4 => "facing",
                5 => "owner",
                6 => "unit_type",
                7 => "move_period",
                8 => "is_dead",
                9 => "goal_unreachable_streak",
                10 | 11 => "hp",
                12..=15 => "target",
                _ => "goal_node_count",
            };
            return format!("unit {} {}", unit_index, field);
        }
        let mut goal_count_offset = unit_start + 16;
        let goal_count = match read_u16_at(bytes, &mut goal_count_offset) {
            Some(count) => count as usize,
            None => return format!("unit {} (truncated)", unit_index),
        };
        let goal_start = unit_start + 18;
        let goal_end = goal_start + goal_count * 4;
        if target < goal_end {
            let into = target - goal_start;
            let node_field = if into % 4 >= 2 {
                "tile_index"
            } else {
                "sector_index"
            };
            return format!("unit {} goal_node {} {}", unit_index, into / 4, node_field);
        }
        if target < goal_end + 2 {
            return format!("unit {} waypoint_count", unit_index);
        }
        let mut waypoint_count_offset = goal_end;
        let waypoint_count = match read_u16_at(bytes, &mut waypoint_count_offset) {
            Some(count) => count as usize,
            None => return format!("unit {} (truncated)", unit_index),
        };
        let waypoint_end = goal_end + 2 + waypoint_count * 4;
        if target < waypoint_end {
            return format!(
                "unit {} waypoint {}",
                unit_index,
                (target - goal_end - 2) / 4
            );
        }
        let mut profile_end = waypoint_end;
        if UnitProfile::de_bin(&mut profile_end, bytes).is_err() {
            return format!("unit {} profile (decode error)", unit_index);
        }
        if target < profile_end {
            return format!("unit {} profile", unit_index);
        }
        if target < profile_end + 2 {
            return format!("unit {} kills", unit_index);
        }
        offset = profile_end + 2;
    }

    if target < offset + 24 {
        return String::from("economy");
    }
    offset += 24;

    if target < offset + 4 {
        return String::from("interactable_count");
    }
    let mut interactable_count_offset = offset;
    let interactable_count = match read_u32_at(bytes, &mut interactable_count_offset) {
        Some(count) => count as usize,
        None => return String::from("interactable_count (truncated)"),
    };
    offset += 4;
    for interactable_index in 0..interactable_count {
        let entry_start = offset;
        if target < entry_start + 4 {
            return format!("interactable {} sector/tile", interactable_index);
        }
        let mut data_offset = entry_start + 4;
        match Interactable::de_bin(&mut data_offset, bytes) {
            Ok(_) => {
                if target < data_offset {
                    return format!("interactable {} data", interactable_index);
                }
                offset = data_offset;
            }
            Err(_) => return format!("interactable {} (decode error)", interactable_index),
        }
    }
    if target < offset + 4 {
        return String::from("player_state length");
    }
    let mut player_state_length_offset = offset;
    let player_state_length = match read_u32_at(bytes, &mut player_state_length_offset) {
        Some(length) => length as usize,
        None => return String::from("player_state length (truncated)"),
    };
    offset += 4 + player_state_length;
    if target < offset {
        return String::from("player_state");
    }
    if target < offset + 4 {
        return String::from("rewind_floor");
    }
    String::from("past end")
}

pub struct HistoryTier {
    pub frames: Vec<HistoryFrame>,
    pub chunk_buffer: Vec<u8>,
    pub chunk_turn_first: u32,
    pub chunk_frame_count: u32,
    pub turn_oldest: u32,
    pub turn_newest: u32,
    pub sealed_chunk_count: u32,
}

impl HistoryTier {
    fn new() -> Self {
        Self::with_chunk_capacity(HISTORY_CHUNK_BYTES)
    }

    fn new_empty() -> Self {
        Self::with_chunk_capacity(0)
    }

    fn with_chunk_capacity(chunk_capacity: usize) -> Self {
        Self {
            frames: Vec::new(),
            chunk_buffer: Vec::with_capacity(chunk_capacity),
            chunk_turn_first: 0,
            chunk_frame_count: 0,
            turn_oldest: 0,
            turn_newest: 0,
            sealed_chunk_count: 0,
        }
    }

    pub fn span_in_tier0_turns(&self) -> u32 {
        if self.frames.is_empty() {
            return 0;
        }
        let newest = self.frames[self.frames.len() - 1].turn;
        let oldest = self.frames[0].turn;
        newest - oldest
    }
}

pub struct HistoryRecorder {
    pub tiers: [HistoryTier; HISTORY_TIER_COUNT],
    pub is_enabled: bool,
    pub head_turn: u32,
    pub sector_table: Vec<IVec2>,
    pub sector_lookup: HashMap<IVec2, u16>,
    frame_moves: Vec<MoveRecord>,
    frame_events: Vec<u8>,
    merge_value: Vec<MoveRecord>,
    merge_epoch: Vec<u32>,
    merge_epoch_current: u32,
    merge_touched: Vec<usize>,
    pub pending_load_identity: Option<(u8, u32)>,
    pub pending_load_is_rewind: bool,
    pub rewind_floor: u32,
}

impl HistoryRecorder {
    pub fn new(levels: &HashMap<IVec2, Level>) -> Self {
        let (sector_table, sector_lookup) = derive_sector_table(levels);
        let tiers = [
            HistoryTier::new(),
            HistoryTier::new(),
            HistoryTier::new(),
            HistoryTier::new(),
            HistoryTier::new(),
        ];

        Self {
            tiers,
            is_enabled: true,
            head_turn: 0,
            sector_table,
            sector_lookup,
            rewind_floor: 0,
            frame_moves: Vec::new(),
            frame_events: Vec::new(),
            merge_value: Vec::new(),
            merge_epoch: Vec::new(),
            merge_epoch_current: 0,
            merge_touched: Vec::new(),
            pending_load_identity: None,
            pending_load_is_rewind: false,
        }
    }

    pub fn new_disabled() -> Self {
        let tiers = [
            HistoryTier::new_empty(),
            HistoryTier::new_empty(),
            HistoryTier::new_empty(),
            HistoryTier::new_empty(),
            HistoryTier::new_empty(),
        ];

        Self {
            tiers,
            is_enabled: false,
            head_turn: 0,
            sector_table: Vec::new(),
            sector_lookup: HashMap::new(),
            rewind_floor: 0,
            frame_moves: Vec::new(),
            frame_events: Vec::new(),
            merge_value: Vec::new(),
            merge_epoch: Vec::new(),
            merge_epoch_current: 0,
            merge_touched: Vec::new(),
            pending_load_identity: None,
            pending_load_is_rewind: false,
        }
    }

    pub fn register_sector(&mut self, level_pos: IVec2) {
        if self.sector_lookup.contains_key(&level_pos) {
            return;
        }
        assert!(
            self.sector_table.len() < u16::MAX as usize,
            "[History] register_sector: sector count exceeds u16::MAX"
        );
        let index = self.sector_table.len() as u16;
        self.sector_table.push(level_pos);
        self.sector_lookup.insert(level_pos, index);
    }

    pub fn restore_sector_table(&mut self, table: Vec<IVec2>) {
        assert!(
            table.len() <= u16::MAX as usize,
            "[History] restore_sector_table: sector count exceeds u16::MAX"
        );
        let mut lookup = HashMap::new();
        for i in 0..table.len() {
            lookup.insert(table[i], i as u16);
        }
        self.sector_table = table;
        self.sector_lookup = lookup;
    }

    pub fn begin_turn(&mut self, turn: u32) {
        if !self.is_enabled {
            return;
        }
        self.frame_moves.clear();
        self.head_turn = turn;
    }

    pub fn record_move(
        &mut self,
        unit_index: u32,
        level_pos: IVec2,
        tile_position: IVec2,
        direction: Direction,
    ) {
        if !self.is_enabled {
            return;
        }
        let sector_index = match self.sector_lookup.get(&level_pos) {
            Some(&index) => index,
            None => panic!("[History] record_move: unknown sector {:?}", level_pos),
        };
        let tile_index = tile_to_index(tile_position);
        let facing = direction_to_facing(direction);
        self.frame_moves.push(MoveRecord {
            unit_index,
            sector_index,
            tile_index,
            facing,
        });
    }

    pub fn record_event_spawn(&mut self, unit_index: u32) {
        if !self.is_enabled {
            return;
        }
        encode_event_spawn(&mut self.frame_events, unit_index);
    }

    pub fn record_event_goal_push(&mut self, unit_index: u32) {
        if !self.is_enabled {
            return;
        }
        encode_event_goal_push(&mut self.frame_events, unit_index);
    }

    pub fn record_event_goal_pop_front(&mut self, unit_index: u32, level_pos: IVec2, tile: IVec2) {
        if !self.is_enabled {
            return;
        }
        let sector_index = match self.sector_lookup.get(&level_pos) {
            Some(&index) => index,
            None => panic!(
                "[History] record_event_goal_pop_front: unknown sector {:?}",
                level_pos
            ),
        };
        let tile_index = tile_to_index(tile);
        encode_event_goal_pop_front(&mut self.frame_events, unit_index, sector_index, tile_index);
    }

    pub fn record_event_goal_clear(&mut self, unit_index: u32, old_queue: &[GoalNode]) {
        if !self.is_enabled {
            return;
        }
        assert!(
            old_queue.len() <= u16::MAX as usize,
            "[History] record_event_goal_clear: {} nodes exceeds u16::MAX",
            old_queue.len()
        );
        let mut nodes: Vec<(u16, u16)> = Vec::with_capacity(old_queue.len());
        for node in old_queue {
            let sector_index = match self.sector_lookup.get(&node.level_pos) {
                Some(&index) => index,
                None => panic!(
                    "[History] record_event_goal_clear: unknown sector {:?}",
                    node.level_pos
                ),
            };
            nodes.push((sector_index, tile_to_index(node.tile)));
        }
        encode_event_goal_clear(&mut self.frame_events, unit_index, &nodes);
    }

    pub fn record_event_unreachable_streak(&mut self, unit_index: u32, old_streak: u8) {
        if !self.is_enabled {
            return;
        }
        encode_event_unreachable_streak(&mut self.frame_events, unit_index, old_streak);
    }

    pub fn record_event_death(&mut self, unit_index: u32) {
        if !self.is_enabled {
            return;
        }
        encode_event_death(&mut self.frame_events, unit_index);
    }

    pub fn record_event_hp(&mut self, unit_index: u32, old_hp: u16) {
        if !self.is_enabled {
            return;
        }
        encode_event_hp(&mut self.frame_events, unit_index, old_hp);
    }

    pub fn record_event_kills(&mut self, unit_index: u32, old_kills: u16) {
        if !self.is_enabled {
            return;
        }
        encode_event_kills(&mut self.frame_events, unit_index, old_kills);
    }

    pub fn record_event_target(&mut self, unit_index: u32, old_target: Option<usize>) {
        if !self.is_enabled {
            return;
        }
        encode_event_target(
            &mut self.frame_events,
            unit_index,
            target_to_wire(old_target),
        );
    }

    pub fn record_event_resources(&mut self, old_resources: &Resources) {
        if !self.is_enabled {
            return;
        }
        encode_event_resources(&mut self.frame_events, old_resources);
    }

    pub fn record_event_technology(&mut self, old_technology: u32) {
        if !self.is_enabled {
            return;
        }
        encode_event_technology(&mut self.frame_events, old_technology);
    }

    pub fn record_event_interactable(
        &mut self,
        level_pos: IVec2,
        tile: IVec2,
        old_interactable: Option<&Interactable>,
    ) {
        if !self.is_enabled {
            return;
        }
        let sector_index = match self.sector_lookup.get(&level_pos) {
            Some(&index) => index,
            None => panic!(
                "[History] record_event_interactable: unknown sector {:?}",
                level_pos
            ),
        };
        let tile_index = tile_to_index(tile);
        encode_event_interactable(
            &mut self.frame_events,
            sector_index,
            tile_index,
            old_interactable,
        );
    }

    pub fn end_turn(&mut self, storage: &HistoryStorageService) -> bool {
        if !self.is_enabled {
            return false;
        }
        let frame = HistoryFrame {
            turn: self.head_turn,
            moves: self.frame_moves.clone(),
            events: self.frame_events.clone(),
        };
        self.frame_moves.clear();
        self.frame_events.clear();

        let sealed = self.push_frame_to_tier(0, frame, storage);
        let promoted_sealed = self.run_promotion(storage);

        sealed || promoted_sealed
    }

    fn push_frame_to_tier(
        &mut self,
        tier_index: usize,
        frame: HistoryFrame,
        storage: &HistoryStorageService,
    ) -> bool {
        let frame_size = frame_byte_size(frame.moves.len(), frame.events.len());

        if frame_size + CHUNK_HEADER_BYTES > HISTORY_CHUNK_BYTES {
            self.seal_tier_chunk(tier_index, storage);
            self.publish_oversized_frame(tier_index, &frame, storage);
            if tier_index != HISTORY_TIER_COUNT - 1 {
                let tier = &mut self.tiers[tier_index];
                tier.turn_newest = frame.turn;
                if tier.frames.is_empty() {
                    tier.turn_oldest = frame.turn;
                }
                tier.frames.push(frame);
            }
            return true;
        }

        let needs_seal = {
            let tier = &self.tiers[tier_index];
            !tier.chunk_buffer.is_empty()
                && CHUNK_HEADER_BYTES + tier.chunk_buffer.len() + frame_size > HISTORY_CHUNK_BYTES
        };
        if needs_seal {
            self.seal_tier_chunk(tier_index, storage);
        }

        let tier = &mut self.tiers[tier_index];

        if tier.chunk_frame_count == 0 {
            tier.chunk_turn_first = frame.turn;
        }

        write_frame(
            &mut tier.chunk_buffer,
            frame.turn,
            &frame.moves,
            &frame.events,
        );
        tier.chunk_frame_count += 1;
        tier.turn_newest = frame.turn;
        if tier.frames.is_empty() {
            tier.turn_oldest = frame.turn;
        }

        tier.frames.push(frame);
        needs_seal
    }

    fn publish_oversized_frame(
        &mut self,
        tier_index: usize,
        frame: &HistoryFrame,
        storage: &HistoryStorageService,
    ) {
        let frame_size = frame_byte_size(frame.moves.len(), frame.events.len());
        let capacity = round_up(frame_size + CHUNK_HEADER_BYTES, 4096);
        let mut chunk_bytes = Vec::with_capacity(capacity);
        write_chunk_header(
            &mut chunk_bytes,
            frame.turn,
            frame.turn,
            frame_size as u32,
            1,
            tier_index as u32,
        );
        write_frame(&mut chunk_bytes, frame.turn, &frame.moves, &frame.events);

        storage.publish_chunk(tier_index as u8, chunk_bytes);

        self.tiers[tier_index].sealed_chunk_count += 1;
    }

    fn seal_tier_chunk(&mut self, tier_index: usize, storage: &HistoryStorageService) {
        let tier = &mut self.tiers[tier_index];
        if tier.chunk_buffer.is_empty() {
            return;
        }
        let mut chunk_bytes = Vec::with_capacity(CHUNK_HEADER_BYTES + tier.chunk_buffer.len());
        let turn_last = tier.turn_newest;
        write_chunk_header(
            &mut chunk_bytes,
            tier.chunk_turn_first,
            turn_last,
            tier.chunk_buffer.len() as u32,
            tier.chunk_frame_count,
            tier_index as u32,
        );
        chunk_bytes.extend_from_slice(&tier.chunk_buffer);

        storage.publish_chunk(tier_index as u8, chunk_bytes);

        tier.sealed_chunk_count += 1;
        let sealed_frames = tier.chunk_frame_count as usize;
        tier.chunk_buffer.clear();
        tier.chunk_frame_count = 0;

        if tier_index == HISTORY_TIER_COUNT - 1 && sealed_frames > 0 {
            assert!(
                sealed_frames <= tier.frames.len(),
                "[History] tier {} sealed {} frames into a chunk but only {} are resident - RAM and disk have desynced",
                tier_index, sealed_frames, tier.frames.len()
            );
            let start = tier.frames.len() - sealed_frames;
            tier.frames.drain(start..);
            if !tier.frames.is_empty() {
                tier.turn_oldest = tier.frames[0].turn;
                tier.turn_newest = tier.frames[tier.frames.len() - 1].turn;
            } else {
                tier.turn_oldest = 0;
                tier.turn_newest = 0;
            }
        }
    }

    fn run_promotion(&mut self, storage: &HistoryStorageService) -> bool {
        let mut any_sealed = false;
        for tier_index in 0..HISTORY_TIER_COUNT - 1 {
            let merge_count = HISTORY_TIER_MERGE[tier_index + 1] as usize;
            let span_limit = HISTORY_TIER_SPAN_TURNS[tier_index];

            let should_promote = {
                let tier = &self.tiers[tier_index];
                tier.frames.len() > merge_count && tier.span_in_tier0_turns() > span_limit
            };
            if should_promote && self.promote_one_group(tier_index, merge_count, storage) {
                any_sealed = true;
            }
        }
        any_sealed
    }

    fn promote_one_group(
        &mut self,
        tier_index: usize,
        merge_count: usize,
        storage: &HistoryStorageService,
    ) -> bool {
        let target_tier = tier_index + 1;

        let mut frames_to_merge: Vec<HistoryFrame> = Vec::with_capacity(merge_count);
        for frame in self.tiers[tier_index].frames.drain(0..merge_count) {
            frames_to_merge.push(frame);
        }

        if !self.tiers[tier_index].frames.is_empty() {
            self.tiers[tier_index].turn_oldest = self.tiers[tier_index].frames[0].turn;
        }

        let merged = self.merge_frames(&frames_to_merge);

        self.push_frame_to_tier(target_tier, merged, storage)
    }

    fn merge_frames(&mut self, frames: &[HistoryFrame]) -> HistoryFrame {
        self.merge_epoch_current += 1;
        let epoch = self.merge_epoch_current;

        let mut unit_count = 0;
        let mut event_bytes = 0;
        for frame in frames {
            for record in &frame.moves {
                if record.unit_index as usize >= unit_count {
                    unit_count = record.unit_index as usize + 1;
                }
            }
            event_bytes += frame.events.len();
        }

        let mut merged_events: Vec<u8> = Vec::with_capacity(event_bytes);

        if self.merge_value.len() < unit_count {
            self.merge_value.resize(
                unit_count,
                MoveRecord {
                    unit_index: 0,
                    sector_index: 0,
                    tile_index: 0,
                    facing: 0,
                },
            );
            self.merge_epoch.resize(unit_count, 0);
        }

        self.merge_touched.clear();
        for i in 0..frames.len() {
            let frame = &frames[i];
            for record in &frame.moves {
                let index = record.unit_index as usize;
                if self.merge_epoch[index] != epoch {
                    self.merge_epoch[index] = epoch;
                    self.merge_value[index] = *record;
                    self.merge_touched.push(index);
                }
            }
        }

        let mut merged_moves: Vec<MoveRecord> = Vec::with_capacity(self.merge_touched.len());
        for i in 0..self.merge_touched.len() {
            merged_moves.push(self.merge_value[self.merge_touched[i]]);
        }

        for frame in frames {
            merged_events.extend_from_slice(&frame.events);
        }

        let oldest_turn = frames[0].turn;

        HistoryFrame {
            turn: oldest_turn,
            moves: merged_moves,
            events: merged_events,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn flush_all_chunks(&mut self, storage: &HistoryStorageService) {
        for tier_index in 0..HISTORY_TIER_COUNT {
            self.seal_tier_chunk(tier_index, storage);
        }
    }

    pub fn start_after_load(&mut self, turn: u32) {
        for tier in &mut self.tiers {
            tier.frames.clear();
            tier.chunk_buffer.clear();
            tier.chunk_frame_count = 0;
            tier.turn_oldest = 0;
            tier.turn_newest = 0;
        }
        self.frame_moves.clear();
        self.frame_events.clear();
        self.head_turn = turn;
        self.rewind_floor = turn;
    }

    pub fn resident_frame_count(&self) -> usize {
        let mut total = 0;
        for tier in &self.tiers {
            total += tier.frames.len();
        }
        total
    }

    pub fn step_back(
        &mut self,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
    ) -> Option<u32> {
        if !self.frame_events.is_empty() {
            let staged = match decode_events(&self.frame_events) {
                Some(events) => events,
                None => panic!("[History] corrupt staged event stream"),
            };
            for i in (0..staged.len()).rev() {
                self.apply_event(&staged[i], unit_manager, level_manager);
            }
            self.frame_moves.clear();
            self.frame_events.clear();
            for i in 0..unit_manager.len() {
                unit_manager.prev_tile_position[i] = unit_manager.tile_position[i];
                unit_manager.prev_level_pos[i] = unit_manager.level_pos[i];
            }
            return Some(self.head_turn);
        }

        if self.head_turn <= self.rewind_floor {
            return None;
        }

        let mut source_tier = HISTORY_TIER_COUNT;
        for tier_index in 0..HISTORY_TIER_COUNT {
            if !self.tiers[tier_index].frames.is_empty() {
                source_tier = tier_index;
                break;
            }
        }
        if source_tier == HISTORY_TIER_COUNT {
            return None;
        }

        let frame = self.tiers[source_tier].frames.pop().unwrap();

        if self.tiers[source_tier].chunk_frame_count > 0 {
            let frame_size = frame_byte_size(frame.moves.len(), frame.events.len());
            let tier = &mut self.tiers[source_tier];
            assert!(
                tier.chunk_buffer.len() >= frame_size,
                "[History] step_back: tier {} chunk_buffer ({} bytes) cannot hold the {}-byte frame being popped",
                source_tier, tier.chunk_buffer.len(), frame_size
            );
            tier.chunk_buffer
                .truncate(tier.chunk_buffer.len() - frame_size);
            tier.chunk_frame_count -= 1;
        }

        if !self.tiers[source_tier].frames.is_empty() {
            let last = &self.tiers[source_tier].frames[self.tiers[source_tier].frames.len() - 1];
            self.tiers[source_tier].turn_newest = last.turn;
        } else {
            self.tiers[source_tier].turn_newest = 0;
            self.tiers[source_tier].turn_oldest = 0;
        }

        let events = match decode_events(&frame.events) {
            Some(events) => events,
            None => panic!(
                "[History] corrupt event stream in frame at turn {}",
                frame.turn
            ),
        };
        for i in (0..events.len()).rev() {
            self.apply_event(&events[i], unit_manager, level_manager);
        }

        for record in &frame.moves {
            self.apply_move(record, unit_manager);
        }

        for i in 0..unit_manager.len() {
            unit_manager.prev_tile_position[i] = unit_manager.tile_position[i];
            unit_manager.prev_level_pos[i] = unit_manager.level_pos[i];
        }

        self.head_turn = frame.turn;
        Some(frame.turn)
    }

    fn encode_republish_chunk(
        republish: &mut Vec<Chunk>,
        frames: &[HistoryFrame],
        tier: u8,
        data_len: usize,
    ) {
        let turn_first = frames[0].turn;
        let turn_last = frames[frames.len() - 1].turn;
        let mut chunk_bytes = Vec::with_capacity(CHUNK_HEADER_BYTES + data_len);
        write_chunk_header(
            &mut chunk_bytes,
            turn_first,
            turn_last,
            0,
            frames.len() as u32,
            tier as u32,
        );
        for frame in frames {
            write_frame(&mut chunk_bytes, frame.turn, &frame.moves, &frame.events);
        }
        let used_bytes = (chunk_bytes.len() - CHUNK_HEADER_BYTES) as u32;
        chunk_bytes[HISTORY_CHUNK_USED_BYTES_OFFSET..HISTORY_CHUNK_USED_BYTES_OFFSET + 4]
            .copy_from_slice(&used_bytes.to_le_bytes());
        republish.push(Chunk {
            bytes: chunk_bytes,
            turn_first,
            turn_last,
            used_bytes,
            frame_count: frames.len() as u32,
        });
    }

    pub fn install_loaded_file(
        &mut self,
        loaded: LoadedFile,
        storage: &HistoryStorageService,
    ) -> bool {
        let tier_index = loaded.tier as usize;
        assert!(
            tier_index < HISTORY_TIER_COUNT,
            "[History] install_loaded_file: tier {} out of range",
            loaded.tier
        );

        let mut frame_capacity = 0;
        for chunk in &loaded.chunks {
            frame_capacity += chunk.frame_count as usize;
        }
        let mut loaded_frames: Vec<HistoryFrame> = Vec::with_capacity(frame_capacity);
        for chunk in &loaded.chunks {
            let end = CHUNK_HEADER_BYTES + chunk.used_bytes as usize;
            let mut offset = CHUNK_HEADER_BYTES;
            let mut parsed_count: u32 = 0;
            while offset < end {
                let frame = match read_frame_forward(&chunk.bytes, &mut offset) {
                    Some(frame) => frame,
                    None => panic!(
                        "[History] corrupt chunk: frame parse failed before reaching used_bytes ({} of {})",
                        offset, end
                    ),
                };
                loaded_frames.push(frame);
                parsed_count += 1;
            }
            assert_eq!(
                offset, end,
                "[History] corrupt chunk: frames overran used_bytes ({} vs {})",
                offset, end
            );
            assert_eq!(
                parsed_count, chunk.frame_count,
                "[History] corrupt chunk: header claims {} frames, {} parsed",
                chunk.frame_count, parsed_count
            );
        }

        let keep_below = if self.tiers[tier_index].frames.is_empty() {
            self.head_turn
        } else {
            self.tiers[tier_index].frames[0].turn
        };
        let mut kept_frames = Vec::with_capacity(loaded_frames.len());
        for frame in loaded_frames {
            if frame.turn < keep_below {
                kept_frames.push(frame);
            }
        }

        if self.pending_load_is_rewind && !kept_frames.is_empty() {
            let mut republish: Vec<Chunk> = Vec::new();
            let mut start = 0;
            let mut chunk_bytes_len: usize = 0;
            for index in 0..kept_frames.len() {
                let frame_size = frame_byte_size(
                    kept_frames[index].moves.len(),
                    kept_frames[index].events.len(),
                );
                if index > start
                    && CHUNK_HEADER_BYTES + chunk_bytes_len + frame_size > HISTORY_CHUNK_BYTES
                {
                    Self::encode_republish_chunk(
                        &mut republish,
                        &kept_frames[start..index],
                        loaded.tier,
                        chunk_bytes_len,
                    );
                    start = index;
                    chunk_bytes_len = 0;
                }
                chunk_bytes_len += frame_size;
            }
            Self::encode_republish_chunk(
                &mut republish,
                &kept_frames[start..],
                loaded.tier,
                chunk_bytes_len,
            );
            storage.republish_chunks(loaded.tier, republish);
        }

        let installed = !kept_frames.is_empty();
        if installed {
            let loaded_newest = kept_frames[kept_frames.len() - 1].turn;
            let resident = &self.tiers[tier_index].frames;
            if !resident.is_empty() {
                let resident_oldest = resident[0].turn;
                assert!(
                    loaded_newest < resident_oldest,
                    "[History] install_loaded_file: loaded frames (newest {}) overlap resident frames (oldest {}) in tier {}",
                    loaded_newest, resident_oldest, tier_index
                );
            }
            if tier_index + 1 < HISTORY_TIER_COUNT {
                let older_tier = &self.tiers[tier_index + 1].frames;
                if !older_tier.is_empty() {
                    let loaded_oldest = kept_frames[0].turn;
                    let older_newest = older_tier[older_tier.len() - 1].turn;
                    assert!(
                        loaded_oldest > older_newest,
                        "[History] install_loaded_file: stale file for tier {} (loaded oldest {}) sits below tier {} (newest {})",
                        tier_index, loaded_oldest, tier_index + 1, older_newest
                    );
                }
            }
            let mut combined = kept_frames;
            combined.append(&mut self.tiers[tier_index].frames);
            self.tiers[tier_index].frames = combined;

            self.tiers[tier_index].turn_oldest = self.tiers[tier_index].frames[0].turn;
            let last = self.tiers[tier_index].frames.len() - 1;
            self.tiers[tier_index].turn_newest = self.tiers[tier_index].frames[last].turn;
        }

        self.pending_load_identity = None;
        self.pending_load_is_rewind = false;
        installed
    }

    pub fn check_prefetch(&mut self, storage: &HistoryStorageService) {
        if self.pending_load_identity.is_some() {
            return;
        }
        if self.head_turn == 0 {
            return;
        }

        if self.resident_frame_count() > HISTORY_PREFETCH_FRAMES as usize {
            return;
        }

        let mut oldest_turn = 0;
        let mut has_resident = false;
        for tier_index in 0..HISTORY_TIER_COUNT {
            if !self.tiers[tier_index].frames.is_empty() {
                oldest_turn = self.tiers[tier_index].turn_oldest;
                has_resident = true;
            }
        }
        if !has_resident || oldest_turn == 0 || oldest_turn <= self.rewind_floor {
            return;
        }
        let target_turn = oldest_turn - 1;

        let (tier, sequence) = match find_file_covering(&storage.files, target_turn) {
            Some(identity) => identity,
            None => return,
        };
        if storage.request_load(tier, sequence) {
            self.pending_load_identity = Some((tier, sequence));
            self.pending_load_is_rewind = false;
        }
    }

    pub fn files_fully_promoted(&self, storage: &HistoryStorageService) -> Vec<usize> {
        let mut result = Vec::new();
        let last_tier = (HISTORY_TIER_COUNT - 1) as u8;
        for file_index in 0..storage.files.len() {
            let file_ref = &storage.files[file_index];
            if file_ref.tier >= last_tier {
                continue;
            }
            let next_tier = file_ref.tier as usize + 1;
            let covered = match self.tier_coverage(storage, next_tier) {
                Some((coverage_oldest, coverage_newest)) => {
                    file_ref.turn_first >= coverage_oldest && file_ref.turn_last <= coverage_newest
                }
                None => false,
            };
            if covered {
                result.push(file_index);
            }
        }
        result.reverse();
        result
    }

    fn tier_coverage(
        &self,
        storage: &HistoryStorageService,
        tier_index: usize,
    ) -> Option<(u32, u32)> {
        let mut oldest: u32 = u32::MAX;
        let mut newest: u32 = 0;
        let mut found = false;
        for file_ref in &storage.files {
            if file_ref.tier == tier_index as u8 {
                if file_ref.turn_last < file_ref.turn_first {
                    continue;
                }
                if file_ref.turn_first < oldest {
                    oldest = file_ref.turn_first;
                }
                if file_ref.turn_last > newest {
                    newest = file_ref.turn_last;
                }
                found = true;
            }
        }
        let tier = &self.tiers[tier_index];
        if !tier.frames.is_empty() {
            if tier.turn_oldest < oldest {
                oldest = tier.turn_oldest;
            }
            if tier.turn_newest > newest {
                newest = tier.turn_newest;
            }
            found = true;
        }
        if found {
            Some((oldest, newest))
        } else {
            None
        }
    }

    pub fn request_load_for_rewind(&mut self, storage: &HistoryStorageService) -> bool {
        if self.pending_load_identity.is_some() {
            return true;
        }

        if self.head_turn == 0 || self.head_turn <= self.rewind_floor {
            return false;
        }
        let target_turn = self.head_turn - 1;

        let (tier, sequence) = match find_file_covering(&storage.files, target_turn) {
            Some(identity) => identity,
            None => return false,
        };
        if !storage.request_load(tier, sequence) {
            return false;
        }
        self.pending_load_identity = Some((tier, sequence));
        self.pending_load_is_rewind = true;
        true
    }

    fn apply_move(&self, record: &MoveRecord, unit_manager: &mut UnitManager) {
        let unit_index = record.unit_index as usize;
        if unit_index >= unit_manager.len() {
            return;
        }
        let level_pos = self.sector_to_level_pos_or_panic(record.sector_index, record.unit_index);
        unit_manager.tile_position[unit_index] = index_to_tile(record.tile_index);
        unit_manager.level_pos[unit_index] = level_pos;
        if let Some(direction) = facing_to_direction(record.facing) {
            unit_manager.direction[unit_index] = direction;
        }
    }

    fn sector_to_level_pos(&self, sector_index: u16) -> Option<IVec2> {
        if sector_index as usize >= self.sector_table.len() {
            return None;
        }
        Some(self.sector_table[sector_index as usize])
    }

    fn sector_to_level_pos_or_panic(&self, sector_index: u16, unit_index: u32) -> IVec2 {
        match self.sector_to_level_pos(sector_index) {
            Some(pos) => pos,
            None => panic!(
                "[History] sector {} not in table (unit {})",
                sector_index, unit_index
            ),
        }
    }

    fn apply_event(
        &self,
        event: &HistoryEvent,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
    ) {
        match event {
            HistoryEvent::Spawn { unit_index } => {
                let truncate_at = *unit_index as usize;
                if truncate_at < unit_manager.len() {
                    unit_manager.truncate_units(truncate_at);
                }
            }
            HistoryEvent::GoalPush { unit_index } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.goal_queue[i].pop();
                }
            }
            HistoryEvent::GoalPopFront {
                unit_index,
                sector_index,
                tile_index,
            } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    let level_pos = self.sector_to_level_pos_or_panic(*sector_index, *unit_index);
                    let tile = index_to_tile(*tile_index);
                    unit_manager.goal_queue[i].insert(0, GoalNode { level_pos, tile });
                }
            }
            HistoryEvent::GoalClear { unit_index, nodes } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    let mut queue = Vec::with_capacity(nodes.len());
                    for (sector_index, tile_index) in nodes {
                        let level_pos =
                            self.sector_to_level_pos_or_panic(*sector_index, *unit_index);
                        let tile = index_to_tile(*tile_index);
                        queue.push(GoalNode { level_pos, tile });
                    }
                    unit_manager.goal_queue[i] = queue;
                }
            }
            HistoryEvent::UnreachableStreak { unit_index, streak } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.goal_unreachable_streak[i] = *streak;
                }
            }
            HistoryEvent::Interactable {
                sector_index,
                tile_index,
                present,
                data,
            } => {
                let level_pos = match self.sector_to_level_pos(*sector_index) {
                    Some(pos) => pos,
                    None => panic!(
                        "[History] apply_event: sector {} not in table (interactable)",
                        sector_index
                    ),
                };
                let tile = index_to_tile(*tile_index);
                if let Some(level) = level_manager.levels.get_mut(&level_pos) {
                    if *present == 1 {
                        if let Some(interactable) = data {
                            level.interactables.insert(tile, interactable.clone());
                        }
                    } else {
                        level.interactables.remove(&tile);
                    }
                }
            }
            HistoryEvent::Death { unit_index } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.is_dead[i] = false;
                }
            }
            HistoryEvent::Revive { unit_index } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.is_dead[i] = true;
                }
            }
            HistoryEvent::MovePeriod {
                unit_index,
                move_period,
            } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.move_period[i] = *move_period;
                }
            }
            HistoryEvent::Owner { unit_index, owner } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.owner[i] = UnitOwner::from_index(*owner as usize);
                }
            }
            HistoryEvent::UnitType {
                unit_index,
                unit_type,
            } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.unit_type[i] = UnitType::from_index(*unit_type as usize);
                }
            }
            HistoryEvent::Hp { unit_index, hp } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.hp[i] = *hp;
                }
            }
            HistoryEvent::Target { unit_index, target } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.target[i] = wire_to_target(*target);
                }
            }
            HistoryEvent::Resources { resources } => {
                unit_manager.economy.resources = *resources;
            }
            HistoryEvent::Technology { technology } => {
                unit_manager.economy.technology = *technology;
            }
            HistoryEvent::Kills { unit_index, kills } => {
                let i = *unit_index as usize;
                if i < unit_manager.len() {
                    unit_manager.kills[i] = *kills;
                }
            }
        }
    }

    pub fn apply_head_state(
        &self,
        state: &HeadState,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
    ) {
        let truncate_at = state.units.len();
        if truncate_at < unit_manager.len() {
            unit_manager.truncate_units(truncate_at);
        }

        for i in 0..state.units.len() {
            let unit = &state.units[i];
            let level_pos = self.sector_to_level_pos_or_panic(unit.sector_index, i as u32);

            if i >= unit_manager.len() {
                let owner = UnitOwner::from_index(unit.owner as usize);
                let unit_type = UnitType::from_index(unit.unit_type as usize);
                let tile = index_to_tile(unit.tile_index);
                let direction = facing_to_direction(unit.facing).unwrap_or(Direction::Up);
                unit_manager.add(
                    owner,
                    unit_type,
                    level_pos,
                    tile,
                    direction,
                    unit.profile,
                    MEN_PER_UNIT,
                );
            }

            unit_manager.level_pos[i] = level_pos;
            unit_manager.tile_position[i] = index_to_tile(unit.tile_index);
            if let Some(direction) = facing_to_direction(unit.facing) {
                unit_manager.direction[i] = direction;
            }
            unit_manager.owner[i] = UnitOwner::from_index(unit.owner as usize);
            unit_manager.unit_type[i] = UnitType::from_index(unit.unit_type as usize);
            unit_manager.move_period[i] = unit.move_period;
            unit_manager.is_dead[i] = unit.is_dead != 0;
            unit_manager.goal_unreachable_streak[i] = unit.goal_unreachable_streak;
            unit_manager.hp[i] = unit.hp;
            unit_manager.target[i] = wire_to_target(unit.target);
            unit_manager.kills[i] = unit.kills;
            unit_manager.profile[i] = unit.profile;

            let mut queue = Vec::with_capacity(unit.goal_nodes.len());
            for (sector_index, tile_index) in &unit.goal_nodes {
                let node_level_pos = self.sector_to_level_pos_or_panic(*sector_index, i as u32);
                let node_tile = index_to_tile(*tile_index);
                queue.push(GoalNode {
                    level_pos: node_level_pos,
                    tile: node_tile,
                });
            }
            unit_manager.goal_queue[i] = queue;

            let mut waypoints = Vec::with_capacity(unit.waypoint_nodes.len());
            for (sector_index, tile_index) in &unit.waypoint_nodes {
                waypoints.push(GoalNode {
                    level_pos: self.sector_to_level_pos_or_panic(*sector_index, i as u32),
                    tile: index_to_tile(*tile_index),
                });
            }
            unit_manager.waypoints[i] = waypoints;
        }
        unit_manager.economy = state.economy;

        for level in level_manager.levels.values_mut() {
            level.interactables.clear();
        }
        for entry in &state.interactables {
            let level_pos = match self.sector_to_level_pos(entry.sector_index) {
                Some(pos) => pos,
                None => panic!(
                    "[History] apply_head_state: sector {} not in table (interactable)",
                    entry.sector_index
                ),
            };
            let tile = index_to_tile(entry.tile_index);
            if let Some(level) = level_manager.levels.get_mut(&level_pos) {
                level.interactables.insert(tile, entry.interactable.clone());
            }
        }

        unit_manager.rebuild_occupancy();

        for i in 0..unit_manager.len() {
            unit_manager.prev_tile_position[i] = unit_manager.tile_position[i];
            unit_manager.prev_level_pos[i] = unit_manager.level_pos[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        HISTORY_FILE_HEADER_BYTES, HISTORY_FILE_MAGIC, HISTORY_FILE_PATCH_OFFSET,
        HISTORY_FILE_TIER_OFFSET, HISTORY_FILE_TURN_FIRST_OFFSET, HISTORY_TIER_RATIO,
    };
    use crate::enums::Direction;
    use crate::history_manager::service::{
        build_bundle_container, build_manifest_bytes, build_republish_container,
        build_sector_table_bytes, history_file_ref_from_header, parse_boot_container,
        parse_bundle_container, parse_files_container, parse_history_file_bytes,
        parse_manifest_bytes, parse_sector_table_bytes,
    };
    use crate::level::interactable::{Interactable, LightIntensity, LightSource, Rubble};
    use crate::level::level_manager::LevelSet;
    use crate::unit_design::{default_profile, StructureSprite};
    use crate::unit_manager::{UnitOwner, UnitType};
    use nanoserde::DeBin;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[test]
    fn move_record_round_trip_10000() {
        let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;
        let mut buffer = Vec::new();
        let mut records = Vec::with_capacity(10_000);
        for _ in 0..10_000 {
            let record = MoveRecord {
                unit_index: xorshift(&mut state) as u32,
                sector_index: (xorshift(&mut state) % 65_536) as u16,
                tile_index: (xorshift(&mut state) % HISTORY_TILES_PER_SECTOR as u64) as u16,
                facing: (xorshift(&mut state) % 4) as u8,
            };
            record.write_to(&mut buffer);
            records.push(record);
        }
        let mut offset = 0;
        for i in 0..10_000 {
            let decoded = MoveRecord::read_from(&buffer, &mut offset).unwrap();
            assert_eq!(decoded, records[i], "mismatch at record {}", i);
        }
        assert_eq!(offset, buffer.len());
    }

    #[test]
    fn move_record_rejects_bad_tile_index() {
        let record = MoveRecord {
            unit_index: 1,
            sector_index: 0,
            tile_index: HISTORY_TILES_PER_SECTOR as u16,
            facing: 0,
        };
        let mut buffer = Vec::new();
        record.write_to(&mut buffer);
        let mut offset = 0;
        assert!(MoveRecord::read_from(&buffer, &mut offset).is_none());
    }

    #[test]
    fn move_record_packs_last_tile_and_facing_in_eight_bytes() {
        let record = MoveRecord {
            unit_index: 1,
            sector_index: 0,
            tile_index: HISTORY_TILES_PER_SECTOR as u16 - 1,
            facing: 3,
        };
        let mut buffer = Vec::new();
        record.write_to(&mut buffer);
        assert_eq!(buffer.len(), MOVE_RECORD_BYTES);
        let mut offset = 0;
        assert_eq!(MoveRecord::read_from(&buffer, &mut offset), Some(record));
    }

    #[test]
    fn frame_all_event_tags_round_trip() {
        let mut events = Vec::new();
        encode_event_spawn(&mut events, 42);
        encode_event_goal_push(&mut events, 45);
        encode_event_goal_pop_front(&mut events, 46, 7, 100);
        encode_event_goal_clear(&mut events, 47, &[(1, 2), (3, 4), (5, 6)]);
        encode_event_unreachable_streak(&mut events, 52, 3);
        encode_event_interactable(
            &mut events,
            7,
            100,
            Some(&Interactable::Rubble(Rubble {
                is_from_combat: true,
            })),
        );
        encode_event_interactable(&mut events, 8, 50, None);
        encode_event_death(&mut events, 60);
        encode_event_hp(&mut events, 61, 45);
        encode_event_target(&mut events, 62, TARGET_NONE);
        encode_event_resources(&mut events, &Resources::new(1, 2, 3, 4, 5));
        encode_event_technology(&mut events, 0b101);
        encode_event_kills(&mut events, 63, 7);

        let moves = vec![
            MoveRecord {
                unit_index: 1,
                sector_index: 0,
                tile_index: 0,
                facing: 0,
            },
            MoveRecord {
                unit_index: 2,
                sector_index: 1,
                tile_index: 575,
                facing: 3,
            },
        ];

        let mut buffer = Vec::new();
        write_frame(&mut buffer, 999, &moves, &events);

        let mut offset = 0;
        let frame = read_frame_forward(&buffer, &mut offset).unwrap();
        assert_eq!(frame.turn, 999);
        assert_eq!(frame.moves.len(), 2);
        assert_eq!(frame.moves[0].unit_index, 1);
        assert_eq!(frame.moves[1].tile_index, 575);
        assert_eq!(frame.events, events);
        assert_eq!(offset, buffer.len());

        let (frame_backward, start) = read_frame_backward(&buffer, buffer.len()).unwrap();
        assert_eq!(start, 0);
        assert_eq!(frame_backward.turn, 999);
        assert_eq!(frame_backward.moves.len(), 2);
        assert_eq!(frame_backward.events, events);

        let decoded = decode_events(&frame.events).unwrap();
        assert_eq!(decoded.len(), 13);
        match &decoded[0] {
            HistoryEvent::Spawn { unit_index } => assert_eq!(*unit_index, 42),
            _ => panic!("expected Spawn"),
        }
        match &decoded[1] {
            HistoryEvent::GoalPush { unit_index } => assert_eq!(*unit_index, 45),
            _ => panic!("expected GoalPush"),
        }
        match &decoded[2] {
            HistoryEvent::GoalPopFront {
                unit_index,
                sector_index,
                tile_index,
            } => {
                assert_eq!(*unit_index, 46);
                assert_eq!(*sector_index, 7);
                assert_eq!(*tile_index, 100);
            }
            _ => panic!("expected GoalPopFront"),
        }
        match &decoded[3] {
            HistoryEvent::GoalClear { unit_index, nodes } => {
                assert_eq!(*unit_index, 47);
                assert_eq!(*nodes, vec![(1, 2), (3, 4), (5, 6)]);
            }
            _ => panic!("expected GoalClear"),
        }
        match &decoded[4] {
            HistoryEvent::UnreachableStreak { unit_index, streak } => {
                assert_eq!(*unit_index, 52);
                assert_eq!(*streak, 3);
            }
            _ => panic!("expected UnreachableStreak"),
        }
        match &decoded[5] {
            HistoryEvent::Interactable {
                sector_index,
                tile_index,
                present,
                data,
            } => {
                assert_eq!(*sector_index, 7);
                assert_eq!(*tile_index, 100);
                assert_eq!(*present, 1);
                assert!(data.is_some());
            }
            _ => panic!("expected Interactable"),
        }
        match &decoded[6] {
            HistoryEvent::Interactable {
                sector_index,
                tile_index,
                present,
                data,
            } => {
                assert_eq!(*sector_index, 8);
                assert_eq!(*tile_index, 50);
                assert_eq!(*present, 0);
                assert!(data.is_none());
            }
            _ => panic!("expected Interactable"),
        }
        match &decoded[7] {
            HistoryEvent::Death { unit_index } => assert_eq!(*unit_index, 60),
            _ => panic!("expected Death"),
        }
        match &decoded[8] {
            HistoryEvent::Hp { unit_index, hp } => {
                assert_eq!(*unit_index, 61);
                assert_eq!(*hp, 45);
            }
            _ => panic!("expected Hp"),
        }
        match &decoded[9] {
            HistoryEvent::Target { unit_index, target } => {
                assert_eq!(*unit_index, 62);
                assert_eq!(*target, TARGET_NONE);
            }
            _ => panic!("expected Target"),
        }
        match &decoded[10] {
            HistoryEvent::Resources { resources } => {
                assert_eq!(*resources, Resources::new(1, 2, 3, 4, 5));
            }
            _ => panic!("expected Resources"),
        }
        match &decoded[11] {
            HistoryEvent::Technology { technology } => assert_eq!(*technology, 0b101),
            _ => panic!("expected Technology"),
        }
        match &decoded[12] {
            HistoryEvent::Kills { unit_index, kills } => {
                assert_eq!(*unit_index, 63);
                assert_eq!(*kills, 7);
            }
            _ => panic!("expected Kills"),
        }
    }

    #[test]
    fn frame_empty_round_trip() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, 0, &[], &[]);
        assert_eq!(buffer.len(), 16);

        let mut offset = 0;
        let frame = read_frame_forward(&buffer, &mut offset).unwrap();
        assert_eq!(frame.turn, 0);
        assert_eq!(frame.moves.len(), 0);
        assert_eq!(frame.events.len(), 0);
        assert_eq!(offset, 16);
    }

    #[test]
    fn frame_backward_multiple() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, 10, &[], &[]);
        write_frame(&mut buffer, 11, &[], &[]);
        write_frame(&mut buffer, 12, &[], &[]);

        let (frame3, start3) = read_frame_backward(&buffer, buffer.len()).unwrap();
        assert_eq!(frame3.turn, 12);
        let (frame2, start2) = read_frame_backward(&buffer, start3).unwrap();
        assert_eq!(frame2.turn, 11);
        let (frame1, start1) = read_frame_backward(&buffer, start2).unwrap();
        assert_eq!(frame1.turn, 10);
        assert_eq!(start1, 0);
    }

    #[test]
    fn frame_rejects_truncated() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, 5, &[], &[]);
        buffer.truncate(buffer.len() - 1);
        let mut offset = 0;
        assert!(read_frame_forward(&buffer, &mut offset).is_none());
    }

    #[test]
    fn oversized_frame_round_trip() {
        let move_count = 35_000;
        let mut moves = Vec::with_capacity(move_count);
        for i in 0..move_count {
            moves.push(MoveRecord {
                unit_index: i as u32,
                sector_index: (i % 65_536) as u16,
                tile_index: (i % HISTORY_TILES_PER_SECTOR as usize) as u16,
                facing: (i % 4) as u8,
            });
        }
        let mut events = Vec::new();
        encode_event_spawn(&mut events, 999);

        let frame_size = frame_byte_size(moves.len(), events.len());
        assert!(frame_size > HISTORY_CHUNK_BYTES);

        let mut buffer = Vec::new();
        write_frame(&mut buffer, 77, &moves, &events);

        let mut offset = 0;
        let frame = read_frame_forward(&buffer, &mut offset).unwrap();
        assert_eq!(frame.turn, 77);
        assert_eq!(frame.moves.len(), move_count);
        assert_eq!(frame.moves[0].unit_index, 0);
        assert_eq!(
            frame.moves[move_count - 1].unit_index,
            (move_count - 1) as u32
        );
        assert_eq!(frame.events, events);

        let (frame_backward, start) = read_frame_backward(&buffer, buffer.len()).unwrap();
        assert_eq!(start, 0);
        assert_eq!(frame_backward.turn, 77);
        assert_eq!(frame_backward.moves.len(), move_count);

        let decoded = decode_events(&frame.events).unwrap();
        assert_eq!(decoded.len(), 1);
        match &decoded[0] {
            HistoryEvent::Spawn { unit_index } => assert_eq!(*unit_index, 999),
            _ => panic!("expected Spawn"),
        }
    }

    #[test]
    fn chunk_header_round_trip() {
        let mut buffer = Vec::new();
        write_chunk_header(&mut buffer, 100, 200, 1024, 50, 2);
        assert_eq!(buffer.len(), CHUNK_HEADER_BYTES);

        let mut offset = 0;
        let header = read_chunk_header(&buffer, &mut offset).unwrap();
        assert_eq!(header.turn_first, 100);
        assert_eq!(header.turn_last, 200);
        assert_eq!(header.used_bytes, 1024);
        assert_eq!(header.frame_count, 50);
        assert_eq!(header.tier, 2);
        assert_eq!(offset, CHUNK_HEADER_BYTES);
    }

    #[test]
    fn chunk_header_rejects_bad_magic() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&0xDEAD_BEEF_u32.to_le_bytes());
        buffer.extend_from_slice(&[0u8; 20]);
        let mut offset = 0;
        assert!(read_chunk_header(&buffer, &mut offset).is_none());
    }

    #[test]
    fn sector_table_derivation() {
        let mut levels = HashMap::new();
        levels.insert(
            IVec2::new(1, 0),
            Level::new(
                "a",
                "A",
                IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
            ),
        );
        levels.insert(
            IVec2::new(0, 0),
            Level::new(
                "b",
                "B",
                IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
            ),
        );
        levels.insert(
            IVec2::new(0, 1),
            Level::new(
                "c",
                "C",
                IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
            ),
        );
        levels.insert(
            IVec2::new(-1, -1),
            Level::new(
                "d",
                "D",
                IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
            ),
        );

        let (table, lookup) = derive_sector_table(&levels);
        assert_eq!(table.len(), 4);
        assert_eq!(table[0], IVec2::new(-1, -1));
        assert_eq!(table[1], IVec2::new(0, 0));
        assert_eq!(table[2], IVec2::new(1, 0));
        assert_eq!(table[3], IVec2::new(0, 1));
        assert_eq!(lookup[&IVec2::new(-1, -1)], 0);
        assert_eq!(lookup[&IVec2::new(0, 0)], 1);
        assert_eq!(lookup[&IVec2::new(1, 0)], 2);
        assert_eq!(lookup[&IVec2::new(0, 1)], 3);
    }

    #[test]
    fn head_state_round_trip_1000_units() {
        let mut levels = HashMap::new();
        let mut level = Level::new(
            "test",
            "Test",
            IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
        );
        level.interactables.insert(
            IVec2::new(3, 2),
            Interactable::LightSource(LightSource::new(LightIntensity::Medium)),
        );
        level
            .interactables
            .insert(IVec2::new(7, 4), Interactable::Rubble(Rubble::default()));
        levels.insert(IVec2::new(0, 0), level);

        let (_sector_table, sector_lookup) = derive_sector_table(&levels);

        let mut unit_manager = UnitManager::new();
        for i in 0..1000 {
            let tile_x = (i % DEFAULT_LEVEL_TILES_X as usize) as i32;
            let tile_y =
                ((i / DEFAULT_LEVEL_TILES_X as usize) % DEFAULT_LEVEL_TILES_Y as usize) as i32;
            let direction = match i % 4 {
                0 => Direction::Up,
                1 => Direction::Right,
                2 => Direction::Down,
                _ => Direction::Left,
            };
            let unit_type = if i % 10 == 0 {
                UnitType::Structure
            } else {
                UnitType::Soldier
            };
            unit_manager.add(
                UnitOwner::P0,
                unit_type,
                IVec2::new(0, 0),
                IVec2::new(tile_x, tile_y),
                direction,
                default_profile(unit_type, StructureSprite::TowerWood),
                (i % 6 + 1) as u16,
            );
        }

        for i in 0..1000 {
            if i % 5 == 0 {
                unit_manager.target[i] = Some((i + 1) % 1000);
                unit_manager.waypoints[i].push(GoalNode {
                    level_pos: IVec2::new(0, 0),
                    tile: IVec2::new(1, 2),
                });
            }
            if i % 7 == 0 {
                unit_manager.is_dead[i] = true;
            }
            if i % 3 == 0 {
                unit_manager.goal_unreachable_streak[i] = (i % 4) as u8;
            }
            if i % 13 == 0 {
                unit_manager.kills[i] = i as u16;
            }
            if i % 11 == 0 {
                unit_manager.goal_queue[i].push(GoalNode {
                    level_pos: IVec2::new(0, 0),
                    tile: IVec2::new(
                        (i % DEFAULT_LEVEL_TILES_X as usize) as i32,
                        (i % DEFAULT_LEVEL_TILES_Y as usize) as i32,
                    ),
                });
                unit_manager.goal_queue[i].push(GoalNode {
                    level_pos: IVec2::new(0, 0),
                    tile: IVec2::new(
                        ((i + 1) % DEFAULT_LEVEL_TILES_X as usize) as i32,
                        ((i + 1) % DEFAULT_LEVEL_TILES_Y as usize) as i32,
                    ),
                });
            }
        }

        unit_manager.economy.resources = Resources::new(3, 40, 50, 6, 7);
        unit_manager.economy.technology = 0b1011;
        let encoded = encode_head_state(
            12345,
            &unit_manager,
            &sector_lookup,
            &levels,
            &[1, 2, 3],
            77,
        );
        let decoded = decode_head_state(&encoded).unwrap();

        assert_eq!(decoded.turn, 12345);
        assert_eq!(decoded.units.len(), 1000);
        assert_eq!(decoded.interactables.len(), 2);

        for i in 0..1000 {
            let unit = &decoded.units[i];
            assert_eq!(unit.sector_index, 0);
            let expected_tile_index = tile_to_index(unit_manager.tile_position[i]);
            assert_eq!(
                unit.tile_index, expected_tile_index,
                "tile mismatch at unit {}",
                i
            );
            let expected_facing = direction_to_facing(unit_manager.direction[i]);
            assert_eq!(
                unit.facing, expected_facing,
                "facing mismatch at unit {}",
                i
            );
            assert_eq!(unit.owner, unit_manager.owner[i].to_index() as u8);
            assert_eq!(unit.unit_type, unit_manager.unit_type[i].to_index() as u8);
            assert_eq!(unit.move_period, unit_manager.move_period[i]);
            assert_eq!(unit.hp, unit_manager.hp[i], "hp mismatch at unit {}", i);
            assert_eq!(unit.target, target_to_wire(unit_manager.target[i]));
            assert_eq!(unit.profile, unit_manager.profile[i]);
            assert_eq!(unit.kills, unit_manager.kills[i]);
            assert_eq!(unit.waypoint_nodes.len(), unit_manager.waypoints[i].len());

            let expected_dead = if unit_manager.is_dead[i] { 1u8 } else { 0u8 };
            assert_eq!(
                unit.is_dead, expected_dead,
                "is_dead mismatch at unit {}",
                i
            );
            assert_eq!(
                unit.goal_unreachable_streak, unit_manager.goal_unreachable_streak[i],
                "streak mismatch at unit {}",
                i
            );

            let expected_goal_count = unit_manager.goal_queue[i].len();
            assert_eq!(
                unit.goal_nodes.len(),
                expected_goal_count,
                "goal count mismatch at unit {}",
                i
            );
            for j in 0..expected_goal_count {
                let node = &unit_manager.goal_queue[i][j];
                let expected_sector = sector_lookup[&node.level_pos];
                let expected_tile = tile_to_index(node.tile);
                assert_eq!(unit.goal_nodes[j], (expected_sector, expected_tile));
            }
        }

        assert_eq!(decoded.interactables[0].sector_index, 0);
        assert_eq!(
            decoded.interactables[0].tile_index,
            tile_to_index(IVec2::new(3, 2))
        );
        assert_eq!(decoded.economy, unit_manager.economy);
        assert_eq!(decoded.player_state, vec![1, 2, 3]);
        assert_eq!(decoded.rewind_floor, 77);
        assert_eq!(decoded.interactables[1].sector_index, 0);
        assert_eq!(
            decoded.interactables[1].tile_index,
            tile_to_index(IVec2::new(7, 4))
        );
    }

    #[test]
    fn head_state_rejects_bad_magic() {
        let bytes = [0u8; 64];
        assert!(decode_head_state(&bytes).is_none());
    }

    #[test]
    fn head_state_rejects_bad_tile_index() {
        let mut output = Vec::new();
        output.extend_from_slice(&HISTORY_STATE_MAGIC);
        output.extend_from_slice(&(DATA_VERSION as u32).to_le_bytes());
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&1u32.to_le_bytes());
        output.extend_from_slice(&0u16.to_le_bytes());
        output.extend_from_slice(&(HISTORY_TILES_PER_SECTOR as u16).to_le_bytes());
        output.extend_from_slice(&[0, 0, 0, 1, 0, 0]);
        output.extend_from_slice(&60u16.to_le_bytes());
        output.extend_from_slice(&u32::MAX.to_le_bytes());
        output.extend_from_slice(&0u16.to_le_bytes());
        output.extend_from_slice(&0u16.to_le_bytes());
        assert!(decode_head_state(&output).is_none());
    }

    #[test]
    fn tile_index_conversion() {
        for y in 0..DEFAULT_LEVEL_TILES_Y {
            for x in 0..DEFAULT_LEVEL_TILES_X {
                let tile = IVec2::new(x, y);
                let index = tile_to_index(tile);
                assert!((index as u32) < HISTORY_TILES_PER_SECTOR);
                let back = index_to_tile(index);
                assert_eq!(back, tile);
            }
        }
    }

    #[test]
    fn direction_facing_conversion() {
        assert_eq!(direction_to_facing(Direction::Up), 0);
        assert_eq!(direction_to_facing(Direction::Right), 1);
        assert_eq!(direction_to_facing(Direction::Down), 2);
        assert_eq!(direction_to_facing(Direction::Left), 3);
        assert_eq!(facing_to_direction(0), Some(Direction::Up));
        assert_eq!(facing_to_direction(1), Some(Direction::Right));
        assert_eq!(facing_to_direction(2), Some(Direction::Down));
        assert_eq!(facing_to_direction(3), Some(Direction::Left));
        assert_eq!(facing_to_direction(4), None);
    }

    #[test]
    fn manifest_round_trip() {
        let save_id: u128 = 0x0102_0304_0506_0708_090A_0B0C_0D0E_0F10;
        let bytes = build_manifest_bytes(save_id, 17);
        let (parsed_id, parsed_count) = parse_manifest_bytes(&bytes).unwrap();
        assert_eq!(parsed_id, save_id);
        assert_eq!(parsed_count, 17);

        assert!(parse_manifest_bytes(&bytes[..bytes.len() - 1]).is_none());

        let mut bad_magic = bytes.clone();
        bad_magic[0] = 0;
        assert!(parse_manifest_bytes(&bad_magic).is_none());
    }

    #[test]
    fn sector_table_round_trip() {
        let table = vec![IVec2::new(0, 0), IVec2::new(-3, 5), IVec2::new(12, -1)];
        let bytes = build_sector_table_bytes(&table);
        let parsed = parse_sector_table_bytes(&bytes).unwrap();
        assert_eq!(parsed, table);
    }

    #[test]
    fn history_file_parse_round_trip() {
        let tier: u8 = 0;

        let mut frame_data = Vec::new();
        let moves = vec![MoveRecord {
            unit_index: 1,
            sector_index: 0,
            tile_index: 5,
            facing: 2,
        }];
        write_frame(&mut frame_data, 100, &moves, &[]);
        write_frame(&mut frame_data, 101, &[], &[]);
        let used_bytes = frame_data.len() as u32;

        let mut chunk = Vec::new();
        write_chunk_header(&mut chunk, 100, 101, used_bytes, 2, tier as u32);
        chunk.extend_from_slice(&frame_data);

        let mut file = Vec::new();
        file.extend_from_slice(&HISTORY_FILE_MAGIC);
        file.extend_from_slice(&(DATA_VERSION as u32).to_le_bytes());
        file.extend_from_slice(&(tier as u32).to_le_bytes());
        file.extend_from_slice(&HISTORY_TIER_RATIO[tier as usize].to_le_bytes());
        file.extend_from_slice(&100u32.to_le_bytes());
        file.extend_from_slice(&101u32.to_le_bytes());
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        file.extend_from_slice(&chunk);

        let mut bad_version = file.clone();
        bad_version[8..12].copy_from_slice(&999u32.to_le_bytes());
        assert!(parse_history_file_bytes(&bad_version, 7, tier).is_none());

        let loaded = parse_history_file_bytes(&file, 7, tier).unwrap();
        assert_eq!(loaded.sequence, 7);
        assert_eq!(loaded.tier, tier);
        assert_eq!(loaded.chunks.len(), 1);
        assert_eq!(loaded.chunks[0].turn_first, 100);
        assert_eq!(loaded.chunks[0].turn_last, 101);
        assert_eq!(loaded.chunks[0].frame_count, 2);

        let chunk_bytes = &loaded.chunks[0].bytes;
        let mut offset = CHUNK_HEADER_BYTES;
        let frame0 = read_frame_forward(chunk_bytes, &mut offset).unwrap();
        assert_eq!(frame0.turn, 100);
        assert_eq!(frame0.moves.len(), 1);
        assert_eq!(frame0.moves[0].tile_index, 5);
        let frame1 = read_frame_forward(chunk_bytes, &mut offset).unwrap();
        assert_eq!(frame1.turn, 101);
        assert_eq!(frame1.moves.len(), 0);
    }

    #[test]
    fn boot_container_round_trip() {
        let manifest = vec![1u8, 2, 3];
        let sectors = vec![4u8, 5];
        let head = vec![6u8, 7, 8, 9];
        let head_prev = vec![13u8, 14];
        let bundle = vec![10u8];
        let files = vec![0u8, 0, 0, 0];
        let snapshots = vec![15u8, 16, 17];
        let snapshots_prev = vec![18u8];

        let mut blob = Vec::new();
        blob.push(1u8);
        for section in [
            &manifest,
            &sectors,
            &head,
            &head_prev,
            &bundle,
            &files,
            &snapshots,
            &snapshots_prev,
        ] {
            blob.extend_from_slice(&(section.len() as u32).to_le_bytes());
            blob.extend_from_slice(section);
        }

        let probe = parse_boot_container(&blob).unwrap();
        assert!(probe.has_save);
        assert_eq!(probe.manifest_bytes, manifest);
        assert_eq!(probe.sectors_bytes, sectors);
        assert_eq!(probe.head_bytes, head);
        assert_eq!(probe.head_prev_bytes, head_prev);
        assert_eq!(probe.bundle_bytes, bundle);
        assert!(probe.files.is_empty());
        assert_eq!(probe.snapshots_bytes, snapshots);
        assert_eq!(probe.snapshots_prev_bytes, snapshots_prev);

        let fresh = vec![0u8];
        let probe = parse_boot_container(&fresh).unwrap();
        assert!(!probe.has_save);
        assert!(probe.manifest_bytes.is_empty());
        assert!(probe.files.is_empty());
    }

    #[test]
    fn bundle_container_round_trip() {
        let mut levels = HashMap::new();
        levels.insert(
            IVec2::new(0, 0),
            Level::new(
                "a",
                "A",
                IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
            ),
        );
        levels.insert(
            IVec2::new(1, 2),
            Level::new(
                "b",
                "B",
                IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
            ),
        );
        let level_set = LevelSet {
            level_grid: HashMap::new(),
            initial_level_name: String::from("a"),
        };

        let bytes = build_bundle_container(&levels, &Some(level_set));
        let entries = parse_bundle_container(&bytes).unwrap();

        assert_eq!(entries.len(), 3);
        let mut names: Vec<&str> = Vec::new();
        for &(name, _) in &entries {
            names.push(name);
        }
        names.sort();
        assert_eq!(names, ["0_0.asb", "1_2.asb", "level_set.asb"]);
        for &(_, data) in &entries {
            let mut offset = 0;
            let version = usize::de_bin(&mut offset, data).unwrap();
            assert_eq!(version, DATA_VERSION);
        }
    }

    #[test]
    fn files_container_ref_from_header() {
        let tier: u8 = 2;
        let mut header = [0u8; HISTORY_FILE_HEADER_BYTES];
        header[0..8].copy_from_slice(&HISTORY_FILE_MAGIC);
        header[8..12].copy_from_slice(&(DATA_VERSION as u32).to_le_bytes());
        header[HISTORY_FILE_TIER_OFFSET..HISTORY_FILE_TIER_OFFSET + 4]
            .copy_from_slice(&(tier as u32).to_le_bytes());
        header[16..20].copy_from_slice(&HISTORY_TIER_RATIO[tier as usize].to_le_bytes());
        header[HISTORY_FILE_TURN_FIRST_OFFSET..HISTORY_FILE_TURN_FIRST_OFFSET + 4]
            .copy_from_slice(&1000u32.to_le_bytes());
        header[HISTORY_FILE_PATCH_OFFSET..HISTORY_FILE_PATCH_OFFSET + 4]
            .copy_from_slice(&1999u32.to_le_bytes());

        let file_ref =
            history_file_ref_from_header("t2-0000001000-000007.thf", 4096, &header).unwrap();
        assert_eq!(file_ref.tier, 2);
        assert_eq!(file_ref.sequence, 7);
        assert_eq!(file_ref.turn_first, 1000);
        assert_eq!(file_ref.turn_last, 1999);
        assert_eq!(file_ref.byte_length, 4096);
    }

    #[test]
    fn files_container_round_trip() {
        let tier: u8 = 1;
        let mut header = [0u8; HISTORY_FILE_HEADER_BYTES];
        header[0..8].copy_from_slice(&HISTORY_FILE_MAGIC);
        header[8..12].copy_from_slice(&(DATA_VERSION as u32).to_le_bytes());
        header[HISTORY_FILE_TIER_OFFSET..HISTORY_FILE_TIER_OFFSET + 4]
            .copy_from_slice(&(tier as u32).to_le_bytes());
        header[16..20].copy_from_slice(&HISTORY_TIER_RATIO[tier as usize].to_le_bytes());
        header[HISTORY_FILE_TURN_FIRST_OFFSET..HISTORY_FILE_TURN_FIRST_OFFSET + 4]
            .copy_from_slice(&500u32.to_le_bytes());
        header[HISTORY_FILE_PATCH_OFFSET..HISTORY_FILE_PATCH_OFFSET + 4]
            .copy_from_slice(&599u32.to_le_bytes());

        let name = b"t1-0000000500-000003.thf";
        let mut blob = Vec::new();
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.extend_from_slice(&(name.len() as u16).to_le_bytes());
        blob.extend_from_slice(name);
        blob.extend_from_slice(&2048u32.to_le_bytes());
        blob.extend_from_slice(&header);

        let files = parse_files_container(&blob).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].tier, 1);
        assert_eq!(files[0].sequence, 3);
        assert_eq!(files[0].turn_first, 500);
        assert_eq!(files[0].turn_last, 599);
        assert_eq!(files[0].byte_length, 2048);
    }

    #[test]
    fn republish_container_round_trip() {
        let chunks = vec![
            Chunk {
                bytes: vec![1, 2, 3],
                turn_first: 10,
                turn_last: 19,
                used_bytes: 3,
                frame_count: 1,
            },
            Chunk {
                bytes: vec![4, 5],
                turn_first: 20,
                turn_last: 29,
                used_bytes: 2,
                frame_count: 1,
            },
        ];
        let blob = build_republish_container(&chunks);

        let count = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
        assert_eq!(count, 2);
        let mut offset = 4;

        let len0 = u32::from_le_bytes([
            blob[offset],
            blob[offset + 1],
            blob[offset + 2],
            blob[offset + 3],
        ]) as usize;
        offset += 4;
        assert_eq!(len0, 3);
        assert_eq!(&blob[offset..offset + len0], &[1, 2, 3]);
        offset += len0;

        let len1 = u32::from_le_bytes([
            blob[offset],
            blob[offset + 1],
            blob[offset + 2],
            blob[offset + 3],
        ]) as usize;
        offset += 4;
        assert_eq!(len1, 2);
        assert_eq!(&blob[offset..offset + len1], &[4, 5]);
        offset += len1;

        assert_eq!(offset, blob.len());
    }
}
