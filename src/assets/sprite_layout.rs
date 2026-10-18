use macroquad::prelude::Vec2;

use crate::unit_design::{Armor, Chassis, Shielding, StructureSprite, Weapon};

pub const MACHINE_CREW_SLOTS: [Vec2; 6] = [
    Vec2::new(2.0, 3.0),
    Vec2::new(4.0, 8.0),
    Vec2::new(7.0, 3.0),
    Vec2::new(9.0, 8.0),
    Vec2::new(-3.0, 3.0),
    Vec2::new(-1.0, 8.0),
];
const MACHINE_CREW_SIDES: u16 = 2;

pub fn is_far_crew(k: u16) -> bool {
    k % MACHINE_CREW_SIDES == 0
}

const FLAMES_BLOCK: [Vec2; 4] = [
    Vec2::new(4.0, 6.0),
    Vec2::new(11.0, 8.0),
    Vec2::new(6.0, 12.0),
    Vec2::new(12.0, 15.0),
];
const FLAMES_TOWER: [Vec2; 3] = [
    Vec2::new(6.0, 9.0),
    Vec2::new(11.0, 6.0),
    Vec2::new(11.0, 14.0),
];
const FLAMES_JAIL: [Vec2; 3] = [
    Vec2::new(3.0, 6.0),
    Vec2::new(10.0, 9.0),
    Vec2::new(13.0, 15.0),
];
const FLAMES_UNIVERSITY: [Vec2; 3] = [
    Vec2::new(7.0, 8.0),
    Vec2::new(4.0, 13.0),
    Vec2::new(12.0, 15.0),
];
const FLAMES_STOREHOUSE: [Vec2; 3] = [
    Vec2::new(4.0, 6.0),
    Vec2::new(10.0, 10.0),
    Vec2::new(12.0, 15.0),
];
const FLAMES_HOUSE_LOW: [Vec2; 2] = [Vec2::new(6.0, 14.0), Vec2::new(10.0, 15.0)];
const FLAMES_HOUSE_TALL: [Vec2; 2] = [Vec2::new(6.0, 12.0), Vec2::new(10.0, 14.0)];
const FLAMES_MINING_CAMP: [Vec2; 2] = [Vec2::new(5.0, 13.0), Vec2::new(10.0, 15.0)];
const FLAMES_LUMBER_CAMP: [Vec2; 2] = [Vec2::new(13.0, 9.0), Vec2::new(4.0, 16.0)];
const FLAMES_BLACKSMITH: [Vec2; 2] = [Vec2::new(4.0, 13.0), Vec2::new(12.0, 14.0)];
const FLAMES_CHASSIS: [Vec2; 2] = [Vec2::new(5.0, 11.0), Vec2::new(10.0, 13.0)];

pub fn structure_flame_bases(structure_sprite: StructureSprite) -> &'static [Vec2] {
    match structure_sprite {
        StructureSprite::WallWood
        | StructureSprite::WallWood2
        | StructureSprite::WallStone
        | StructureSprite::WallStone2
        | StructureSprite::Castle
        | StructureSprite::Fortress => &FLAMES_BLOCK,
        StructureSprite::TowerWood | StructureSprite::TowerStone => &FLAMES_TOWER,
        StructureSprite::Jail => &FLAMES_JAIL,
        StructureSprite::University => &FLAMES_UNIVERSITY,
        StructureSprite::Storehouse => &FLAMES_STOREHOUSE,
        StructureSprite::House1 | StructureSprite::House3 | StructureSprite::House4 => {
            &FLAMES_HOUSE_LOW
        }
        StructureSprite::House2 => &FLAMES_HOUSE_TALL,
        StructureSprite::MiningCamp => &FLAMES_MINING_CAMP,
        StructureSprite::LumberCamp => &FLAMES_LUMBER_CAMP,
        StructureSprite::Blacksmith => &FLAMES_BLACKSMITH,
    }
}

pub fn chassis_flame_bases(chassis: Chassis) -> &'static [Vec2] {
    match chassis {
        Chassis::Carried
        | Chassis::LogRollers
        | Chassis::SolidWheels
        | Chassis::IronRimmedWheels
        | Chassis::SpokedWheels => &FLAMES_CHASSIS,
    }
}

#[derive(Clone, Copy)]
pub struct ImpactPoints {
    pub left: Vec2,
    pub right: Vec2,
}

const fn impact_points(left_x: f32, left_y: f32, right_x: f32, right_y: f32) -> ImpactPoints {
    ImpactPoints {
        left: Vec2::new(left_x, left_y),
        right: Vec2::new(right_x, right_y),
    }
}

pub const MAN_BASE_IMPACT_POINTS: ImpactPoints = impact_points(3.0, 3.0, 5.0, 3.0);

