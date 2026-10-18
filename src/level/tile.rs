use crate::assets::AssetManager;
use crate::constants::{TILE_SIZE_X, TILE_SIZE_Y};
use crate::simple_enum_serbin;
use macroquad::prelude::*;

simple_enum_serbin!(TileType, ALL_TILES {
    Water    => 420,
    Mountain => 450,
    Dirt     => 460,
    Grass    => 470,
    Stone    => 480,
    Tree1    => 490,
    Tree2    => 500,
    Tree3    => 510,
});

pub fn get_texture_for_tile(assets: &AssetManager, tile_type: TileType) -> &Texture2D {
    match tile_type {
        TileType::Water => &assets.texture_tile_water,
        TileType::Mountain => &assets.texture_tile_mountain,
        TileType::Dirt => &assets.texture_tile_dirt,
        TileType::Grass => &assets.texture_tile_grass,
        TileType::Stone => &assets.texture_tile_stone,
        TileType::Tree1 => &assets.texture_tile_tree_1,
        TileType::Tree2 => &assets.texture_tile_tree_2,
        TileType::Tree3 => &assets.texture_tile_tree_3,
    }
}

pub fn render_tile(assets: &AssetManager, tile_type: TileType, world_pos: Vec2, color: Color) {
    let texture = get_texture_for_tile(assets, tile_type);
    draw_texture(texture, world_pos.x, world_pos.y, color);
}

pub fn tile_center(world_pos: Vec2) -> Vec2 {
    world_pos + Vec2::new(TILE_SIZE_X / 2.0, TILE_SIZE_Y / 2.0)
}

#[inline(always)]
pub fn is_tile_obstacle(tile_type: TileType) -> bool {
    matches!(
        tile_type,
        TileType::Water | TileType::Mountain | TileType::Tree1 | TileType::Tree2 | TileType::Tree3
    )
}
