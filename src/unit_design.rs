use crate::simple_enum_serbin;
use crate::unit_manager::UnitType;
use nanoserde::{DeBin, SerBin};

pub const MEN_PER_UNIT: u16 = 6;
pub const PLAYER_AGGRO_RANGE: u8 = 5;
const ENEMY_MELEE_AGGRO_RANGE: u8 = 5;
const ENEMY_AGGRO_PAST_RANGE: u8 = 2;
pub const DESIGN_SLOTS_MAX: usize = 8;

simple_enum_serbin!(UnitKind, ALL_UNIT_KINDS {
    Soldier => 0,
    Machine => 100,
});

simple_enum_serbin!(Weapon, ALL_WEAPONS {
    Unarmed => 0,
    Dagger => 100,
    Sword => 110,
    SwordAndShield => 120,
    Spear => 130,
    Javelin => 140,
    Bow => 150,
    Crossbow => 160,
    Grenades => 170,
    HandCannon => 180,
    WoodRam => 200,
    IronRam => 210,
    Catapult => 220,
    Trebuchet => 230,
    Ballista => 240,
    Cannon => 250,
    OrganGun => 260,
});

simple_enum_serbin!(Armor, ALL_ARMORS {
    NoArmor => 0,
    Gambeson => 100,
    Leather => 110,
    ChainMail => 120,
    Plate => 130,
});

simple_enum_serbin!(Shielding, ALL_SHIELDINGS {
    NoShielding => 0,
    WoodShielding => 100,
    IronShielding => 110,
    ReinforcedShielding => 120,
});

simple_enum_serbin!(Chassis, ALL_CHASSIS {
    Carried => 0,
    LogRollers => 100,
    SolidWheels => 110,
    IronRimmedWheels => 120,
    SpokedWheels => 130,
});

simple_enum_serbin!(StructureSprite, ALL_STRUCTURE_SPRITES {
    WallWood => 0,
    WallWood2 => 60,
    WallStone => 70,
    WallStone2 => 80,
    TowerWood => 10,
    TowerStone => 90,
    Castle => 100,
    House1 => 110,
    House2 => 120,
    House3 => 130,
    House4 => 140,
    Jail => 20,
    University => 30,
    MiningCamp => 150,
    LumberCamp => 160,
    Blacksmith => 170,
    Storehouse => 40,
    Fortress => 50,
});

simple_enum_serbin!(Technology, ALL_TECHNOLOGIES {
    Spear => 0,
    Javelin => 10,
    Bow => 20,
    Crossbow => 30,
    Grenades => 40,
    HandCannon => 50,
    IronRam => 100,
    Catapult => 110,
    Trebuchet => 120,
    Ballista => 130,
    Cannon => 140,
    OrganGun => 150,
    Gambeson => 200,
    Leather => 210,
    ChainMail => 220,
    Plate => 230,
    WoodShielding => 300,
    IronShielding => 310,
    ReinforcedShielding => 320,
    LogRollers => 400,
    SolidWheels => 410,
    IronRimmedWheels => 420,
    SpokedWheels => 430,
});

impl Weapon {
    pub fn display_name(&self) -> &'static str {
        match self {
            Weapon::Unarmed => "Unarmed",
            Weapon::Dagger => "Dagger",
            Weapon::Sword => "Sword",
            Weapon::SwordAndShield => "Sword and shield",
            Weapon::Spear => "Spear",
            Weapon::Javelin => "Javelin",
            Weapon::Bow => "Bow",
            Weapon::Crossbow => "Crossbow",
            Weapon::Grenades => "Grenades",
            Weapon::HandCannon => "Hand cannon",
            Weapon::WoodRam => "Wood ram",
            Weapon::IronRam => "Iron ram",
            Weapon::Catapult => "Catapult",
            Weapon::Trebuchet => "Trebuchet",
            Weapon::Ballista => "Ballista",
            Weapon::Cannon => "Cannon",
            Weapon::OrganGun => "Organ gun",
        }
    }
}