pub fn man_armor_impact_points(armor: Armor) -> Option<ImpactPoints> {
    match armor {
        Armor::NoArmor => None,
        Armor::Gambeson | Armor::Leather | Armor::ChainMail | Armor::Plate => {
            Some(impact_points(3.0, 5.0, 5.0, 5.0))
        }
    }
}

pub fn man_weapon_impact_points(weapon: Weapon) -> Option<ImpactPoints> {
    match weapon {
        Weapon::Unarmed
        | Weapon::WoodRam
        | Weapon::IronRam
        | Weapon::Catapult
        | Weapon::Trebuchet
        | Weapon::Ballista
        | Weapon::Cannon
        | Weapon::OrganGun => None,
        Weapon::Dagger | Weapon::Sword | Weapon::Spear | Weapon::Grenades => {
            Some(impact_points(6.0, 4.0, 6.0, 4.0))
        }
        Weapon::SwordAndShield => Some(impact_points(2.0, 4.0, 6.0, 4.0)),
        Weapon::Javelin => Some(impact_points(2.0, 6.0, 7.0, 1.0)),
        Weapon::Bow => Some(impact_points(4.0, 5.0, 7.0, 5.0)),
        Weapon::Crossbow => Some(impact_points(3.0, 5.0, 7.0, 5.0)),
        Weapon::HandCannon => Some(impact_points(3.0, 5.0, 6.0, 5.0)),
    }
}

pub fn chassis_impact_points(chassis: Chassis) -> ImpactPoints {
    match chassis {
        Chassis::LogRollers => impact_points(0.0, 9.0, 14.0, 12.0),
        Chassis::Carried
        | Chassis::SolidWheels
        | Chassis::IronRimmedWheels
        | Chassis::SpokedWheels => impact_points(1.0, 8.0, 14.0, 12.0),
    }
}

pub fn shielding_impact_points(shielding: Shielding) -> Option<ImpactPoints> {
    match shielding {
        Shielding::NoShielding => None,
        Shielding::WoodShielding | Shielding::IronShielding | Shielding::ReinforcedShielding => {
            Some(impact_points(1.0, 8.0, 14.0, 12.0))
        }
    }
}

pub fn machine_weapon_impact_points(weapon: Weapon) -> Option<ImpactPoints> {
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
        | Weapon::HandCannon => None,
        Weapon::WoodRam | Weapon::IronRam => Some(impact_points(3.0, 10.0, 15.0, 11.0)),
        Weapon::Catapult => Some(impact_points(0.0, 7.0, 10.0, 10.0)),
        Weapon::Trebuchet => Some(impact_points(1.0, 9.0, 10.0, 10.0)),
        Weapon::Ballista => Some(impact_points(3.0, 8.0, 10.0, 8.0)),
        Weapon::Cannon => Some(impact_points(3.0, 8.0, 14.0, 9.0)),
        Weapon::OrganGun => Some(impact_points(5.0, 9.0, 15.0, 11.0)),
    }
}

