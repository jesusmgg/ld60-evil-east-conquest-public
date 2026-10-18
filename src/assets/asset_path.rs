use crate::unit_design::{ALL_STRUCTURE_SPRITES, RESOURCE_COUNT};

pub const ASSETS_DIR: &str = "assets";

macro_rules! asset_path {
    ($path:expr) => {
        concat!("assets", "/", $path)
    };
}
pub(crate) use asset_path;

pub const FONT_KENNEY_PIXEL: &str = asset_path!("fonts/KenneyPixel.ttf");

pub const BACKGROUND_DEFAULT: &str = asset_path!("sprites/background_default.png");

pub const MAN_BASE: &str = asset_path!("sprites/man/base.png");
pub const MAN_ARMOR_GAMBESON: &str = asset_path!("sprites/man/armor_gambeson.png");
pub const MAN_ARMOR_LEATHER: &str = asset_path!("sprites/man/armor_leather.png");
pub const MAN_ARMOR_CHAIN_MAIL: &str = asset_path!("sprites/man/armor_chain_mail.png");
pub const MAN_ARMOR_PLATE: &str = asset_path!("sprites/man/armor_chain_plate.png");
pub const MAN_WEAPON_DAGGER: &str = asset_path!("sprites/man/weapon_dagger.png");
pub const MAN_WEAPON_SWORD: &str = asset_path!("sprites/man/weapon_sword.png");
pub const MAN_WEAPON_SWORD_AND_SHIELD: &str =
    asset_path!("sprites/man/weapon_sword_and_shield.png");
pub const MAN_WEAPON_SPEAR: &str = asset_path!("sprites/man/weapon_spear.png");
pub const MAN_WEAPON_JAVELIN: &str = asset_path!("sprites/man/weapon_javelin.png");
pub const MAN_WEAPON_BOW: &str = asset_path!("sprites/man/weapon_bow.png");
pub const MAN_WEAPON_CROSSBOW: &str = asset_path!("sprites/man/weapon_crossbow.png");
pub const MAN_WEAPON_GRENADE: &str = asset_path!("sprites/man/weapon_grenade.png");
pub const MAN_WEAPON_HAND_CANNON: &str = asset_path!("sprites/man/weapon_hand_cannon.png");
pub const MAN_COLOR_PLAYER_0: &str = asset_path!("sprites/man/color_player_0.png");
pub const MAN_COLOR_PLAYER_1: &str = asset_path!("sprites/man/color_player_1.png");

pub const MAN_ARMOR_LAYERS: [Option<&str>; 5] = [
    None,
    Some(MAN_ARMOR_GAMBESON),
    Some(MAN_ARMOR_LEATHER),
    Some(MAN_ARMOR_CHAIN_MAIL),
    Some(MAN_ARMOR_PLATE),
];
pub const MAN_WEAPON_LAYERS: [Option<&str>; 10] = [
    None,
    Some(MAN_WEAPON_DAGGER),
    Some(MAN_WEAPON_SWORD),
    Some(MAN_WEAPON_SWORD_AND_SHIELD),
    Some(MAN_WEAPON_SPEAR),
    Some(MAN_WEAPON_JAVELIN),
    Some(MAN_WEAPON_BOW),
    Some(MAN_WEAPON_CROSSBOW),
    Some(MAN_WEAPON_GRENADE),
    Some(MAN_WEAPON_HAND_CANNON),
];
pub const MAN_COLOR_LAYERS: [&str; 2] = [MAN_COLOR_PLAYER_0, MAN_COLOR_PLAYER_1];

pub const MACHINE_CHASSIS_CARRIED: &str = asset_path!("sprites/machine/chassis_carried.png");
pub const MACHINE_CHASSIS_LOG_ROLLERS: &str =
    asset_path!("sprites/machine/chassis_log_rollers.png");
pub const MACHINE_CHASSIS_SOLID_WHEELS: &str =
    asset_path!("sprites/machine/chassis_wheels_solid.png");
pub const MACHINE_CHASSIS_IRON_RIMMED_WHEELS: &str =
    asset_path!("sprites/machine/chassis_wheels_iron_rimmed.png");
pub const MACHINE_CHASSIS_SPOKED_WHEELS: &str =
    asset_path!("sprites/machine/chassis_wheels_spoked.png");
pub const MACHINE_SHIELDING_WOOD: &str = asset_path!("sprites/machine/shielding_wood.png");
pub const MACHINE_SHIELDING_IRON: &str = asset_path!("sprites/machine/shielding_iron.png");
pub const MACHINE_SHIELDING_REINFORCED: &str =
    asset_path!("sprites/machine/shielding_iron_reinforced.png");
pub const MACHINE_WEAPON_WOOD_RAM: &str = asset_path!("sprites/machine/weapon_ram_wood.png");
pub const MACHINE_WEAPON_IRON_RAM: &str = asset_path!("sprites/machine/weapon_ram_iron.png");
pub const MACHINE_WEAPON_CATAPULT: &str = asset_path!("sprites/machine/weapon_catapult.png");
pub const MACHINE_WEAPON_TREBUCHET: &str = asset_path!("sprites/machine/weapon_trebuchet.png");
pub const MACHINE_WEAPON_BALLISTA: &str = asset_path!("sprites/machine/weapon_ballista.png");
pub const MACHINE_WEAPON_CANNON: &str = asset_path!("sprites/machine/weapon_cannon.png");
pub const MACHINE_WEAPON_ORGAN_GUN: &str = asset_path!("sprites/machine/weapon_organ_gun.png");