impl Armor {
    pub fn display_name(&self) -> &'static str {
        match self {
            Armor::NoArmor => "No armor",
            Armor::Gambeson => "Gambeson",
            Armor::Leather => "Leather",
            Armor::ChainMail => "Chain mail",
            Armor::Plate => "Plate",
        }
    }
}

impl Shielding {
    pub fn display_name(&self) -> &'static str {
        match self {
            Shielding::NoShielding => "No shielding",
            Shielding::WoodShielding => "Wood shielding",
            Shielding::IronShielding => "Iron shielding",
            Shielding::ReinforcedShielding => "Reinforced shielding",
        }
    }
}

impl Chassis {
    pub fn display_name(&self) -> &'static str {
        match self {
            Chassis::Carried => "Carried",
            Chassis::LogRollers => "Log rollers",
            Chassis::SolidWheels => "Solid wheels",
            Chassis::IronRimmedWheels => "Iron rimmed wheels",
            Chassis::SpokedWheels => "Spoked wheels",
        }
    }
}

impl StructureSprite {
    pub fn display_name(&self) -> &'static str {
        match self {
            StructureSprite::WallWood | StructureSprite::WallWood2 => "Wood wall",
            StructureSprite::WallStone | StructureSprite::WallStone2 => "Stone wall",
            StructureSprite::TowerWood => "Wood tower",
            StructureSprite::TowerStone => "Stone tower",
            StructureSprite::Castle => "Castle",
            StructureSprite::House1
            | StructureSprite::House2
            | StructureSprite::House3
            | StructureSprite::House4 => "House",
            StructureSprite::Jail => "Jail",
            StructureSprite::University => "University",
            StructureSprite::MiningCamp => "Mining camp",
            StructureSprite::LumberCamp => "Lumber camp",
            StructureSprite::Blacksmith => "Blacksmith",
            StructureSprite::Storehouse => "Storehouse",
            StructureSprite::Fortress => "Fortress",
        }
    }
}

