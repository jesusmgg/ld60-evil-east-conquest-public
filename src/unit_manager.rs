use crate::{
    assets::{
        crewed_machine_texture_index, machine_texture_index, man_texture_index,
        sprite_layout::{
            chassis_flame_bases, chassis_impact_points, is_far_crew, machine_weapon_impact_points,
            man_armor_impact_points, man_weapon_impact_points, shielding_impact_points,
            structure_flame_bases, structure_impact_points, ImpactPoints, MACHINE_CREW_SLOTS,
            MAN_BASE_IMPACT_POINTS,
        },
        AssetManager,
    },
    audio_manager::AudioManager,
    camera_manager::CameraManager,
    constants::{
        CULL_MARGIN_PX, DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y, TILE_SIZE_X, TILE_SIZE_Y,
        TURN_PERIOD,
    },
    debug::DebugLog,
    debug_control_manager::DebugControlManager,
    debug_overlay_manager::DebugOverlayManager,
    enums::Direction,
    flame::{draw_flame_at_base, DamageFlame, FLAME_ANIMATION_FPS},
    game_data::proxy::{IVec2Proxy, VecIVec2Proxy},
    gpu_path_manager::GpuPathManager,
    history_manager::HistoryManager,
    level::{
        interactable::Gate,
        level_editor_manager::LevelEditorManager,
        level_manager::{sector_in_range, sector_range_for_world_rect, LevelManager},
        tile::is_tile_obstacle,
        Interactable,
    },
    loading_manager::LoadingManager,
    minimap_manager::MinimapManager,
    player_manager::PlayerManager,
    selector_manager::{is_mouse_over_overlay, SelectorManager},
    sfx::{unit_sfx, SfxEvent},
    simple_enum_serbin,
    styles::Styles,
    text::draw_scaled_text,
    ui_draw::{draw_tooltip, virtual_mouse_position},
    unit_design::{
        crew_scaled_period, default_profile, design_cost_for_men, is_design_available, men_from_hp,
        sell_refund, structure_profile, unit_max_hp, unit_move_period, unit_name, unit_type_for,
        Armor, Chassis, Economy, Resources, Shielding, StructureSprite, Technology, UnitDesign,
        UnitProfile, Weapon, MEN_PER_UNIT,
    },
};
use macroquad::prelude::*;
use nanoserde::{DeBin, SerBin};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::mem;

const RESOLVE_ROUNDS_MAX: usize = 5;

const GOAL_UNREACHABLE_GIVE_UP: u8 = 3;

pub const ARMY_UNIT_MAX: usize = 16;

const TRANSFER_HINT_SECONDS: f32 = 3.0;
const TRANSFER_PULSE_SPEED: f32 = 4.0;
const TRANSFER_BLOCKED_TEXT: &str = "Units are far away or fighting";

const MAN_SPRITE_SIZE: f32 = 8.0;
const MAN_BOX_LEFT: f32 = 2.0;
const MAN_BOX_RIGHT: f32 = 7.0;
const MAN_BOX_TOP: f32 = 1.0;
const MAN_BOX_HEIGHT: f32 = 7.0;
const MAN_COLUMN_STEP: f32 = 5.0;
const MAN_ROW_STEP: f32 = 8.0;
const MAN_ROWS: u16 = 2;
const MAN_COLUMNS: u16 = MEN_PER_UNIT / MAN_ROWS;
const _: () = assert!(MACHINE_CREW_SLOTS.len() == MEN_PER_UNIT as usize);

pub struct AttackEvent {
    pub attacker_i: usize,
    pub target_i: usize,
    pub target_men: u16,
    pub weapon: Weapon,
    pub impact_sfx_start: usize,
    pub impact_sfx_end: usize,
}

pub struct GatherEvent {
    pub level_pos: IVec2,
    pub tile: IVec2,
    pub resources: Resources,
    pub technology: Option<Technology>,
}

simple_enum_serbin!(UnitOwner, ALL_UNIT_OWNERS {
    P0 => 0,
    P1 => 100,
});

simple_enum_serbin!(UnitType, ALL_UNIT_TYPES {
    Soldier => 0,
    Machine => 10,
    Structure => 100,
});

impl UnitType {
    pub fn is_structure(&self) -> bool {
        *self == UnitType::Structure
    }
}

#[derive(Clone, SerBin, DeBin, PartialEq)]
pub struct UnitSpawnData {
    pub owner: UnitOwner,
    pub unit_type: UnitType,
    #[nserde(proxy = "IVec2Proxy")]
    pub tile_pos: IVec2,
    pub direction: Direction,
    pub profile: UnitProfile,
    #[nserde(proxy = "VecIVec2Proxy")]
    pub waypoints: Vec<IVec2>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct GoalNode {
    pub level_pos: IVec2,
    pub tile: IVec2,
}

#[derive(Clone, Copy)]
pub(crate) enum Intent {
    Step {
        direction: Direction,
        next: GoalNode,
    },
    Crossing {
        direction: Direction,
        next: GoalNode,
        entry_direction: Direction,
    },
}

#[derive(Clone, Copy)]
pub struct MoveOrder {
    pub unit_i: usize,
    pub target: GoalNode,
    pub is_queued: bool,
}

#[derive(Clone, Copy)]
pub struct TrainOrder {
    pub design: UnitDesign,
    pub level_pos: IVec2,
    pub tile: IVec2,
}

#[derive(Clone, Copy)]
pub struct TargetOrder {
    pub unit_i: usize,
    pub target: Option<usize>,
}

pub fn default_unit_spawn(
    owner: UnitOwner,
    unit_type: UnitType,
    structure_sprite: StructureSprite,
) -> UnitSpawnData {
    UnitSpawnData {
        owner,
        unit_type,
        tile_pos: IVec2::ZERO,
        direction: Direction::Down,
        profile: default_profile(unit_type, structure_sprite),
        waypoints: Vec::new(),
    }
}

pub struct Occupancy(HashMap<(IVec2, IVec2), usize>);

impl Occupancy {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn insert(&mut self, level_pos: IVec2, tile_pos: IVec2, unit_i: usize) {
        self.0.insert((level_pos, tile_pos), unit_i);
    }

    pub fn contains(&self, level_pos: IVec2, tile_pos: IVec2) -> bool {
        self.0.contains_key(&(level_pos, tile_pos))
    }

    pub fn remove(&mut self, level_pos: IVec2, tile_pos: IVec2) {
        self.0.remove(&(level_pos, tile_pos));
    }

    pub fn get(&self, level_pos: IVec2, tile_pos: IVec2) -> Option<usize> {
        self.0.get(&(level_pos, tile_pos)).copied()
    }
}

pub(crate) fn snap_to_2px(pos: Vec2) -> Vec2 {
    Vec2::new(
        ((pos.x - 1.0) / 2.0).round() * 2.0 + 1.0,
        ((pos.y - 1.0) / 2.0).round() * 2.0 + 1.0,
    )
}

fn spawn_sort_order(
    a: &(IVec2, IVec2, &UnitSpawnData),
    b: &(IVec2, IVec2, &UnitSpawnData),
) -> std::cmp::Ordering {
    (a.0.y, a.0.x, a.1.y, a.1.x).cmp(&(b.0.y, b.0.x, b.1.y, b.1.x))
}

pub fn man_armor_slot(armor: Armor) -> usize {
    match armor {
        Armor::NoArmor => 0,
        Armor::Gambeson => 1,
        Armor::Leather => 2,
        Armor::ChainMail => 3,
        Armor::Plate => 4,
    }
}

pub fn man_weapon_slot(weapon: Weapon) -> usize {
    match weapon {
        Weapon::Unarmed
        | Weapon::WoodRam
        | Weapon::IronRam
        | Weapon::Catapult
        | Weapon::Trebuchet
        | Weapon::Ballista
        | Weapon::Cannon
        | Weapon::OrganGun => 0,
        Weapon::Dagger => 1,
        Weapon::Sword => 2,
        Weapon::SwordAndShield => 3,
        Weapon::Spear => 4,
        Weapon::Javelin => 5,
        Weapon::Bow => 6,
        Weapon::Crossbow => 7,
        Weapon::Grenades => 8,
        Weapon::HandCannon => 9,
    }
}

fn man_color_slot(owner: UnitOwner) -> usize {
    match owner {
        UnitOwner::P0 => 0,
        UnitOwner::P1 => 1,
    }
}

pub fn man_texture(
    assets: &AssetManager,
    armor: Armor,
    weapon: Weapon,
    owner: UnitOwner,
) -> &Texture2D {
    let index = man_texture_index(
        man_armor_slot(armor),
        man_weapon_slot(weapon),
        man_color_slot(owner),
    );
    &assets.texture_men[index]
}

fn draw_sprite(texture: &Texture2D, pos: Vec2, tint: Color, flip_x: bool) {
    draw_texture_ex(
        texture,
        pos.x,
        pos.y,
        tint,
        DrawTextureParams {
            flip_x,
            ..Default::default()
        },
    );
}

fn man_slot(k: u16, men: u16, flip_x: bool) -> Vec2 {
    let column_from_front = (k / MAN_ROWS) as f32;
    let row = (k % MAN_ROWS) as f32;
    let mut x = -MAN_BOX_LEFT + ((MAN_COLUMNS - 1) as f32 - column_from_front) * MAN_COLUMN_STEP;
    if flip_x {
        x = -(MAN_SPRITE_SIZE - 1.0 - MAN_BOX_RIGHT) + column_from_front * MAN_COLUMN_STEP;
    }
    let mut y = -MAN_BOX_TOP + row * MAN_ROW_STEP;
    if men == 1 {
        y = ((TILE_SIZE_Y - MAN_BOX_HEIGHT) / 2.0).floor() - MAN_BOX_TOP;
    }
    vec2(x, y)
}

pub fn draw_soldier_group(
    assets: &AssetManager,
    design: &UnitDesign,
    owner: UnitOwner,
    world_pos: Vec2,
    men: u16,
    tint: Color,
    flip_x: bool,
) {
    let texture = man_texture(assets, design.armor, design.weapon, owner);
    for k in 0..men {
        draw_sprite(texture, world_pos + man_slot(k, men, flip_x), tint, flip_x);
    }
}

pub fn machine_chassis_slot(chassis: Chassis) -> usize {
    match chassis {
        Chassis::Carried => 0,
        Chassis::LogRollers => 1,
        Chassis::SolidWheels => 2,
        Chassis::IronRimmedWheels => 3,
        Chassis::SpokedWheels => 4,
    }
}

pub fn machine_shielding_slot(shielding: Shielding) -> usize {
    match shielding {
        Shielding::NoShielding => 0,
        Shielding::WoodShielding => 1,
        Shielding::IronShielding => 2,
        Shielding::ReinforcedShielding => 3,
    }
}

pub fn machine_weapon_slot(weapon: Weapon) -> usize {
    match weapon {
        Weapon::Unarmed
        | Weapon::Dagger
        | Weapon::Sword
        | Weapon::SwordAndShield
        | Weapon::Spear
        | Weapon::Javelin
        | Weapon::Bow
        | Weapon::Crossbow
        | Weapon::Grenades
        | Weapon::HandCannon => 0,
        Weapon::WoodRam => 1,
        Weapon::IronRam => 2,
        Weapon::Catapult => 3,
        Weapon::Trebuchet => 4,
        Weapon::Ballista => 5,
        Weapon::Cannon => 6,
        Weapon::OrganGun => 7,
    }
}

fn machine_design_texture_index(design: &UnitDesign) -> usize {
    machine_texture_index(
        machine_chassis_slot(design.chassis),
        machine_shielding_slot(design.shielding),
        machine_weapon_slot(design.weapon),
    )
}

fn machine_texture<'a>(assets: &'a AssetManager, design: &UnitDesign) -> &'a Texture2D {
    &assets.texture_machines[machine_design_texture_index(design)]
}