pub const MACHINE_CHASSIS_LAYERS: [&str; 5] = [
    MACHINE_CHASSIS_CARRIED,
    MACHINE_CHASSIS_LOG_ROLLERS,
    MACHINE_CHASSIS_SOLID_WHEELS,
    MACHINE_CHASSIS_IRON_RIMMED_WHEELS,
    MACHINE_CHASSIS_SPOKED_WHEELS,
];
pub const MACHINE_SHIELDING_LAYERS: [Option<&str>; 4] = [
    None,
    Some(MACHINE_SHIELDING_WOOD),
    Some(MACHINE_SHIELDING_IRON),
    Some(MACHINE_SHIELDING_REINFORCED),
];
pub const MACHINE_WEAPON_LAYERS: [Option<&str>; 8] = [
    None,
    Some(MACHINE_WEAPON_WOOD_RAM),
    Some(MACHINE_WEAPON_IRON_RAM),
    Some(MACHINE_WEAPON_CATAPULT),
    Some(MACHINE_WEAPON_TREBUCHET),
    Some(MACHINE_WEAPON_BALLISTA),
    Some(MACHINE_WEAPON_CANNON),
    Some(MACHINE_WEAPON_ORGAN_GUN),
];

pub const SPRITESHEET_FLAME_BRIGHT: &str = asset_path!("sprites/flame_bright.png");
pub const SPRITESHEET_FLAME_DIM: &str = asset_path!("sprites/flame_dim.png");

pub const PROJECTILE_SPRITES: [&str; 6] = [
    asset_path!("sprites/projectiles/arrow_small.png"),
    asset_path!("sprites/projectiles/arrow_big.png"),
    asset_path!("sprites/projectiles/stone.png"),
    asset_path!("sprites/projectiles/grenade.png"),
    asset_path!("sprites/projectiles/bullet.png"),
    asset_path!("sprites/projectiles/ball.png"),
];
pub const EFFECT_SPRITES: [&str; 4] = [
    asset_path!("sprites/effects/hit_single_target.png"),
    asset_path!("sprites/effects/explosion_small.png"),
    asset_path!("sprites/effects/explosion_medium.png"),
    asset_path!("sprites/effects/explosion_large.png"),
];

pub const SPRITESHEET_LIGHT_STRONG: &str = asset_path!("sprites/light-strong.png");
pub const SPRITESHEET_LIGHT_MEDIUM: &str = asset_path!("sprites/light-medium.png");
pub const SPRITESHEET_LIGHT_DIM: &str = asset_path!("sprites/light-dim.png");
pub const SPRITESHEET_LIGHT_AMBIENT: &str = asset_path!("sprites/light-ambient.png");

pub const TILE_DIRT: &str = asset_path!("sprites/tiles/dirt.png");
pub const TILE_GRASS: &str = asset_path!("sprites/tiles/grass.png");
pub const TILE_MOUNTAIN: &str = asset_path!("sprites/tiles/mountain.png");
pub const TILE_STONE: &str = asset_path!("sprites/tiles/stone.png");
pub const TILE_TREE_1: &str = asset_path!("sprites/tiles/tree_1.png");
pub const TILE_TREE_2: &str = asset_path!("sprites/tiles/tree_2.png");
pub const TILE_TREE_3: &str = asset_path!("sprites/tiles/tree_3.png");
pub const TILE_WATER: &str = asset_path!("sprites/tiles/water.png");

pub const INTERACTABLE_GATE: &str = asset_path!("sprites/interactables/gate.png");
pub const INTERACTABLE_RUBBLE: &str = asset_path!("sprites/interactables/rubble.png");

pub const STRUCTURE_SPRITES: [&str; ALL_STRUCTURE_SPRITES.len()] = [
    asset_path!("sprites/structures/wall_wood.png"),
    asset_path!("sprites/structures/wall_wood_2.png"),
    asset_path!("sprites/structures/wall_stone.png"),
    asset_path!("sprites/structures/wall_stone_2.png"),
    asset_path!("sprites/structures/tower_wood.png"),
    asset_path!("sprites/structures/tower_stone.png"),
    asset_path!("sprites/structures/castle.png"),
    asset_path!("sprites/structures/house_1.png"),
    asset_path!("sprites/structures/house_2.png"),
    asset_path!("sprites/structures/house_3.png"),
    asset_path!("sprites/structures/house_4.png"),
    asset_path!("sprites/structures/jail.png"),
    asset_path!("sprites/structures/university.png"),
    asset_path!("sprites/structures/mining_camp.png"),
    asset_path!("sprites/structures/lumber_camp.png"),
    asset_path!("sprites/structures/blacksmith.png"),
    asset_path!("sprites/structures/storehouse.png"),
    asset_path!("sprites/structures/fortress.png"),
];

pub const RESOURCE_ICONS: [&str; RESOURCE_COUNT] = [
    asset_path!("sprites/resource_icons/men.png"),
    asset_path!("sprites/resource_icons/iron.png"),
    asset_path!("sprites/resource_icons/wood.png"),
    asset_path!("sprites/resource_icons/mercury.png"),
    asset_path!("sprites/resource_icons/sulfur.png"),
];