pub fn structure_impact_points(structure_sprite: StructureSprite) -> ImpactPoints {
    match structure_sprite {
        StructureSprite::WallWood
        | StructureSprite::WallWood2
        | StructureSprite::WallStone
        | StructureSprite::WallStone2
        | StructureSprite::Castle
        | StructureSprite::Jail
        | StructureSprite::Storehouse
        | StructureSprite::Fortress => impact_points(0.0, 8.0, 15.0, 8.0),
        StructureSprite::TowerWood | StructureSprite::TowerStone => {
            impact_points(0.0, 3.0, 15.0, 3.0)
        }
        StructureSprite::House1 | StructureSprite::House3 => impact_points(2.0, 12.0, 13.0, 12.0),
        StructureSprite::House2 => impact_points(2.0, 9.0, 13.0, 9.0),
        StructureSprite::House4 => impact_points(2.0, 11.0, 13.0, 11.0),
        StructureSprite::University => impact_points(1.0, 9.0, 14.0, 9.0),
        StructureSprite::MiningCamp => impact_points(1.0, 15.0, 13.0, 12.0),
        StructureSprite::LumberCamp => impact_points(1.0, 6.0, 15.0, 6.0),
        StructureSprite::Blacksmith => impact_points(1.0, 12.0, 13.0, 12.0),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        chassis_flame_bases, chassis_impact_points, machine_weapon_impact_points,
        man_armor_impact_points, man_weapon_impact_points, shielding_impact_points,
        structure_flame_bases, structure_impact_points, ImpactPoints, MAN_BASE_IMPACT_POINTS,
    };
    use crate::assets::asset_path::{
        MACHINE_CHASSIS_LAYERS, MACHINE_SHIELDING_LAYERS, MACHINE_WEAPON_LAYERS, MAN_ARMOR_LAYERS,
        MAN_BASE, MAN_WEAPON_LAYERS, STRUCTURE_SPRITES,
    };
    use crate::unit_design::{
        ALL_ARMORS, ALL_CHASSIS, ALL_SHIELDINGS, ALL_STRUCTURE_SPRITES, ALL_WEAPONS,
    };
    use crate::unit_manager::{
        machine_chassis_slot, machine_shielding_slot, machine_weapon_slot, man_armor_slot,
        man_weapon_slot,
    };
    use image::RgbaImage;
    use macroquad::prelude::Vec2;

    fn assert_bases_on_drawn_pixels(path: &str, bases: &[Vec2]) {
        let image: RgbaImage = image::open(path).unwrap().to_rgba8();
        for base in bases {
            let x = base.x as u32;
            let y = base.y as u32 - 1;
            assert!(
                image.get_pixel(x - 1, y)[3] > 0,
                "{path}: flame base {base} on empty pixel"
            );
            assert!(
                image.get_pixel(x, y)[3] > 0,
                "{path}: flame base {base} on empty pixel"
            );
        }
    }

    #[test]
    fn every_flame_base_sits_on_its_sprite() {
        for &structure_sprite in ALL_STRUCTURE_SPRITES {
            assert_bases_on_drawn_pixels(
                STRUCTURE_SPRITES[structure_sprite as usize],
                structure_flame_bases(structure_sprite),
            );
        }
        for &chassis in ALL_CHASSIS {
            assert_bases_on_drawn_pixels(
                MACHINE_CHASSIS_LAYERS[machine_chassis_slot(chassis)],
                chassis_flame_bases(chassis),
            );
        }
    }

    fn assert_impact_points_outermost(path: &str, points: ImpactPoints) {
        let image: RgbaImage = image::open(path).unwrap().to_rgba8();
        let (width, height) = image.dimensions();
        let mut min_x = width;
        let mut max_x = 0;
        for y in 0..height {
            for x in 0..width {
                if image.get_pixel(x, y)[3] > 0 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
        }
        let left = points.left;
        let right = points.right;
        assert_eq!(
            left.x as u32, min_x,
            "{path}: left impact point not outermost"
        );
        assert_eq!(
            right.x as u32, max_x,
            "{path}: right impact point not outermost"
        );
        assert!(
            image.get_pixel(left.x as u32, left.y as u32)[3] > 0,
            "{path}: left impact point {left} on empty pixel"
        );
        assert!(
            image.get_pixel(right.x as u32, right.y as u32)[3] > 0,
            "{path}: right impact point {right} on empty pixel"
        );
        assert_eq!(
            left.y as u32,
            row_nearest_middle(&image, min_x),
            "{path}: left impact point not nearest the middle row"
        );
        assert_eq!(
            right.y as u32,
            row_nearest_middle(&image, max_x),
            "{path}: right impact point not nearest the middle row"
        );
    }

    fn row_nearest_middle(image: &RgbaImage, x: u32) -> u32 {
        let middle = (image.height() / 2) as i32;
        let mut nearest = 0;
        let mut nearest_distance = i32::MAX;
        for y in 0..image.height() {
            let distance = (y as i32 - middle).abs();
            if image.get_pixel(x, y)[3] > 0 && distance < nearest_distance {
                nearest = y;
                nearest_distance = distance;
            }
        }
        nearest
    }

    #[test]
    fn every_impact_point_is_outermost_on_its_sprite() {
        assert_impact_points_outermost(MAN_BASE, MAN_BASE_IMPACT_POINTS);
        for &armor in ALL_ARMORS {
            if let Some(points) = man_armor_impact_points(armor) {
                assert_impact_points_outermost(
                    MAN_ARMOR_LAYERS[man_armor_slot(armor)].unwrap(),
                    points,
                );
            }
        }
        for &weapon in ALL_WEAPONS {
            if let Some(points) = man_weapon_impact_points(weapon) {
                assert_impact_points_outermost(
                    MAN_WEAPON_LAYERS[man_weapon_slot(weapon)].unwrap(),
                    points,
                );
            }
            if let Some(points) = machine_weapon_impact_points(weapon) {
                assert_impact_points_outermost(
                    MACHINE_WEAPON_LAYERS[machine_weapon_slot(weapon)].unwrap(),
                    points,
                );
            }
        }
        for &chassis in ALL_CHASSIS {
            assert_impact_points_outermost(
                MACHINE_CHASSIS_LAYERS[machine_chassis_slot(chassis)],
                chassis_impact_points(chassis),
            );
        }
        for &shielding in ALL_SHIELDINGS {
            if let Some(points) = shielding_impact_points(shielding) {
                assert_impact_points_outermost(
                    MACHINE_SHIELDING_LAYERS[machine_shielding_slot(shielding)].unwrap(),
                    points,
                );
            }
        }
        for &structure_sprite in ALL_STRUCTURE_SPRITES {
            assert_impact_points_outermost(
                STRUCTURE_SPRITES[structure_sprite as usize],
                structure_impact_points(structure_sprite),
            );
        }
    }
}