pub fn crewed_machine_texture<'a>(
    assets: &'a AssetManager,
    design: &UnitDesign,
    owner: UnitOwner,
) -> &'a Texture2D {
    let index =
        crewed_machine_texture_index(machine_design_texture_index(design), man_color_slot(owner));
    &assets.texture_crewed_machines[index]
}

fn machine_crew_slot(k: u16, flip_x: bool) -> Vec2 {
    let mut slot = MACHINE_CREW_SLOTS[k as usize];
    if flip_x {
        slot.x = TILE_SIZE_X - MAN_SPRITE_SIZE - slot.x;
    }
    slot
}

pub fn draw_machine(
    assets: &AssetManager,
    design: &UnitDesign,
    owner: UnitOwner,
    world_pos: Vec2,
    men: u16,
    tint: Color,
    flip_x: bool,
) {
    if men == MEN_PER_UNIT {
        draw_sprite(
            crewed_machine_texture(assets, design, owner),
            world_pos,
            tint,
            flip_x,
        );
        return;
    }
    let man = man_texture(assets, Armor::NoArmor, Weapon::Unarmed, owner);
    for k in 0..men {
        if is_far_crew(k) {
            draw_sprite(man, world_pos + machine_crew_slot(k, flip_x), tint, flip_x);
        }
    }
    draw_sprite(machine_texture(assets, design), world_pos, tint, flip_x);
    for k in 0..men {
        if !is_far_crew(k) {
            draw_sprite(man, world_pos + machine_crew_slot(k, flip_x), tint, flip_x);
        }
    }
}

pub fn structure_texture(assets: &AssetManager, structure_sprite: StructureSprite) -> &Texture2D {
    &assets.texture_structures[structure_sprite as usize]
}

pub fn draw_structure(
    assets: &AssetManager,
    structure_sprite: StructureSprite,
    world_pos: Vec2,
    tint: Color,
) {
    draw_sprite(
        structure_texture(assets, structure_sprite),
        world_pos,
        tint,
        false,
    );
}

struct ImpactSearch {
    source: Vec2,
    is_from_left: bool,
    flip_x: bool,
    best: Option<Vec2>,
}

impl ImpactSearch {
    fn consider(&mut self, points: Option<ImpactPoints>, sprite_pos: Vec2, sprite_width: f32) {
        let Some(points) = points else {
            return;
        };
        let mut pixel = points.left;
        if self.is_from_left == self.flip_x {
            pixel = points.right;
        }
        let mut x = pixel.x;
        if self.flip_x {
            x = sprite_width - 1.0 - pixel.x;
        }
        if !self.is_from_left {
            x += 1.0;
        }
        let candidate = sprite_pos + vec2(x, pixel.y);
        let Some(best) = self.best else {
            self.best = Some(candidate);
            return;
        };
        let mut outward = candidate.x - best.x;
        if self.is_from_left {
            outward = best.x - candidate.x;
        }
        let is_nearer_height = (candidate.y - self.source.y).abs() < (best.y - self.source.y).abs();
        if outward > 0.0 || (outward == 0.0 && is_nearer_height) {
            self.best = Some(candidate);
        }
    }
}

pub struct UnitManager {
    pub tile_position: Vec<IVec2>,
    pub direction: Vec<Direction>,
    pub owner: Vec<UnitOwner>,
    pub unit_type: Vec<UnitType>,
    pub level_pos: Vec<IVec2>,

    pub move_period: Vec<u8>,

    pub goal_queue: Vec<Vec<GoalNode>>,

    pub hp: Vec<u16>,
    pub target: Vec<Option<usize>>,
    pub kills: Vec<u16>,
    pub profile: Vec<UnitProfile>,
    pub waypoints: Vec<Vec<GoalNode>>,

    pub prev_tile_position: Vec<IVec2>,
    pub prev_level_pos: Vec<IVec2>,
    pub goal_unreachable_streak: Vec<u8>,
    pub is_initialized: bool,

    pub turn_count: u32,
    pub turn_timer: f32,
    pub turn_tick_this_frame: bool,
    pub turn_alpha: f32,

    pub economy: Economy,

    pub input_build: bool,
    pub is_dead: Vec<bool>,

    pub pending_attacks: Vec<AttackEvent>,
    pub pending_impact_sfx: Vec<SfxEvent>,
    pub pending_gathers: Vec<GatherEvent>,
    live_enemy_count: HashMap<IVec2, u32>,

    pub pending_orders: Vec<MoveOrder>,
    pub pending_target_orders: Vec<TargetOrder>,
    pub pending_trains: Vec<TrainOrder>,
    pub pending_sells: Vec<usize>,
    pub pending_transfer: Option<IVec2>,
    pub transfer_arrival: Option<IVec2>,
    input_hovered_tile: Option<IVec2>,
    input_hovered_gate: Option<IVec2>,
    input_move_ordered_unit_i: Option<usize>,
    input_mouse: Vec2,
    pub transfer_tooltip: Option<&'static str>,
    transfer_hint_timer: f32,
    transfer_hint_gate: Option<(IVec2, IVec2)>,
    transfer_pulse_timer: f32,
    flame_animation_timer: f32,
    flame_frames_shown: usize,

    pub occupancy: Occupancy,

    scratch_intents: Vec<Option<Intent>>,
    scratch_accepted: Vec<bool>,
    scratch_goal_reachable: Vec<bool>,
    scratch_next_occupancy: Occupancy,
    scratch_rejected: Vec<usize>,
}

impl UnitManager {
    pub fn len(&self) -> usize {
        self.tile_position.len()
    }

    fn direction_to_offset(direction: Direction) -> IVec2 {
        match direction {
            Direction::Up => IVec2::new(0, -1),
            Direction::Down => IVec2::new(0, 1),
            Direction::Left => IVec2::new(-1, 0),
            Direction::Right => IVec2::new(1, 0),
        }
    }

    fn is_move_eligible(&self, unit_i: usize, turn_count: u32) -> bool {
        let move_period = self.effective_move_period(unit_i);
        !self.is_dead[unit_i] && move_period != 0 && turn_count % move_period as u32 == 0
    }

    pub fn max_hp(&self, unit_i: usize) -> u16 {
        unit_max_hp(self.unit_type[unit_i], &self.profile[unit_i])
    }

    pub fn damage_flame(&self, unit_i: usize) -> Option<DamageFlame> {
        if self.unit_type[unit_i] == UnitType::Soldier {
            return None;
        }
        let hp = self.hp[unit_i] as u32;
        let max_hp = self.max_hp(unit_i) as u32;
        if hp * 4 <= max_hp {
            return Some(DamageFlame::Bright);
        }
        if hp * 4 <= max_hp * 3 {
            return Some(DamageFlame::Dim);
        }
        None
    }

