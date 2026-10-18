use crate::assets::asset_path::asset_path;
use crate::unit_design::RESOURCE_COUNT;
use crate::unit_manager::UnitType;

pub const PITCH_SHIFT_RATES: &[f32] = &[0.94, 0.97, 1.0, 1.03, 1.06];
pub const NO_PITCH_SHIFT: &[f32] = &[1.0];

macro_rules! sfx_events {
    ($($variant:ident => $file:literal, $rates:expr),* $(,)?) => {
        #[derive(Clone, Copy)]
        pub enum SfxEvent {
            $($variant,)*
        }

        pub const SFX_PATHS: &[&str] = &[$(asset_path!(concat!("sfx/", $file)),)*];
        pub const SFX_RATES: &[&[f32]] = &[$($rates,)*];
        pub const SFX_COUNT: usize = SFX_PATHS.len();
    };
}

sfx_events! {
    UnitPlacedSoldier => "unit_placed_soldier.ogg", PITCH_SHIFT_RATES,
    UnitPlacedMachine => "unit_placed_machine.ogg", PITCH_SHIFT_RATES,
    UnitSoldSoldier => "unit_sold_soldier.ogg", PITCH_SHIFT_RATES,
    UnitSoldMachine => "unit_sold_machine.ogg", PITCH_SHIFT_RATES,
    MoveStartSoldier => "move_start_soldier.ogg", PITCH_SHIFT_RATES,
    MoveStartMachine => "move_start_machine.ogg", PITCH_SHIFT_RATES,
    TargetSetSoldier => "target_set_soldier.ogg", PITCH_SHIFT_RATES,
    TargetSetMachine => "target_set_machine.ogg", PITCH_SHIFT_RATES,
    SectorTransitionSuccess => "sector_transition_success.ogg", PITCH_SHIFT_RATES,
    SectorTransitionDeny => "sector_transition_deny.ogg", PITCH_SHIFT_RATES,
    UiButtonClick => "ui_button_click.ogg", NO_PITCH_SHIFT,
    UiPanelClose => "ui_panel_close.ogg", NO_PITCH_SHIFT,
    GameWin => "game_win.ogg", NO_PITCH_SHIFT,
    GameLose => "game_lose.ogg", NO_PITCH_SHIFT,
    GatherResourceMan => "gather_resource_man.ogg", NO_PITCH_SHIFT,
    GatherResourceStone => "gather_resource_stone.ogg", NO_PITCH_SHIFT,
    GatherResourceWood => "gather_resource_wood.ogg", NO_PITCH_SHIFT,
    GatherResourceMercury => "gather_resource_mercury.ogg", NO_PITCH_SHIFT,
    GatherResourceSulfur => "gather_resource_sulfur.ogg", NO_PITCH_SHIFT,
    GatherTechnology => "gather_technology.ogg", NO_PITCH_SHIFT,
    HitMetal => "hit_metal.ogg", PITCH_SHIFT_RATES,
    HitRamWood => "hit_ram_wood.ogg", PITCH_SHIFT_RATES,
    HitRamIron => "hit_ram_iron.ogg", PITCH_SHIFT_RATES,
    HitArrowSmall => "hit_arrow_small.ogg", PITCH_SHIFT_RATES,
    HitArrowLarge => "hit_arrow_large.ogg", PITCH_SHIFT_RATES,
    HitBullet => "hit_bullet.ogg", PITCH_SHIFT_RATES,
    ExplosionSmall => "explosion_small.ogg", PITCH_SHIFT_RATES,
    ExplosionMedium => "explosion_medium.ogg", PITCH_SHIFT_RATES,
    ExplosionLarge => "explosion_large.ogg", PITCH_SHIFT_RATES,
    ShotArrowSmall => "shot_arrow_small.ogg", PITCH_SHIFT_RATES,
    ShotArrowLarge => "shot_arrow_large.ogg", PITCH_SHIFT_RATES,
    ShotStone => "shot_stone.ogg", PITCH_SHIFT_RATES,
    ShotCannon => "shot_cannon.ogg", PITCH_SHIFT_RATES,
    ShotBullet => "shot_bullet.ogg", PITCH_SHIFT_RATES,
    DestroyMachine => "destroy_machine.ogg", PITCH_SHIFT_RATES,
    DestroyStructure => "destroy_structure.ogg", PITCH_SHIFT_RATES,
    DestroyMan => "destroy_man.ogg", PITCH_SHIFT_RATES,
}

pub const GATHER_RESOURCE_SFX: [SfxEvent; RESOURCE_COUNT] = [
    SfxEvent::GatherResourceMan,
    SfxEvent::GatherResourceStone,
    SfxEvent::GatherResourceWood,
    SfxEvent::GatherResourceMercury,
    SfxEvent::GatherResourceSulfur,
];

pub fn unit_sfx(unit_type: UnitType, soldier: SfxEvent, machine: SfxEvent) -> SfxEvent {
    if unit_type == UnitType::Soldier {
        return soldier;
    }
    machine
}
