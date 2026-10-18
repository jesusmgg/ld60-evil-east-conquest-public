use macroquad::prelude::*;
use nanoserde::{DeBin, SerBin};

use std::f32::consts::PI;

use crate::assets::AssetManager;
use crate::enum_wrapper_serbin;
use crate::enums::Direction;
use crate::game_data::proxy::IVec2Proxy;
use crate::level::level_manager::Level;
use crate::simple_enum_serbin;

#[derive(Clone)]
pub enum Interactable {
    LightSource(LightSource),
    Rubble(Rubble),
    Gate(Gate),
}

impl Interactable {
    pub fn name(&self) -> &'static str {
        match self {
            Interactable::LightSource(_) => "LightSource",
            Interactable::Rubble(_) => "Rubble",
            Interactable::Gate(_) => "Gate",
        }
    }

    pub fn render(&self, position: IVec2, grid_pos: IVec2, level: &Level, assets: &AssetManager) {
        match self {
            Interactable::LightSource(_) => {}
            Interactable::Rubble(rubble) => rubble.render(position, grid_pos, level, assets),
            Interactable::Gate(gate) => gate.render(position, grid_pos, level, assets),
        }
    }
}

enum_wrapper_serbin!(Interactable, ALL_INTERACTABLES {
    LightSource(LightSource) => 300,
    Rubble(Rubble) => 500,
    Gate(Gate) => 600,
});

simple_enum_serbin!(LightIntensity {
    Strong => 0,
    Medium => 1,
    Dim => 2,
});

#[derive(Clone, SerBin, DeBin)]
pub struct LightSource {
    pub intensity: LightIntensity,
}

impl LightSource {
    pub fn new(intensity: LightIntensity) -> Self {
        Self { intensity }
    }
}

impl Default for LightSource {
    fn default() -> Self {
        Self::new(LightIntensity::Strong)
    }
}

#[derive(Clone, SerBin, DeBin, Default)]
pub struct Rubble {
    pub is_from_combat: bool,
}

impl Rubble {
    pub fn render(&self, position: IVec2, grid_pos: IVec2, level: &Level, assets: &AssetManager) {
        let world_pos = level.tile_to_world_pos(grid_pos, position);
        draw_texture(
            &assets.texture_interactable_rubble,
            world_pos.x,
            world_pos.y,
            WHITE,
        );
    }
}

#[derive(Clone, SerBin, DeBin)]
pub struct Gate {
    pub direction: Direction,
    #[nserde(proxy = "IVec2Proxy")]
    pub transfer_area_tile: IVec2,
    #[nserde(proxy = "IVec2Proxy")]
    pub transfer_area_size: IVec2,
}

impl Gate {
    pub fn is_in_transfer_area(&self, tile: IVec2) -> bool {
        tile.x >= self.transfer_area_tile.x
            && tile.y >= self.transfer_area_tile.y
            && tile.x < self.transfer_area_tile.x + self.transfer_area_size.x
            && tile.y < self.transfer_area_tile.y + self.transfer_area_size.y
    }

    fn rotation(&self) -> f32 {
        match self.direction {
            Direction::Up => 0.0,
            Direction::Right => PI / 2.0,
            Direction::Down => PI,
            Direction::Left => 3.0 * PI / 2.0,
        }
    }

    pub fn render(&self, position: IVec2, grid_pos: IVec2, level: &Level, assets: &AssetManager) {
        let world_pos = level.tile_to_world_pos(grid_pos, position);
        draw_texture_ex(
            &assets.texture_interactable_gate,
            world_pos.x,
            world_pos.y,
            WHITE,
            DrawTextureParams {
                rotation: self.rotation(),
                ..Default::default()
            },
        );
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            direction: Direction::Up,
            transfer_area_tile: IVec2::ZERO,
            transfer_area_size: IVec2::ONE,
        }
    }
}