    pub fn name(&self, unit_i: usize) -> &'static str {
        unit_name(self.unit_type[unit_i], &self.profile[unit_i])
    }

    pub fn men(&self, unit_i: usize) -> u16 {
        men_from_hp(self.hp[unit_i], self.max_hp(unit_i))
    }

    pub fn effective_move_period(&self, unit_i: usize) -> u8 {
        let move_period = self.move_period[unit_i];
        if move_period == 0 || self.unit_type[unit_i] != UnitType::Machine {
            return move_period;
        }
        crew_scaled_period(move_period as u32, self.men(unit_i)).min(u8::MAX as u32) as u8
    }

    pub fn effective_attack_period(&self, unit_i: usize, weapon_period: u32) -> u32 {
        if self.unit_type[unit_i] != UnitType::Machine {
            return weapon_period;
        }
        crew_scaled_period(weapon_period, self.men(unit_i))
    }

    pub fn new() -> Self {
        Self {
            tile_position: Vec::new(),
            direction: Vec::new(),
            owner: Vec::new(),
            unit_type: Vec::new(),
            level_pos: Vec::new(),
            move_period: Vec::new(),
            goal_queue: Vec::new(),
            hp: Vec::new(),
            target: Vec::new(),
            kills: Vec::new(),
            profile: Vec::new(),
            waypoints: Vec::new(),
            prev_tile_position: Vec::new(),
            prev_level_pos: Vec::new(),
            goal_unreachable_streak: Vec::new(),
            is_initialized: false,
            turn_count: 0,
            turn_timer: 0.0,
            turn_tick_this_frame: false,
            turn_alpha: 0.0,
            economy: Economy::starting(),
            input_build: false,
            is_dead: Vec::new(),
            pending_attacks: Vec::new(),
            pending_impact_sfx: Vec::new(),
            pending_gathers: Vec::new(),
            live_enemy_count: HashMap::new(),
            pending_orders: Vec::new(),
            pending_target_orders: Vec::new(),
            pending_trains: Vec::new(),
            pending_sells: Vec::new(),
            pending_transfer: None,
            transfer_arrival: None,
            input_hovered_tile: None,
            input_hovered_gate: None,
            input_move_ordered_unit_i: None,
            input_mouse: Vec2::ZERO,
            transfer_tooltip: None,
            transfer_hint_timer: 0.0,
            transfer_hint_gate: None,
            transfer_pulse_timer: 0.0,
            flame_animation_timer: 0.0,
            flame_frames_shown: 0,
            occupancy: Occupancy::new(),
            scratch_intents: Vec::new(),
            scratch_accepted: Vec::new(),
            scratch_goal_reachable: Vec::new(),
            scratch_next_occupancy: Occupancy::new(),
            scratch_rejected: Vec::new(),
        }
    }

    fn debug_assert_array_lengths(&self) {
        let n = self.len();
        debug_assert_eq!(self.direction.len(), n);
        debug_assert_eq!(self.owner.len(), n);
        debug_assert_eq!(self.unit_type.len(), n);
        debug_assert_eq!(self.level_pos.len(), n);
        debug_assert_eq!(self.prev_tile_position.len(), n);
        debug_assert_eq!(self.prev_level_pos.len(), n);
        debug_assert_eq!(self.move_period.len(), n);
        debug_assert_eq!(self.goal_queue.len(), n);
        debug_assert_eq!(self.hp.len(), n);
        debug_assert_eq!(self.target.len(), n);
        debug_assert_eq!(self.kills.len(), n);
        debug_assert_eq!(self.profile.len(), n);
        debug_assert_eq!(self.waypoints.len(), n);
        debug_assert_eq!(self.goal_unreachable_streak.len(), n);
        debug_assert_eq!(self.is_dead.len(), n);
    }

    pub fn truncate_units(&mut self, n: usize) {
        self.tile_position.truncate(n);
        self.direction.truncate(n);
        self.owner.truncate(n);
        self.unit_type.truncate(n);
        self.level_pos.truncate(n);
        self.move_period.truncate(n);
        self.goal_queue.truncate(n);
        self.hp.truncate(n);
        self.target.truncate(n);
        self.kills.truncate(n);
        self.profile.truncate(n);
        self.waypoints.truncate(n);
        self.is_dead.truncate(n);
        self.prev_tile_position.truncate(n);
        self.prev_level_pos.truncate(n);
        self.goal_unreachable_streak.truncate(n);
        self.debug_assert_array_lengths();
    }

    pub fn add(
        &mut self,
        owner: UnitOwner,
        unit_type: UnitType,
        level_pos: IVec2,
        tile_pos: IVec2,
        direction: Direction,
        profile: UnitProfile,
        men: u16,
    ) -> usize {
        let max_hp = unit_max_hp(unit_type, &profile) as u32;
        let hp = max_hp * men.min(MEN_PER_UNIT) as u32 / MEN_PER_UNIT as u32;
        self.tile_position.push(tile_pos);
        self.direction.push(direction);
        self.owner.push(owner);
        self.unit_type.push(unit_type);
        self.level_pos.push(level_pos);
        self.prev_tile_position.push(tile_pos);
        self.prev_level_pos.push(level_pos);
        self.move_period
            .push(unit_move_period(unit_type, &profile.design));
        self.goal_queue.push(Vec::new());
        self.hp.push(hp as u16);
        self.target.push(None);
        self.kills.push(0);
        self.profile.push(profile);
        self.waypoints.push(Vec::new());
        self.goal_unreachable_streak.push(0);
        self.is_dead.push(false);

        self.debug_assert_array_lengths();

        let unit_i = self.len() - 1;
        self.count_enemy_in(unit_i, level_pos);
        unit_i
    }

    pub fn teleport_unit(
        &mut self,
        unit_i: usize,
        target_level_pos: IVec2,
        target_tile_pos: IVec2,
    ) {
        self.count_enemy_out(unit_i, self.level_pos[unit_i]);
        self.tile_position[unit_i] = target_tile_pos;
        self.level_pos[unit_i] = target_level_pos;
        self.count_enemy_in(unit_i, target_level_pos);
    }

    fn handle_level_transition(
        &mut self,
        unit_i: usize,
        next: GoalNode,
        entry_direction: Direction,
        debug_log: &mut DebugLog,
    ) {
        let unit_level_pos = self.level_pos[unit_i];

        self.teleport_unit(unit_i, next.level_pos, next.tile);

        self.direction[unit_i] = entry_direction;

        debug_log.log(format!(
            "Level transition: {:?} -> {:?} at tile {:?}",
            unit_level_pos, next.level_pos, next.tile
        ));
    }

    fn update_init(&mut self, level_manager: &LevelManager, debug_log: &mut DebugLog) {
        if !self.is_initialized && level_manager.is_initialized {
            self.spawn_units_from_levels(level_manager, debug_log);
            self.is_initialized = true;
        }
    }

    fn spawn_units_from_levels(&mut self, level_manager: &LevelManager, debug_log: &mut DebugLog) {
        let mut spawns: Vec<(IVec2, IVec2, &UnitSpawnData)> = Vec::new();
        for (level_pos, level) in &level_manager.levels {
            for (tile_key, unit_spawn) in &level.unit_spawns {
                spawns.push((*level_pos, *tile_key, unit_spawn));
            }
        }
        spawns.sort_by(spawn_sort_order);

        for (level_pos, _tile_key, unit_spawn) in &spawns {
            let unit_i = self.add(
                unit_spawn.owner,
                unit_spawn.unit_type,
                *level_pos,
                unit_spawn.tile_pos,
                unit_spawn.direction,
                unit_spawn.profile,
                MEN_PER_UNIT,
            );
            for waypoint in &unit_spawn.waypoints {
                self.waypoints[unit_i].push(GoalNode {
                    level_pos: *level_pos,
                    tile: *waypoint,
                });
            }
        }

        if !spawns.is_empty() {
            debug_log.log(format!(
                "[UnitManager] Spawned {} units from level data",
                spawns.len()
            ));
        }
    }

    pub fn init(&mut self, _debug_log: &mut DebugLog) {}

    fn input_move_order(
        &mut self,
        selector: &SelectorManager,
        level_editor: &LevelEditorManager,
        level_manager: &LevelManager,
        minimap: &MinimapManager,
        camera_manager: &CameraManager,
        player_manager: &PlayerManager,
    ) {
        self.input_move_ordered_unit_i = None;
        let selected_unit_i = selector.selected_unit_i;
        if !is_mouse_button_pressed(MouseButton::Right) {
            return;
        }
        if level_editor.is_enabled {
            return;
        }
        if is_mouse_over_overlay(camera_manager, level_editor, minimap, player_manager) {
            return;
        }

        let Some(unit_i) = selected_unit_i else {
            return;
        };
        if unit_i >= self.len() || self.is_dead[unit_i] {
            return;
        }
        if self.owner[unit_i] != UnitOwner::P0 {
            return;
        }
        if self.move_period[unit_i] == 0 {
            return;
        }

        let Some(view_sector) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(view_sector) else {
            return;
        };
        let Some(tile) = level.world_to_tile_pos(view_sector, camera_manager.mouse_world_pos)
        else {
            return;
        };

        if let Some(other_i) = self.occupancy.get(view_sector, tile) {
            if self.owner[other_i] != UnitOwner::P0 {
                self.pending_target_orders.push(TargetOrder {
                    unit_i,
                    target: Some(other_i),
                });
                return;
            }
        }

        let target = GoalNode {
            level_pos: view_sector,
            tile,
        };

        self.pending_target_orders.push(TargetOrder {
            unit_i,
            target: None,
        });
        self.pending_orders.push(MoveOrder {
            unit_i,
            target,
            is_queued: is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift),
        });
        self.input_move_ordered_unit_i = Some(unit_i);
    }

    fn input_transfer(
        &mut self,
        level_editor: &LevelEditorManager,
        level_manager: &LevelManager,
        minimap: &MinimapManager,
        camera_manager: &CameraManager,
        player_manager: &PlayerManager,
    ) {
        self.input_hovered_tile = None;
        self.input_hovered_gate = None;
        self.input_mouse = virtual_mouse_position(camera_manager);
        if level_editor.is_enabled {
            return;
        }
        if is_mouse_over_overlay(camera_manager, level_editor, minimap, player_manager) {
            return;
        }
        let Some(view_sector) = level_manager.view_sector_pos else {
            return;
        };
        let Some(tile) = level_manager.tile_under_mouse(camera_manager) else {
            return;
        };
        self.input_hovered_tile = Some(tile);
        if level_manager.gate_arrival(view_sector, tile).is_none() {
            return;
        }
        self.input_hovered_gate = Some(tile);
        if is_mouse_button_pressed(MouseButton::Left) && self.army_level() == Some(view_sector) {
            self.pending_transfer = Some(tile);
        }
    }

    pub fn input(
        &mut self,
        selector: &SelectorManager,
        level_editor: &LevelEditorManager,
        level_manager: &LevelManager,
        minimap: &MinimapManager,
        camera_manager: &CameraManager,
        player_manager: &PlayerManager,
    ) {
        if level_editor.is_editing_properties() {
            return;
        }

        self.input_move_order(
            selector,
            level_editor,
            level_manager,
            minimap,
            camera_manager,
            player_manager,
        );
        self.input_transfer(
            level_editor,
            level_manager,
            minimap,
            camera_manager,
            player_manager,
        );

        #[cfg(debug_assertions)]
        if !player_manager.is_keyboard_captured() && is_key_pressed(KeyCode::B) {
            self.input_build = true;
        }
    }

    fn process_build_input(
        &mut self,
        selector: &SelectorManager,
        level_manager: &LevelManager,
        debug_log: &mut DebugLog,
        history: &mut HistoryManager,
    ) {
        if !self.input_build {
            return;
        }
        self.input_build = false;

        let Some(unit_i) = selector.selected_unit_i else {
            return;
        };
        if unit_i >= self.len() {
            return;
        }

        let level_pos = self.level_pos[unit_i];
        let target = self.tile_position[unit_i] + Self::direction_to_offset(self.direction[unit_i]);

        if !Self::is_tile_walkable(level_manager, level_pos, target)
            || self.is_tile_occupied(level_pos, target)
        {
            return;
        }

        let build_i = self.add(
            self.owner[unit_i],
            UnitType::Structure,
            level_pos,
            target,
            Direction::Up,
            structure_profile(StructureSprite::WallWood),
            MEN_PER_UNIT,
        );
        history.record_event_spawn(build_i as u32);
        self.occupancy.insert(level_pos, target, build_i);
        debug_log.log(format!("Built WallWood at {:?} in {:?}", target, level_pos));
    }

    pub fn update_clock(
        &mut self,
        dt: f32,
        loading_manager: &LoadingManager,
        debug_control_manager: &mut DebugControlManager,
        level_manager: &LevelManager,
        debug_log: &mut DebugLog,
    ) {
        let is_initialized = loading_manager.is_initialized;
        self.update_init(level_manager, debug_log);

        self.turn_tick_this_frame = false;
        self.transfer_arrival = None;
        self.transfer_hint_timer = (self.transfer_hint_timer - dt).max(0.0);
        self.transfer_pulse_timer = (self.transfer_pulse_timer + dt * TRANSFER_PULSE_SPEED) % TAU;
        self.flame_animation_timer += dt;
        let flame_frame_duration = 1.0 / FLAME_ANIMATION_FPS;
        if self.flame_animation_timer >= flame_frame_duration {
            self.flame_animation_timer -= flame_frame_duration;
            self.flame_frames_shown = self.flame_frames_shown.wrapping_add(1);
        }

        if is_initialized {
            if debug_control_manager.steps_pending > 0 {
                debug_control_manager.steps_pending -= 1;
                self.turn_timer = 0.0;
                self.turn_tick_this_frame = true;
            } else if !debug_control_manager.is_paused && !debug_control_manager.is_rewinding() {
                self.turn_timer += dt;
                if self.turn_timer >= TURN_PERIOD {
                    self.turn_timer -= TURN_PERIOD;
                    self.turn_tick_this_frame = true;
                }
            }
        }

        self.turn_alpha = if debug_control_manager.is_paused {
            1.0
        } else {
            (self.turn_timer / TURN_PERIOD).min(1.0)
        };
    }

    pub fn update(
        &mut self,
        level_manager: &mut LevelManager,
        selector: &mut SelectorManager,
        gpu_path: &mut GpuPathManager,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
        debug_log: &mut DebugLog,
    ) {
        if !self.is_initialized {
            return;
        }

        #[cfg(debug_assertions)]
        self.process_build_input(selector, level_manager, debug_log, history);

        if let Some(unit_i) = self.input_move_ordered_unit_i.take() {
            audio_manager.pending_sfx.push(unit_sfx(
                self.unit_type[unit_i],
                SfxEvent::MoveStartSoldier,
                SfxEvent::MoveStartMachine,
            ));
        }

        if !self.turn_tick_this_frame {
            return;
        }
        history.begin_turn(self.turn_count);
        self.prepare_field_inputs(level_manager, history, audio_manager);
        self.process_transfer(level_manager, history, audio_manager);
        gpu_path.build_and_run(level_manager, self);
        self.advance_turn(level_manager, selector, debug_log, gpu_path, history);
    }

    pub fn update_transfer_tooltip(&mut self, level_manager: &LevelManager) {
        self.transfer_tooltip = self.find_transfer_tooltip(level_manager);
    }

    pub fn end_turn(&mut self, history: &mut HistoryManager) {
        if !self.is_initialized || !self.turn_tick_this_frame {
            return;
        }
        history.end_turn();
    }

    pub fn prepare_field_inputs(
        &mut self,
        level_manager: &LevelManager,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        self.process_orders(level_manager, history, audio_manager);
        self.pop_arrived_goals(history);
        self.rebuild_occupancy();
    }

    fn process_orders(
        &mut self,
        level_manager: &LevelManager,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        self.process_sell_orders(history, audio_manager);
        self.process_train_orders(level_manager, history, audio_manager);
        self.process_target_orders(history, audio_manager);
        if self.pending_orders.is_empty() {
            return;
        }

        let orders = mem::take(&mut self.pending_orders);
        for order in orders {
            let unit_i = order.unit_i;
            if unit_i >= self.len() || self.is_dead[unit_i] {
                continue;
            }
            if self.move_period[unit_i] == 0 {
                continue;
            }
            if !Self::is_tile_walkable(level_manager, order.target.level_pos, order.target.tile) {
                continue;
            }
            if self.owner[unit_i] == UnitOwner::P0
                && order.target.level_pos != self.level_pos[unit_i]
            {
                continue;
            }

            if !order.is_queued {
                self.clear_goals(unit_i, history);
            }
            history.record_event_goal_push(unit_i as u32);
            self.goal_queue[unit_i].push(order.target);
            self.reset_unreachable_streak(unit_i, history);
        }
    }

    pub fn train_cost(&self, design: &UnitDesign) -> Option<Resources> {
        let men = self.economy.resources.men.min(MEN_PER_UNIT as i32);
        if men <= 0 {
            return None;
        }
        Some(design_cost_for_men(design, men))
    }

    pub fn can_afford(&self, design: &UnitDesign) -> bool {
        if !is_design_available(design, &self.economy) {
            return false;
        }
        match self.train_cost(design) {
            Some(cost) => self.economy.resources.covers(&cost),
            None => false,
        }
    }

    pub fn is_placement_valid(
        &self,
        level_manager: &LevelManager,
        level_pos: IVec2,
        tile: IVec2,
    ) -> bool {
        if let Some(army_level) = self.army_level() {
            if army_level != level_pos {
                return false;
            }
        }
        Self::is_order_target_valid(level_manager, level_pos, tile)
            && Self::is_tile_walkable(level_manager, level_pos, tile)
            && !self.is_tile_occupied(level_pos, tile)
    }

    pub fn can_sell(&self, unit_i: usize) -> bool {
        unit_i < self.len()
            && !self.is_dead[unit_i]
            && self.owner[unit_i] == UnitOwner::P0
            && !self.unit_type[unit_i].is_structure()
    }

    pub fn sell_refund(&self, unit_i: usize) -> Resources {
        sell_refund(
            &self.profile[unit_i].design,
            self.hp[unit_i],
            self.max_hp(unit_i),
            self.men(unit_i),
        )
    }

    pub fn is_game_won(&self) -> bool {
        for unit_i in 0..self.len() {
            if self.is_dead[unit_i] && self.profile[unit_i].is_final_fortress {
                return true;
            }
        }
        false
    }

    pub fn has_live_enemy(&self, level_pos: IVec2) -> bool {
        let Some(count) = self.live_enemy_count.get(&level_pos) else {
            return false;
        };
        *count > 0
    }

    fn count_enemy_in(&mut self, unit_i: usize, level_pos: IVec2) {
        if self.owner[unit_i] != UnitOwner::P1 || self.is_dead[unit_i] {
            return;
        }
        *self.live_enemy_count.entry(level_pos).or_insert(0) += 1;
    }

    fn count_enemy_out(&mut self, unit_i: usize, level_pos: IVec2) {
        if self.owner[unit_i] != UnitOwner::P1 || self.is_dead[unit_i] {
            return;
        }
        *self.live_enemy_count.get_mut(&level_pos).unwrap() -= 1;
    }

    pub fn is_game_lost(&self) -> bool {
        self.economy.resources.men <= 0 && self.army_level().is_none()
    }

    pub fn clear_pending_orders(&mut self) {
        self.pending_orders.clear();
        self.pending_target_orders.clear();
        self.pending_trains.clear();
        self.pending_sells.clear();
        self.pending_transfer = None;
    }

    pub fn has_trained(&self, level_manager: &LevelManager) -> bool {
        let mut spawned = 0;
        for level in level_manager.levels.values() {
            for unit_spawn in level.unit_spawns.values() {
                if unit_spawn.owner == UnitOwner::P0 && !unit_spawn.unit_type.is_structure() {
                    spawned += 1;
                }
            }
        }
        let mut existing = 0;
        for unit_i in 0..self.len() {
            if self.owner[unit_i] == UnitOwner::P0 && !self.unit_type[unit_i].is_structure() {
                existing += 1;
            }
        }
        existing > spawned
    }

    fn process_train_orders(
        &mut self,
        level_manager: &LevelManager,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        let orders = mem::take(&mut self.pending_trains);
        for order in orders {
            let Some(cost) = self.train_cost(&order.design) else {
                continue;
            };
            if self.army_unit_count() >= ARMY_UNIT_MAX
                || !is_design_available(&order.design, &self.economy)
                || !self.economy.resources.covers(&cost)
                || !self.is_placement_valid(level_manager, order.level_pos, order.tile)
            {
                continue;
            }
            audio_manager.pending_sfx.push(unit_sfx(
                unit_type_for(order.design.kind),
                SfxEvent::UnitPlacedSoldier,
                SfxEvent::UnitPlacedMachine,
            ));
            let unit_i = self.add(
                UnitOwner::P0,
                unit_type_for(order.design.kind),
                order.level_pos,
                order.tile,
                Direction::Down,
                UnitProfile::new(order.design),
                cost.men as u16,
            );
            history.record_event_spawn(unit_i as u32);
            history.record_event_resources(&self.economy.resources);
            self.economy.resources.subtract(&cost);
            self.occupancy.insert(order.level_pos, order.tile, unit_i);
        }
    }

    fn process_sell_orders(
        &mut self,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        let orders = mem::take(&mut self.pending_sells);
        for unit_i in orders {
            if !self.can_sell(unit_i) {
                continue;
            }
            let refund = self.sell_refund(unit_i);
            audio_manager.pending_sfx.push(unit_sfx(
                self.unit_type[unit_i],
                SfxEvent::UnitSoldSoldier,
                SfxEvent::UnitSoldMachine,
            ));
            self.remove_unit(unit_i, history);
            history.record_event_resources(&self.economy.resources);
            self.economy.resources.add(&refund);
        }
    }

    pub fn army_level(&self) -> Option<IVec2> {
        for unit_i in 0..self.len() {
            if self.is_live_player_mobile(unit_i) {
                return Some(self.level_pos[unit_i]);
            }
        }
        None
    }

    pub fn army_unit_count(&self) -> usize {
        let mut count = 0;
        for unit_i in 0..self.len() {
            if self.is_live_player_mobile(unit_i) {
                count += 1;
            }
        }
        count
    }

    fn is_live_player_mobile(&self, unit_i: usize) -> bool {
        self.owner[unit_i] == UnitOwner::P0
            && !self.is_dead[unit_i]
            && !self.unit_type[unit_i].is_structure()
    }

    pub fn remove_unit(&mut self, unit_i: usize, history: &mut HistoryManager) {
        history.record_event_death(unit_i as u32);
        self.count_enemy_out(unit_i, self.level_pos[unit_i]);
        self.is_dead[unit_i] = true;
        self.occupancy
            .remove(self.level_pos[unit_i], self.tile_position[unit_i]);
    }

    fn is_army_unit(&self, unit_i: usize, army_level: IVec2) -> bool {
        self.is_live_player_mobile(unit_i) && self.level_pos[unit_i] == army_level
    }

    pub fn transfer_blocker(&self, unit_i: usize, gate: &Gate) -> Option<&'static str> {
        if let Some(target_i) = self.target[unit_i] {
            let is_live = target_i < self.len() && !self.is_dead[target_i];
            if is_live && self.level_pos[target_i] == self.level_pos[unit_i] {
                return Some("Fighting: the army can't leave");
            }
        }
        if !gate.is_in_transfer_area(self.tile_position[unit_i]) {
            return Some("Outside the transfer area: the army can't leave");
        }
        None
    }

    pub fn is_transfer_ready(&self, army_level: IVec2, gate: &Gate) -> bool {
        let mut army_size = 0;
        for unit_i in 0..self.len() {
            if !self.is_army_unit(unit_i, army_level) {
                continue;
            }
            if self.transfer_blocker(unit_i, gate).is_some() {
                return false;
            }
            army_size += 1;
        }
        army_size > 0
    }

    fn shown_transfer_gate(&self, level_manager: &LevelManager) -> Option<(IVec2, IVec2)> {
        if let Some(tile) = self.input_hovered_gate {
            return Some((level_manager.view_sector_pos?, tile));
        }
        if self.transfer_hint_timer > 0.0 {
            return self.transfer_hint_gate;
        }
        None
    }

    fn deny_transfer(
        &mut self,
        gate_level: IVec2,
        gate_tile: IVec2,
        audio_manager: &mut AudioManager,
    ) {
        audio_manager
            .pending_sfx
            .push(SfxEvent::SectorTransitionDeny);
        self.transfer_hint_timer = TRANSFER_HINT_SECONDS;
        self.transfer_hint_gate = Some((gate_level, gate_tile));
    }

    fn process_transfer(
        &mut self,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        let Some(gate_tile) = self.pending_transfer.take() else {
            return;
        };
        let Some(army_level) = self.army_level() else {
            return;
        };
        let Some((destination, arrival_tile)) = level_manager.gate_arrival(army_level, gate_tile)
        else {
            return;
        };
        let Some(gate) = level_manager.get_gate(army_level, gate_tile) else {
            return;
        };
        if !self.is_transfer_ready(army_level, gate) {
            self.deny_transfer(army_level, gate_tile, audio_manager);
            return;
        }
        let Some(arrival_gate) = level_manager.get_gate(destination, arrival_tile) else {
            return;
        };
        let area_tile = arrival_gate.transfer_area_tile;
        let area_size = arrival_gate.transfer_area_size;

        let mut free_tiles = Vec::new();
        for y in area_tile.y..area_tile.y + area_size.y {
            for x in area_tile.x..area_tile.x + area_size.x {
                let tile = IVec2::new(x, y);
                if !Self::is_order_target_valid(level_manager, destination, tile) {
                    continue;
                }
                if !Self::is_tile_walkable(level_manager, destination, tile) {
                    continue;
                }
                if self.is_tile_occupied(destination, tile) {
                    continue;
                }
                free_tiles.push(tile);
            }
        }
        let mut army = Vec::new();
        for unit_i in 0..self.len() {
            if self.is_army_unit(unit_i, army_level) {
                army.push(unit_i);
            }
        }
        if free_tiles.len() < army.len() {
            self.deny_transfer(army_level, gate_tile, audio_manager);
            return;
        }

        for k in 0..army.len() {
            let unit_i = army[k];
            history.record_move(
                unit_i as u32,
                self.level_pos[unit_i],
                self.tile_position[unit_i],
                self.direction[unit_i],
            );
            self.clear_goals(unit_i, history);
            self.occupancy
                .remove(self.level_pos[unit_i], self.tile_position[unit_i]);
            self.level_pos[unit_i] = destination;
            self.tile_position[unit_i] = free_tiles[k];
            self.occupancy.insert(destination, free_tiles[k], unit_i);
        }
        level_manager.view_sector_pos = Some(destination);
        self.transfer_arrival = Some(destination);
        audio_manager
            .pending_sfx
            .push(SfxEvent::SectorTransitionSuccess);
    }

    fn process_target_orders(
        &mut self,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        let orders = mem::take(&mut self.pending_target_orders);
        for order in orders {
            let unit_i = order.unit_i;
            if unit_i >= self.len() || self.is_dead[unit_i] {
                continue;
            }
            if let Some(target_i) = order.target {
                if target_i >= self.len()
                    || self.is_dead[target_i]
                    || self.owner[target_i] == self.owner[unit_i]
                {
                    continue;
                }
                self.clear_goals(unit_i, history);
                if self.owner[unit_i] == UnitOwner::P0 {
                    audio_manager.pending_sfx.push(unit_sfx(
                        self.unit_type[unit_i],
                        SfxEvent::TargetSetSoldier,
                        SfxEvent::TargetSetMachine,
                    ));
                }
            }
            self.set_target(unit_i, order.target, history);
        }
    }

    pub fn set_target(
        &mut self,
        unit_i: usize,
        target: Option<usize>,
        history: &mut HistoryManager,
    ) {
        if self.target[unit_i] == target {
            return;
        }
        history.record_event_target(unit_i as u32, self.target[unit_i]);
        self.target[unit_i] = target;
        self.reset_unreachable_streak(unit_i, history);
    }

    pub fn clear_goals(&mut self, unit_i: usize, history: &mut HistoryManager) {
        if self.goal_queue[unit_i].is_empty() {
            return;
        }
        history.record_event_goal_clear(unit_i as u32, &self.goal_queue[unit_i]);
        self.goal_queue[unit_i].clear();
    }

    pub fn set_goal(&mut self, unit_i: usize, goal: GoalNode, history: &mut HistoryManager) {
        self.clear_goals(unit_i, history);
        history.record_event_goal_push(unit_i as u32);
        self.goal_queue[unit_i].push(goal);
    }

    fn reset_unreachable_streak(&mut self, unit_i: usize, history: &mut HistoryManager) {
        if self.goal_unreachable_streak[unit_i] == 0 {
            return;
        }
        history
            .record_event_unreachable_streak(unit_i as u32, self.goal_unreachable_streak[unit_i]);
        self.goal_unreachable_streak[unit_i] = 0;
    }

    fn pop_arrived_goals(&mut self, history: &mut HistoryManager) {
        for unit_i in 0..self.len() {
            if self.is_dead[unit_i] || self.goal_queue[unit_i].is_empty() {
                continue;
            }
            let here = self.effective_node(unit_i);
            let mut arrived = false;
            while self.goal_queue[unit_i].first() == Some(&here) {
                let removed = self.goal_queue[unit_i][0];
                history.record_event_goal_pop_front(unit_i as u32, removed.level_pos, removed.tile);
                self.goal_queue[unit_i].remove(0);
                arrived = true;
            }
            if arrived && self.goal_unreachable_streak[unit_i] != 0 {
                history.record_event_unreachable_streak(
                    unit_i as u32,
                    self.goal_unreachable_streak[unit_i],
                );
                self.goal_unreachable_streak[unit_i] = 0;
            }
        }
    }

    fn advance_turn(
        &mut self,
        level_manager: &mut LevelManager,
        selector: &mut SelectorManager,
        debug_log: &mut DebugLog,
        gpu_path: &mut GpuPathManager,
        history: &mut HistoryManager,
    ) {
        self.turn_count += 1;

        for unit_i in 0..self.len() {
            self.prev_tile_position[unit_i] = self.tile_position[unit_i];
            self.prev_level_pos[unit_i] = self.level_pos[unit_i];
        }

        let n = self.len();
        self.scratch_intents.resize(n, None);
        self.scratch_accepted.clear();
        self.scratch_accepted.resize(n, false);
        self.scratch_goal_reachable.clear();
        self.scratch_goal_reachable.resize(n, true);

        for round in 0..RESOLVE_ROUNDS_MAX {
            self.scratch_intents.fill(None);

            for unit_i in 0..n {
                if self.scratch_accepted[unit_i] {
                    continue;
                }
                if !self.is_move_eligible(unit_i, self.turn_count) {
                    continue;
                }
                if self.goal_queue[unit_i].is_empty() {
                    continue;
                }

                let dir_code = gpu_path.direction_for_unit(unit_i);
                self.scratch_goal_reachable[unit_i] = gpu_path.goal_reachable_for_unit(unit_i);

                if dir_code == 0 {
                    continue;
                }

                let direction = match dir_code {
                    1 => Direction::Up,
                    2 => Direction::Down,
                    3 => Direction::Left,
                    4 => Direction::Right,
                    _ => continue,
                };

                let offset = Self::direction_to_offset(direction);
                let exit_tile = self.tile_position[unit_i] + offset;

                let (next, entry_direction) = if let Some((entry_level, entry_tile, entry_dir)) =
                    level_manager.calculate_level_transition(self.level_pos[unit_i], exit_tile)
                {
                    (
                        GoalNode {
                            level_pos: entry_level,
                            tile: entry_tile,
                        },
                        Some(entry_dir),
                    )
                } else {
                    let grid_tiles = match level_manager.get_level_at_pos(self.level_pos[unit_i]) {
                        Some(level) => level.grid_tiles,
                        None => IVec2::new(DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y),
                    };
                    if exit_tile.x < 0
                        || exit_tile.x >= grid_tiles.x
                        || exit_tile.y < 0
                        || exit_tile.y >= grid_tiles.y
                    {
                        continue;
                    }
                    (
                        GoalNode {
                            level_pos: self.level_pos[unit_i],
                            tile: exit_tile,
                        },
                        None,
                    )
                };

                if entry_direction.is_some() && self.owner[unit_i] == UnitOwner::P0 {
                    continue;
                }

                self.scratch_intents[unit_i] = Some(match entry_direction {
                    Some(entry_dir) => Intent::Crossing {
                        direction,
                        next,
                        entry_direction: entry_dir,
                    },
                    None => Intent::Step { direction, next },
                });
            }

            self.scratch_next_occupancy.clear();
            for i in 0..n {
                if self.is_dead[i] {
                    continue;
                }
                self.scratch_next_occupancy
                    .insert(self.level_pos[i], self.tile_position[i], i);
            }

            for &owner in ALL_UNIT_OWNERS {
                for unit_i in 0..n {
                    if self.owner[unit_i] != owner {
                        continue;
                    }
                    let entry = &self.scratch_intents[unit_i];
                    let (direction, next, crossing) = match entry {
                        Some(Intent::Step { direction, next }) => (direction, next, false),
                        Some(Intent::Crossing {
                            direction,
                            next,
                            entry_direction: _,
                        }) => (direction, next, true),
                        None => continue,
                    };

                    let from_tile = self.tile_position[unit_i];
                    let exit_tile = from_tile + Self::direction_to_offset(*direction);

                    let (blocked_tile_level, blocked_tile) = if crossing {
                        (next.level_pos, next.tile)
                    } else {
                        (self.level_pos[unit_i], exit_tile)
                    };

                    let blocked = if crossing {
                        if !Self::is_tile_walkable(level_manager, next.level_pos, next.tile) {
                            true
                        } else {
                            self.scratch_next_occupancy
                                .contains(next.level_pos, next.tile)
                        }
                    } else {
                        self.scratch_next_occupancy
                            .contains(self.level_pos[unit_i], exit_tile)
                            || !Self::is_tile_walkable(
                                level_manager,
                                self.level_pos[unit_i],
                                exit_tile,
                            )
                    };

                    if blocked {
                        let is_goal = self.goal_queue[unit_i].first() == Some(next);
                        let mut occupant_stationary = false;
                        if let Some(occupant) = self
                            .scratch_next_occupancy
                            .get(blocked_tile_level, blocked_tile)
                        {
                            occupant_stationary = self.move_period[occupant] == 0
                                || self.goal_queue[occupant].is_empty();
                        }
                        if is_goal && occupant_stationary {
                            let removed = self.goal_queue[unit_i][0];
                            history.record_event_goal_pop_front(
                                unit_i as u32,
                                removed.level_pos,
                                removed.tile,
                            );
                            self.goal_queue[unit_i].remove(0);
                            if self.goal_unreachable_streak[unit_i] != 0 {
                                history.record_event_unreachable_streak(
                                    unit_i as u32,
                                    self.goal_unreachable_streak[unit_i],
                                );
                                self.goal_unreachable_streak[unit_i] = 0;
                            }
                            self.scratch_intents[unit_i] = None;
                        }
                        continue;
                    }

                    self.scratch_accepted[unit_i] = true;
                    if crossing {
                        self.scratch_next_occupancy
                            .insert(next.level_pos, next.tile, unit_i);
                    } else {
                        self.scratch_next_occupancy.insert(
                            self.level_pos[unit_i],
                            exit_tile,
                            unit_i,
                        );
                    }

                    self.scratch_next_occupancy
                        .remove(self.level_pos[unit_i], from_tile);
                }
            }

            mem::swap(&mut self.occupancy, &mut self.scratch_next_occupancy);

            for unit_i in 0..n {
                if !self.scratch_accepted[unit_i] {
                    continue;
                }

                let entry = &self.scratch_intents[unit_i];
                let Some(intent) = entry else {
                    continue;
                };

                history.record_move(
                    unit_i as u32,
                    self.level_pos[unit_i],
                    self.tile_position[unit_i],
                    self.direction[unit_i],
                );

                match intent {
                    Intent::Step { direction, next } => {
                        self.direction[unit_i] = *direction;
                        self.tile_position[unit_i] = next.tile;
                    }
                    Intent::Crossing {
                        direction,
                        next,
                        entry_direction,
                    } => {
                        let direction = *direction;
                        let next = *next;
                        let entry_direction = *entry_direction;
                        self.direction[unit_i] = direction;
                        self.handle_level_transition(unit_i, next, entry_direction, debug_log);
                    }
                };

                if selector.selected_unit_i == Some(unit_i) {
                    selector.set_highlighted_interactable(
                        self.get_facing_interactable(unit_i, level_manager),
                    );
                }
            }

            self.scratch_rejected.clear();
            for unit_i in 0..n {
                if self.scratch_intents[unit_i].is_some() && !self.scratch_accepted[unit_i] {
                    self.scratch_rejected.push(unit_i);
                }
            }
            if self.scratch_rejected.is_empty() {
                break;
            }

            if round + 1 == RESOLVE_ROUNDS_MAX {
                debug_log.log(format!(
                    "advance_turn: retry cap ({}) hit, some units stuck",
                    RESOLVE_ROUNDS_MAX
                ));
                break;
            }

            gpu_path.rebuild_for_retry(self, &self.scratch_rejected);
            gpu_path.upload_unit_pictures();
            gpu_path.run_pass();
            gpu_path.readback();
        }

        for unit_i in 0..n {
            if self.goal_queue[unit_i].is_empty() || !self.is_move_eligible(unit_i, self.turn_count)
            {
                continue;
            }

            if self.scratch_accepted[unit_i] && self.scratch_goal_reachable[unit_i] {
                if self.goal_unreachable_streak[unit_i] != 0 {
                    history.record_event_unreachable_streak(
                        unit_i as u32,
                        self.goal_unreachable_streak[unit_i],
                    );
                }
                self.goal_unreachable_streak[unit_i] = 0;
                continue;
            }
            history.record_event_unreachable_streak(
                unit_i as u32,
                self.goal_unreachable_streak[unit_i],
            );
            self.goal_unreachable_streak[unit_i] =
                self.goal_unreachable_streak[unit_i].saturating_add(1);
            if self.goal_unreachable_streak[unit_i] >= GOAL_UNREACHABLE_GIVE_UP {
                self.goal_unreachable_streak[unit_i] = 0;
                let removed = self.goal_queue[unit_i][0];
                history.record_event_goal_pop_front(unit_i as u32, removed.level_pos, removed.tile);
                self.goal_queue[unit_i].remove(0);
                if self.owner[unit_i] == UnitOwner::P1 {
                    self.set_target(unit_i, None, history);
                }
            }
        }
    }

    pub fn calculate_world_position(&self, unit_i: usize, level_manager: &LevelManager) -> Vec2 {
        let prev_world = level_manager
            .tile_to_world(self.prev_level_pos[unit_i], self.prev_tile_position[unit_i]);
        let cur_world =
            level_manager.tile_to_world(self.level_pos[unit_i], self.tile_position[unit_i]);
        prev_world.lerp(cur_world, self.turn_alpha)
    }

    pub fn impact_point(
        &self,
        unit_i: usize,
        men: u16,
        level_manager: &LevelManager,
        source: Vec2,
        is_from_left: bool,
    ) -> Option<Vec2> {
        let world_pos = snap_to_2px(self.calculate_world_position(unit_i, level_manager));
        let flip_x = self.direction[unit_i] == Direction::Left;
        let design = &self.profile[unit_i].design;
        let mut search = ImpactSearch {
            source,
            is_from_left,
            flip_x,
            best: None,
        };
        match self.unit_type[unit_i] {
            UnitType::Soldier => {
                for k in 0..men {
                    let man_pos = world_pos + man_slot(k, men, flip_x);
                    search.consider(Some(MAN_BASE_IMPACT_POINTS), man_pos, MAN_SPRITE_SIZE);
                    search.consider(
                        man_armor_impact_points(design.armor),
                        man_pos,
                        MAN_SPRITE_SIZE,
                    );
                    search.consider(
                        man_weapon_impact_points(design.weapon),
                        man_pos,
                        MAN_SPRITE_SIZE,
                    );
                }
            }
            UnitType::Machine => {
                search.consider(
                    Some(chassis_impact_points(design.chassis)),
                    world_pos,
                    TILE_SIZE_X,
                );
                search.consider(
                    shielding_impact_points(design.shielding),
                    world_pos,
                    TILE_SIZE_X,
                );
                search.consider(
                    machine_weapon_impact_points(design.weapon),
                    world_pos,
                    TILE_SIZE_X,
                );
                for k in 0..men {
                    let man_pos = world_pos + machine_crew_slot(k, flip_x);
                    search.consider(Some(MAN_BASE_IMPACT_POINTS), man_pos, MAN_SPRITE_SIZE);
                }
            }
            UnitType::Structure => {
                search.flip_x = false;
                search.consider(
                    Some(structure_impact_points(
                        self.profile[unit_i].structure_sprite,
                    )),
                    world_pos,
                    TILE_SIZE_X,
                );
            }
        }
        search.best
    }

    fn render_unit(&self, i: usize, world_pos: Vec2, assets: &AssetManager) {
        let flip_x = self.direction[i] == Direction::Left;
        match self.unit_type[i] {
            UnitType::Soldier => draw_soldier_group(
                assets,
                &self.profile[i].design,
                self.owner[i],
                world_pos,
                self.men(i),
                WHITE,
                flip_x,
            ),
            UnitType::Machine => draw_machine(
                assets,
                &self.profile[i].design,
                self.owner[i],
                world_pos,
                self.men(i),
                WHITE,
                flip_x,
            ),
            UnitType::Structure => {
                draw_structure(assets, self.profile[i].structure_sprite, world_pos, WHITE)
            }
        }
    }

    fn visible_world_position(
        &self,
        i: usize,
        level_manager: &LevelManager,
        view_rect: Rect,
        sector_min: IVec2,
        sector_max: IVec2,
    ) -> Option<Vec2> {
        if self.is_dead[i] {
            return None;
        }

        let current_in = sector_in_range(self.level_pos[i], sector_min, sector_max);
        let previous_in = sector_in_range(self.prev_level_pos[i], sector_min, sector_max);
        if !current_in && !previous_in {
            return None;
        }

        let world_pos = snap_to_2px(self.calculate_world_position(i, level_manager));

        if !view_rect.contains(world_pos) {
            return None;
        }
        Some(world_pos)
    }

    fn render_damage_flame(
        &self,
        i: usize,
        damage_flame: DamageFlame,
        world_pos: Vec2,
        assets: &AssetManager,
    ) {
        let mut flame = &assets.spritesheet_flame_dim;
        if damage_flame == DamageFlame::Bright {
            flame = &assets.spritesheet_flame_bright;
        }
        let mut bases = chassis_flame_bases(self.profile[i].design.chassis);
        if self.unit_type[i] == UnitType::Structure {
            bases = structure_flame_bases(self.profile[i].structure_sprite);
        }
        let flip_x = self.direction[i] == Direction::Left;
        for base in bases {
            let mut base = *base;
            if flip_x && self.unit_type[i] == UnitType::Machine {
                base.x = TILE_SIZE_X - base.x;
            }
            draw_flame_at_base(flame, self.flame_frames_shown, world_pos + base);
        }
    }

    fn render_goal_debug(&self, unit_i: usize, level_manager: &LevelManager, styles: &Styles) {
        if unit_i >= self.len() {
            return;
        }

        for i in 0..self.goal_queue[unit_i].len() {
            let goal = self.goal_queue[unit_i][i];
            let world_pos = level_manager.tile_to_world(goal.level_pos, goal.tile);
            draw_rectangle_lines(
                world_pos.x + 2.0,
                world_pos.y + 2.0,
                TILE_SIZE_X - 4.0,
                TILE_SIZE_Y - 4.0,
                1.0,
                styles.colors.parasite_spread,
            );
        }
    }

    pub fn render(
        &self,
        assets: &AssetManager,
        level_manager: &LevelManager,
        camera_manager: &CameraManager,
    ) {
        if !self.is_initialized {
            return;
        }

        let view_rect = camera_manager.world_view_rect(CULL_MARGIN_PX);

        let (sector_min, sector_max) = sector_range_for_world_rect(view_rect);

        for i in 0..self.len() {
            let Some(world_pos) =
                self.visible_world_position(i, level_manager, view_rect, sector_min, sector_max)
            else {
                continue;
            };

            self.render_unit(i, world_pos, assets);
        }

        for i in 0..self.len() {
            let Some(damage_flame) = self.damage_flame(i) else {
                continue;
            };
            let Some(world_pos) =
                self.visible_world_position(i, level_manager, view_rect, sector_min, sector_max)
            else {
                continue;
            };
            self.render_damage_flame(i, damage_flame, world_pos, assets);
        }
    }

    pub fn render_overlays(
        &self,
        assets: &AssetManager,
        styles: &Styles,
        level_manager: &LevelManager,
        selector: &SelectorManager,
        debug_overlay_manager: &DebugOverlayManager,
        camera_manager: &CameraManager,
    ) {
        if !self.is_initialized {
            return;
        }

        let selected_unit_i = selector.selected_unit_i;
        let debug_ui_visible = debug_overlay_manager.is_ui_visible;

        if debug_ui_visible {
            let view_rect = camera_manager.world_view_rect(CULL_MARGIN_PX);
            let (sector_min, sector_max) = sector_range_for_world_rect(view_rect);
            for i in 0..self.len() {
                let Some(world_pos) = self.visible_world_position(
                    i,
                    level_manager,
                    view_rect,
                    sector_min,
                    sector_max,
                ) else {
                    continue;
                };
                draw_scaled_text(
                    self.name(i),
                    world_pos.x,
                    world_pos.y - TILE_SIZE_Y / 8.0,
                    TILE_SIZE_Y / 3.0,
                    &WHITE,
                    &assets.font,
                );
            }
        }

        if debug_ui_visible {
            if let Some(unit_i) = selected_unit_i {
                self.render_goal_debug(unit_i, level_manager, styles);
            }
        }

        if let Some(unit_i) = selected_unit_i {
            self.render_target_outline(unit_i, level_manager, styles);
        }
        self.render_transfer(level_manager, styles);
    }

    fn render_target_outline(&self, unit_i: usize, level_manager: &LevelManager, styles: &Styles) {
        if unit_i >= self.len() || self.owner[unit_i] != UnitOwner::P0 {
            return;
        }
        let Some(target_i) = self.target[unit_i] else {
            return;
        };
        if target_i >= self.len() || self.is_dead[target_i] {
            return;
        }
        let world_pos = snap_to_2px(self.calculate_world_position(target_i, level_manager));
        draw_rectangle_lines(
            world_pos.x - 1.0,
            world_pos.y - 1.0,
            TILE_SIZE_X + 2.0,
            TILE_SIZE_Y + 2.0,
            1.0,
            styles.colors.red_alarm,
        );
    }

    fn render_transfer(&self, level_manager: &LevelManager, styles: &Styles) {
        let Some(view_sector) = level_manager.view_sector_pos else {
            return;
        };
        let Some(level) = level_manager.get_level_at_pos(view_sector) else {
            return;
        };
        let is_army_here = self.army_level() == Some(view_sector);
        if is_army_here {
            let mut color = styles.colors.yellow_hot;
            color.a = 0.5 + 0.5 * self.transfer_pulse_timer.sin();
            for (tile, interactable) in &level.interactables {
                let Interactable::Gate(gate) = interactable else {
                    continue;
                };
                if level_manager.gate_arrival(view_sector, *tile).is_none() {
                    continue;
                }
                if !self.is_transfer_ready(view_sector, gate) {
                    continue;
                }
                let world_pos = level.tile_to_world_pos(view_sector, *tile);
                draw_rectangle_lines(
                    world_pos.x + 1.0,
                    world_pos.y + 1.0,
                    TILE_SIZE_X - 2.0,
                    TILE_SIZE_Y - 2.0,
                    2.0,
                    color,
                );
            }
        }

        let Some((gate_level, gate_tile)) = self.shown_transfer_gate(level_manager) else {
            return;
        };
        if gate_level != view_sector {
            return;
        }
        let Some(gate) = level.gate_at(gate_tile) else {
            return;
        };
        let is_ready = is_army_here && self.is_transfer_ready(view_sector, gate);
        let area_pos = level.tile_to_world_pos(view_sector, gate.transfer_area_tile);
        let area_w = gate.transfer_area_size.x as f32 * TILE_SIZE_X;
        let area_h = gate.transfer_area_size.y as f32 * TILE_SIZE_Y;
        let mut color = styles.colors.red_alarm;
        if is_ready {
            color = styles.colors.cyan_bright;
        }
        draw_rectangle_lines(area_pos.x, area_pos.y, area_w, area_h, 2.0, color);
        color.a = 0.15;
        draw_rectangle(area_pos.x, area_pos.y, area_w, area_h, color);

        for unit_i in 0..self.len() {
            if !self.is_army_unit(unit_i, view_sector) {
                continue;
            }
            if self.transfer_blocker(unit_i, gate).is_none() {
                continue;
            }
            let world_pos = snap_to_2px(self.calculate_world_position(unit_i, level_manager));
            draw_rectangle(
                world_pos.x + TILE_SIZE_X / 2.0 - 2.0,
                world_pos.y - 8.0,
                4.0,
                5.0,
                styles.colors.red_alarm,
            );
            draw_rectangle(
                world_pos.x + TILE_SIZE_X / 2.0 - 2.0,
                world_pos.y - 2.0,
                4.0,
                2.0,
                styles.colors.red_alarm,
            );
        }
    }

    pub fn ui(&self, styles: &Styles, assets: &AssetManager, camera_manager: &CameraManager) {
        if !self.is_initialized {
            return;
        }
        let Some(text) = self.transfer_tooltip else {
            return;
        };
        draw_tooltip(text, self.input_mouse, styles, assets, camera_manager);
    }

    fn find_transfer_tooltip(&self, level_manager: &LevelManager) -> Option<&'static str> {
        let tile = self.input_hovered_tile?;
        let view_sector = level_manager.view_sector_pos?;
        let (gate_level, gate_tile) = self.shown_transfer_gate(level_manager)?;
        if gate_level != view_sector {
            return None;
        }
        let gate = level_manager.get_gate(view_sector, gate_tile)?;
        let is_ready =
            self.army_level() == Some(view_sector) && self.is_transfer_ready(view_sector, gate);
        if let Some(unit_i) = self.occupancy.get(view_sector, tile) {
            if self.is_army_unit(unit_i, view_sector) {
                if let Some(reason) = self.transfer_blocker(unit_i, gate) {
                    return Some(reason);
                }
            }
        }
        if self.input_hovered_gate.is_some() {
            if is_ready {
                return Some("Click the gate to move the army through");
            }
            return Some(TRANSFER_BLOCKED_TEXT);
        }
        if !is_ready && gate.is_in_transfer_area(tile) {
            return Some(TRANSFER_BLOCKED_TEXT);
        }
        None
    }

    pub fn get_world_position(&self, unit_i: usize, level_manager: &LevelManager) -> Option<Vec2> {
        if !self.is_initialized {
            return None;
        }

        if unit_i >= self.len() {
            return None;
        }

        let world_pos = self.calculate_world_position(unit_i, level_manager);

        Some(snap_to_2px(world_pos))
    }

    pub fn get_all_positions(&self, level_manager: &LevelManager) -> Vec<(usize, Vec2)> {
        let mut positions = Vec::new();
        for unit_i in 0..self.len() {
            if self.is_dead[unit_i] {
                continue;
            }
            if let Some(pos) = self.get_world_position(unit_i, level_manager) {
                positions.push((unit_i, pos));
            }
        }
        positions
    }

    pub fn find_first_player_unit(&self) -> Option<usize> {
        for unit_i in 0..self.len() {
            if self.owner[unit_i] == UnitOwner::P0 && !self.is_dead[unit_i] {
                return Some(unit_i);
            }
        }
        None
    }

    pub fn get_facing_interactable(
        &self,
        unit_i: usize,
        level_manager: &LevelManager,
    ) -> Option<(IVec2, IVec2)> {
        if unit_i >= self.len() {
            return None;
        }

        let current_tile = self.tile_position[unit_i];
        let facing_tile = current_tile + Self::direction_to_offset(self.direction[unit_i]);

        let level_pos = self.level_pos[unit_i];
        let level = level_manager.get_level_at_pos(level_pos)?;

        if level.interactables.contains_key(&facing_tile) {
            Some((level_pos, facing_tile))
        } else {
            None
        }
    }

    pub fn is_order_target_valid(
        level_manager: &LevelManager,
        level_pos: IVec2,
        tile_pos: IVec2,
    ) -> bool {
        let Some(level) = level_manager.get_level_at_pos(level_pos) else {
            return false;
        };
        tile_pos.x >= 0
            && tile_pos.y >= 0
            && tile_pos.x < level.grid_tiles.x
            && tile_pos.y < level.grid_tiles.y
    }

    pub fn is_tile_walkable(
        level_manager: &LevelManager,
        level_pos: IVec2,
        tile_pos: IVec2,
    ) -> bool {
        let Some(level) = level_manager.get_level_at_pos(level_pos) else {
            return false;
        };

        if tile_pos.x < -1
            || tile_pos.y < -1
            || tile_pos.x > level.grid_tiles.x
            || tile_pos.y > level.grid_tiles.y
        {
            return false;
        }

        if let Some(tile_type) = level.tile_layout.get(&tile_pos) {
            if is_tile_obstacle(*tile_type) {
                return false;
            }
        }

        true
    }

    fn effective_node(&self, unit_i: usize) -> GoalNode {
        GoalNode {
            level_pos: self.level_pos[unit_i],
            tile: self.tile_position[unit_i],
        }
    }

    pub fn rebuild_occupancy(&mut self) {
        self.occupancy.clear();
        self.live_enemy_count.clear();
        for i in 0..self.len() {
            if self.is_dead[i] {
                continue;
            }
            self.occupancy
                .insert(self.level_pos[i], self.tile_position[i], i);
            self.count_enemy_in(i, self.level_pos[i]);
        }
    }

    pub fn is_tile_occupied(&self, level_pos: IVec2, tile_pos: IVec2) -> bool {
        self.occupancy.contains(level_pos, tile_pos)
    }
}