impl Technology {
    pub fn display_name(&self) -> &'static str {
        match self {
            Technology::Spear => Weapon::Spear.display_name(),
            Technology::Javelin => Weapon::Javelin.display_name(),
            Technology::Bow => Weapon::Bow.display_name(),
            Technology::Crossbow => Weapon::Crossbow.display_name(),
            Technology::Grenades => Weapon::Grenades.display_name(),
            Technology::HandCannon => Weapon::HandCannon.display_name(),
            Technology::IronRam => Weapon::IronRam.display_name(),
            Technology::Catapult => Weapon::Catapult.display_name(),
            Technology::Trebuchet => Weapon::Trebuchet.display_name(),
            Technology::Ballista => Weapon::Ballista.display_name(),
            Technology::Cannon => Weapon::Cannon.display_name(),
            Technology::OrganGun => Weapon::OrganGun.display_name(),
            Technology::Gambeson => Armor::Gambeson.display_name(),
            Technology::Leather => Armor::Leather.display_name(),
            Technology::ChainMail => Armor::ChainMail.display_name(),
            Technology::Plate => Armor::Plate.display_name(),
            Technology::WoodShielding => Shielding::WoodShielding.display_name(),
            Technology::IronShielding => Shielding::IronShielding.display_name(),
            Technology::ReinforcedShielding => Shielding::ReinforcedShielding.display_name(),
            Technology::LogRollers => Chassis::LogRollers.display_name(),
            Technology::SolidWheels => Chassis::SolidWheels.display_name(),
            Technology::IronRimmedWheels => Chassis::IronRimmedWheels.display_name(),
            Technology::SpokedWheels => Chassis::SpokedWheels.display_name(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TargetClass {
    Soldier,
    Machine,
    Structure,
}

#[derive(Clone, Copy, PartialEq, Debug, Default, SerBin, DeBin)]
pub struct Resources {
    pub men: i32,
    pub iron: i32,
    pub wood: i32,
    pub mercury: i32,
    pub sulfur: i32,
}

impl Resources {
    pub fn new(men: i32, iron: i32, wood: i32, mercury: i32, sulfur: i32) -> Self {
        Self {
            men,
            iron,
            wood,
            mercury,
            sulfur,
        }
    }

    pub fn starting() -> Self {
        Self::new(24, 100, 100, 0, 0)
    }

    pub fn covers(&self, cost: &Resources) -> bool {
        self.men >= cost.men
            && self.iron >= cost.iron
            && self.wood >= cost.wood
            && self.mercury >= cost.mercury
            && self.sulfur >= cost.sulfur
    }

    pub fn add(&mut self, other: &Resources) {
        self.men += other.men;
        self.iron += other.iron;
        self.wood += other.wood;
        self.mercury += other.mercury;
        self.sulfur += other.sulfur;
    }

    pub fn subtract(&mut self, other: &Resources) {
        self.men -= other.men;
        self.iron -= other.iron;
        self.wood -= other.wood;
        self.mercury -= other.mercury;
        self.sulfur -= other.sulfur;
    }

    pub fn scaled_down(&self, numerator: i32, denominator: i32) -> Resources {
        Resources::new(
            self.men * numerator / denominator,
            self.iron * numerator / denominator,
            self.wood * numerator / denominator,
            self.mercury * numerator / denominator,
            self.sulfur * numerator / denominator,
        )
    }

    pub fn scaled_up(&self, numerator: i32, denominator: i32) -> Resources {
        Resources::new(
            (self.men * numerator + denominator - 1) / denominator,
            (self.iron * numerator + denominator - 1) / denominator,
            (self.wood * numerator + denominator - 1) / denominator,
            (self.mercury * numerator + denominator - 1) / denominator,
            (self.sulfur * numerator + denominator - 1) / denominator,
        )
    }

    pub fn is_empty(&self) -> bool {
        self.men == 0 && self.iron == 0 && self.wood == 0 && self.mercury == 0 && self.sulfur == 0
    }

    pub fn amounts(&self) -> [i32; RESOURCE_COUNT] {
        [self.men, self.iron, self.wood, self.mercury, self.sulfur]
    }
}

pub const RESOURCE_COUNT: usize = 5;
pub const RESOURCE_MEN: usize = 0;
pub const RESOURCE_NAMES: [&str; RESOURCE_COUNT] = ["Men", "Iron", "Wood", "Mercury", "Sulfur"];

#[derive(Clone, Copy, PartialEq, Debug, Default, SerBin, DeBin)]
pub struct Reward {
    pub resources: Resources,
    pub technology: Option<Technology>,
}

#[derive(Clone, Copy, PartialEq, Debug, SerBin, DeBin)]
pub struct UnitDesign {
    pub kind: UnitKind,
    pub weapon: Weapon,
    pub armor: Armor,
    pub shielding: Shielding,
    pub chassis: Chassis,
}

impl UnitDesign {
    pub fn default_for(kind: UnitKind) -> Self {
        let weapon = match kind {
            UnitKind::Soldier => Weapon::Dagger,
            UnitKind::Machine => Weapon::WoodRam,
        };
        Self {
            kind,
            weapon,
            armor: Armor::NoArmor,
            shielding: Shielding::NoShielding,
            chassis: Chassis::Carried,
        }
    }

    pub fn write_name(&self, output: &mut String) {
        output.push_str(self.weapon.name());
        output.push(' ');
        match self.kind {
            UnitKind::Soldier => output.push_str(self.armor.name()),
            UnitKind::Machine => {
                output.push_str(self.shielding.name());
                output.push(' ');
                output.push_str(self.chassis.name());
            }
        }
    }

    pub fn write_display_name(&self, output: &mut String) {
        output.push_str(self.weapon.display_name());
        output.push_str(", ");
        match self.kind {
            UnitKind::Soldier => output.push_str(self.armor.display_name()),
            UnitKind::Machine => {
                output.push_str(self.shielding.display_name());
                output.push_str(", ");
                output.push_str(self.chassis.display_name());
            }
        }
    }
}

pub fn unit_type_for(kind: UnitKind) -> UnitType {
    match kind {
        UnitKind::Soldier => UnitType::Soldier,
        UnitKind::Machine => UnitType::Machine,
    }
}

#[derive(Clone, Copy, PartialEq, Debug, SerBin, DeBin)]
pub struct UnitProfile {
    pub design: UnitDesign,
    pub aggro_range: u8,
    pub reward: Reward,
    pub is_final_fortress: bool,
    pub structure_sprite: StructureSprite,
    pub structure_hp: u16,
}

impl UnitProfile {
    pub fn new(design: UnitDesign) -> Self {
        Self {
            design,
            aggro_range: 0,
            reward: Reward::default(),
            is_final_fortress: false,
            structure_sprite: StructureSprite::WallWood,
            structure_hp: 0,
        }
    }
}

pub fn structure_profile(structure_sprite: StructureSprite) -> UnitProfile {
    let mut design = UnitDesign::default_for(UnitKind::Soldier);
    design.weapon = Weapon::Unarmed;
    let structure_hp = match structure_sprite {
        StructureSprite::WallWood | StructureSprite::WallWood2 => 200,
        StructureSprite::WallStone | StructureSprite::WallStone2 => 400,
        StructureSprite::TowerWood => {
            design.weapon = Weapon::Bow;
            200
        }
        StructureSprite::TowerStone => {
            design.weapon = Weapon::Crossbow;
            500
        }
        StructureSprite::Castle => {
            design.weapon = Weapon::OrganGun;
            700
        }
        StructureSprite::House1 => 50,
        StructureSprite::House2 | StructureSprite::House3 | StructureSprite::House4 => 100,
        StructureSprite::Jail | StructureSprite::University => 120,
        StructureSprite::MiningCamp | StructureSprite::LumberCamp => 100,
        StructureSprite::Blacksmith => 200,
        StructureSprite::Storehouse => 150,
        StructureSprite::Fortress => {
            design.weapon = Weapon::Cannon;
            1000
        }
    };
    let mut profile = UnitProfile::new(design);
    profile.structure_sprite = structure_sprite;
    profile.structure_hp = structure_hp;
    profile.aggro_range = enemy_aggro_range(UnitType::Structure, design.weapon);
    profile
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Economy {
    pub resources: Resources,
    pub technology: u32,
}

impl Economy {
    pub fn starting() -> Self {
        Self {
            resources: Resources::starting(),
            technology: 0,
        }
    }

    pub fn is_unlocked(&self, technology: Option<Technology>) -> bool {
        match technology {
            Some(technology) => self.technology & technology_bit(technology) != 0,
            None => true,
        }
    }
}

pub struct WeaponStats {
    pub kind: UnitKind,
    pub range: i32,
    pub attack_period: u32,
    pub damage: [i32; 3],
    pub splash_radius_squared: i32,
    pub attacks_while_moving: bool,
    pub armor_bonus_hp: i32,
    pub cost: Resources,
    pub technology: Option<Technology>,
}

pub struct PartStats {
    pub hp: i32,
    pub move_period: u8,
    pub cost: Resources,
    pub technology: Option<Technology>,
}

pub fn technology_bit(technology: Technology) -> u32 {
    1 << technology.to_sequential_index()
}

pub const DAMAGE_VS_SOLDIER: usize = 0;
pub const DAMAGE_VS_MACHINE: usize = 1;
pub const DAMAGE_VS_STRUCTURE: usize = 2;

pub fn target_class_index(target_class: TargetClass) -> usize {
    match target_class {
        TargetClass::Soldier => DAMAGE_VS_SOLDIER,
        TargetClass::Machine => DAMAGE_VS_MACHINE,
        TargetClass::Structure => DAMAGE_VS_STRUCTURE,
    }
}

fn weapon(
    kind: UnitKind,
    range: i32,
    attack_period: u32,
    damage: [i32; 3],
    splash_radius_squared: i32,
    attacks_while_moving: bool,
    cost: Resources,
    technology: Option<Technology>,
) -> WeaponStats {
    WeaponStats {
        kind,
        range,
        attack_period,
        damage,
        splash_radius_squared,
        attacks_while_moving,
        armor_bonus_hp: 0,
        cost,
        technology,
    }
}

pub fn weapon_stats(weapon_type: Weapon) -> WeaponStats {
    let soldier = UnitKind::Soldier;
    let machine = UnitKind::Machine;
    match weapon_type {
        Weapon::Unarmed => weapon(
            soldier,
            1,
            2,
            [2, 1, 1],
            0,
            false,
            Resources::default(),
            None,
        ),
        Weapon::Dagger => weapon(
            soldier,
            1,
            1,
            [4, 2, 1],
            0,
            false,
            Resources::new(0, 5, 0, 0, 0),
            None,
        ),
        Weapon::Sword => weapon(
            soldier,
            1,
            2,
            [8, 4, 1],
            0,
            false,
            Resources::new(0, 15, 0, 0, 0),
            None,
        ),
        Weapon::SwordAndShield => {
            let mut stats = weapon(
                soldier,
                1,
                2,
                [8, 4, 1],
                0,
                false,
                Resources::new(0, 15, 10, 0, 0),
                None,
            );
            stats.armor_bonus_hp = 20;
            stats
        }
        Weapon::Spear => weapon(
            soldier,
            1,
            3,
            [12, 6, 2],
            0,
            false,
            Resources::new(0, 5, 10, 0, 0),
            Some(Technology::Spear),
        ),
        Weapon::Javelin => weapon(
            soldier,
            4,
            5,
            [12, 3, 1],
            0,
            true,
            Resources::new(0, 4, 12, 0, 0),
            Some(Technology::Javelin),
        ),
        Weapon::Bow => weapon(
            soldier,
            6,
            4,
            [9, 2, 1],
            0,
            true,
            Resources::new(0, 0, 15, 0, 0),
            Some(Technology::Bow),
        ),
        Weapon::Crossbow => weapon(
            soldier,
            7,
            6,
            [16, 4, 2],
            0,
            true,
            Resources::new(0, 10, 15, 0, 0),
            Some(Technology::Crossbow),
        ),
        Weapon::Grenades => weapon(
            soldier,
            3,
            5,
            [10, 10, 10],
            1,
            true,
            Resources::new(0, 5, 0, 0, 10),
            Some(Technology::Grenades),
        ),
        Weapon::HandCannon => weapon(
            soldier,
            8,
            7,
            [24, 12, 4],
            0,
            true,
            Resources::new(0, 20, 0, 5, 10),
            Some(Technology::HandCannon),
        ),
        Weapon::WoodRam => weapon(
            machine,
            1,
            4,
            [2, 16, 16],
            0,
            false,
            Resources::new(0, 0, 30, 0, 0),
            None,
        ),
        Weapon::IronRam => weapon(
            machine,
            1,
            4,
            [3, 26, 26],
            0,
            false,
            Resources::new(0, 20, 30, 0, 0),
            Some(Technology::IronRam),
        ),
        Weapon::Catapult => weapon(
            machine,
            10,
            8,
            [14, 14, 14],
            5,
            false,
            Resources::new(0, 15, 40, 0, 0),
            Some(Technology::Catapult),
        ),
        Weapon::Trebuchet => weapon(
            machine,
            13,
            10,
            [12, 12, 36],
            2,
            false,
            Resources::new(0, 20, 80, 0, 0),
            Some(Technology::Trebuchet),
        ),
        Weapon::Ballista => weapon(
            machine,
            8,
            3,
            [18, 9, 5],
            0,
            false,
            Resources::new(0, 20, 40, 0, 0),
            Some(Technology::Ballista),
        ),
        Weapon::Cannon => weapon(
            machine,
            11,
            7,
            [22, 22, 22],
            4,
            false,
            Resources::new(0, 80, 0, 0, 20),
            Some(Technology::Cannon),
        ),
        Weapon::OrganGun => weapon(
            machine,
            9,
            3,
            [16, 8, 5],
            0,
            false,
            Resources::new(0, 40, 20, 10, 15),
            Some(Technology::OrganGun),
        ),
    }
}

fn part(hp: i32, move_period: u8, cost: Resources, technology: Option<Technology>) -> PartStats {
    PartStats {
        hp,
        move_period,
        cost,
        technology,
    }
}

pub fn armor_stats(armor: Armor) -> PartStats {
    match armor {
        Armor::NoArmor => part(60, 1, Resources::default(), None),
        Armor::Gambeson => part(
            80,
            2,
            Resources::new(0, 0, 10, 0, 0),
            Some(Technology::Gambeson),
        ),
        Armor::Leather => part(
            90,
            2,
            Resources::new(0, 0, 15, 0, 0),
            Some(Technology::Leather),
        ),
        Armor::ChainMail => part(
            120,
            3,
            Resources::new(0, 25, 0, 0, 0),
            Some(Technology::ChainMail),
        ),
        Armor::Plate => part(
            160,
            4,
            Resources::new(0, 50, 0, 0, 0),
            Some(Technology::Plate),
        ),
    }
}

pub fn shielding_stats(shielding: Shielding) -> PartStats {
    match shielding {
        Shielding::NoShielding => part(100, 0, Resources::default(), None),
        Shielding::WoodShielding => part(
            160,
            0,
            Resources::new(0, 0, 20, 0, 0),
            Some(Technology::WoodShielding),
        ),
        Shielding::IronShielding => part(
            240,
            0,
            Resources::new(0, 20, 10, 0, 0),
            Some(Technology::IronShielding),
        ),
        Shielding::ReinforcedShielding => part(
            320,
            0,
            Resources::new(0, 45, 10, 0, 0),
            Some(Technology::ReinforcedShielding),
        ),
    }
}

pub fn chassis_stats(chassis: Chassis) -> PartStats {
    match chassis {
        Chassis::Carried => part(0, 6, Resources::default(), None),
        Chassis::LogRollers => part(
            0,
            5,
            Resources::new(0, 0, 10, 0, 0),
            Some(Technology::LogRollers),
        ),
        Chassis::SolidWheels => part(
            0,
            4,
            Resources::new(0, 0, 20, 0, 0),
            Some(Technology::SolidWheels),
        ),
        Chassis::IronRimmedWheels => part(
            0,
            3,
            Resources::new(0, 10, 20, 0, 0),
            Some(Technology::IronRimmedWheels),
        ),
        Chassis::SpokedWheels => part(
            0,
            2,
            Resources::new(0, 25, 20, 0, 0),
            Some(Technology::SpokedWheels),
        ),
    }
}

pub fn design_cost(design: &UnitDesign) -> Resources {
    let mut cost = Resources::new(MEN_PER_UNIT as i32, 0, 0, 0, 0);
    cost.add(&weapon_stats(design.weapon).cost);
    match design.kind {
        UnitKind::Soldier => cost.add(&armor_stats(design.armor).cost),
        UnitKind::Machine => {
            cost.add(&shielding_stats(design.shielding).cost);
            cost.add(&chassis_stats(design.chassis).cost);
        }
    }
    cost
}

pub fn design_cost_for_men(design: &UnitDesign, men: i32) -> Resources {
    let full = design_cost(design);
    let mut cost = match design.kind {
        UnitKind::Soldier => full.scaled_up(men, MEN_PER_UNIT as i32),
        UnitKind::Machine => full,
    };
    cost.men = men;
    cost
}

pub fn is_design_available(design: &UnitDesign, economy: &Economy) -> bool {
    let weapon = weapon_stats(design.weapon);
    if weapon.kind != design.kind || !economy.is_unlocked(weapon.technology) {
        return false;
    }
    match design.kind {
        UnitKind::Soldier => economy.is_unlocked(armor_stats(design.armor).technology),
        UnitKind::Machine => {
            economy.is_unlocked(shielding_stats(design.shielding).technology)
                && economy.is_unlocked(chassis_stats(design.chassis).technology)
        }
    }
}

pub fn men_from_hp(hp: u16, max_hp: u16) -> u16 {
    if hp == 0 || max_hp == 0 {
        return 0;
    }
    let men = (MEN_PER_UNIT as u32 * hp as u32 + max_hp as u32 - 1) / max_hp as u32;
    men.min(MEN_PER_UNIT as u32) as u16
}

pub fn default_profile(unit_type: UnitType, structure_sprite: StructureSprite) -> UnitProfile {
    let mut profile = match unit_type {
        UnitType::Soldier => UnitProfile::new(UnitDesign::default_for(UnitKind::Soldier)),
        UnitType::Machine => UnitProfile::new(UnitDesign::default_for(UnitKind::Machine)),
        UnitType::Structure => return structure_profile(structure_sprite),
    };
    profile.aggro_range = enemy_aggro_range(unit_type, profile.design.weapon);
    profile
}

pub fn enemy_aggro_range(unit_type: UnitType, weapon: Weapon) -> u8 {
    if weapon == Weapon::Unarmed {
        return 0;
    }
    let range = weapon_stats(weapon).range as u8;
    if unit_type.is_structure() {
        return range;
    }
    if range <= 1 {
        return ENEMY_MELEE_AGGRO_RANGE;
    }
    range + ENEMY_AGGRO_PAST_RANGE
}

pub fn unit_max_hp(unit_type: UnitType, profile: &UnitProfile) -> u16 {
    let design = &profile.design;
    match unit_type {
        UnitType::Soldier => {
            (armor_stats(design.armor).hp + weapon_stats(design.weapon).armor_bonus_hp) as u16
        }
        UnitType::Machine => shielding_stats(design.shielding).hp as u16,
        UnitType::Structure => profile.structure_hp,
    }
}

pub fn unit_move_period(unit_type: UnitType, design: &UnitDesign) -> u8 {
    match unit_type {
        UnitType::Soldier => armor_stats(design.armor).move_period,
        UnitType::Machine => chassis_stats(design.chassis).move_period,
        UnitType::Structure => 0,
    }
}

pub fn target_class(unit_type: UnitType) -> TargetClass {
    match unit_type {
        UnitType::Soldier => TargetClass::Soldier,
        UnitType::Machine => TargetClass::Machine,
        UnitType::Structure => TargetClass::Structure,
    }
}

pub fn unit_name(unit_type: UnitType, profile: &UnitProfile) -> &'static str {
    match unit_type {
        UnitType::Structure => profile.structure_sprite.display_name(),
        _ => unit_type.name(),
    }
}

pub fn can_attack(unit_type: UnitType, design: &UnitDesign) -> bool {
    !(unit_type.is_structure() && design.weapon == Weapon::Unarmed)
}

pub fn crew_scaled_period(period: u32, men: u16) -> u32 {
    let men = men.max(1) as u32;
    (period * MEN_PER_UNIT as u32 + men - 1) / men
}

pub fn sell_refund(design: &UnitDesign, hp: u16, max_hp: u16, men: u16) -> Resources {
    let mut refund = Resources::default();
    if max_hp > 0 {
        refund = design_cost(design).scaled_down(hp as i32, max_hp as i32);
    }
    refund.men = men as i32;
    refund
}
