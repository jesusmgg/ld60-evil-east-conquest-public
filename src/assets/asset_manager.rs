use super::asset_loader::{load_audio_parallel, load_images_parallel, LoadingProgress};
use super::asset_path;
use super::sprite_layout::{is_far_crew, MACHINE_CREW_SLOTS};
use crate::args::parse_no_audio_flag;
use crate::level::music_track::{music_track_path, MusicTrack};
use crate::sfx::{NO_PITCH_SHIFT, SFX_PATHS, SFX_RATES};
use crate::styles::Styles;
use macroquad::prelude::*;
use std::collections::HashMap;

pub struct AssetManager {
    pub texture_background_default: Texture2D,

    pub texture_tile_dirt: Texture2D,
    pub texture_tile_grass: Texture2D,
    pub texture_tile_mountain: Texture2D,
    pub texture_tile_stone: Texture2D,
    pub texture_tile_tree_1: Texture2D,
    pub texture_tile_tree_2: Texture2D,
    pub texture_tile_tree_3: Texture2D,
    pub texture_tile_water: Texture2D,

    pub texture_interactable_gate: Texture2D,
    pub texture_interactable_rubble: Texture2D,

    pub texture_structures: Vec<Texture2D>,
    pub texture_resource_icons: Vec<Texture2D>,

    pub texture_men: Vec<Texture2D>,
    pub texture_machines: Vec<Texture2D>,
    pub texture_crewed_machines: Vec<Texture2D>,

    pub spritesheet_flame_bright: Texture2D,
    pub spritesheet_flame_dim: Texture2D,

    pub spritesheet_projectiles: Vec<Texture2D>,
    pub spritesheet_effects: Vec<Texture2D>,

    pub spritesheet_light_strong: Texture2D,
    pub spritesheet_light_medium: Texture2D,
    pub spritesheet_light_dim: Texture2D,
    pub spritesheet_light_ambient: Texture2D,

    pub font: Font,

    pub music: Vec<Option<macroquad::audio::Sound>>,

    pub sfx: Vec<Vec<macroquad::audio::Sound>>,
}

fn take_texture(images: &mut HashMap<String, Image>, path: &str) -> Texture2D {
    Texture2D::from_image(&images.remove(path).unwrap())
}

pub fn man_texture_index(armor_slot: usize, weapon_slot: usize, color_slot: usize) -> usize {
    (armor_slot * asset_path::MAN_WEAPON_LAYERS.len() + weapon_slot)
        * asset_path::MAN_COLOR_LAYERS.len()
        + color_slot
}

pub fn machine_texture_index(
    chassis_slot: usize,
    shielding_slot: usize,
    weapon_slot: usize,
) -> usize {
    (chassis_slot * asset_path::MACHINE_SHIELDING_LAYERS.len() + shielding_slot)
        * asset_path::MACHINE_WEAPON_LAYERS.len()
        + weapon_slot
}

pub fn crewed_machine_texture_index(machine_index: usize, color_slot: usize) -> usize {
    machine_index * asset_path::MAN_COLOR_LAYERS.len() + color_slot
}

fn overlay_layer(image: &mut Image, layer: &Image, x: i32, y: i32) {
    for layer_y in 0..layer.height as i32 {
        for layer_x in 0..layer.width as i32 {
            let image_x = x + layer_x;
            let image_y = y + layer_y;
            if image_x < 0
                || image_y < 0
                || image_x >= image.width as i32
                || image_y >= image.height as i32
            {
                continue;
            }
            let source = ((layer_y * layer.width as i32 + layer_x) * 4) as usize;
            if layer.bytes[source + 3] == 0 {
                continue;
            }
            let target = ((image_y * image.width as i32 + image_x) * 4) as usize;
            image.bytes[target..target + 4].copy_from_slice(&layer.bytes[source..source + 4]);
        }
    }
}

fn overlay_crew(image: &mut Image, man: &Image, is_far: bool) {
    for k in 0..MACHINE_CREW_SLOTS.len() as u16 {
        if is_far_crew(k) == is_far {
            let slot = MACHINE_CREW_SLOTS[k as usize];
            overlay_layer(image, man, slot.x as i32, slot.y as i32);
        }
    }
}

fn compose_man_textures(images: &HashMap<String, Image>) -> (Vec<Texture2D>, Vec<Image>) {
    let base = &images[asset_path::MAN_BASE];
    let mut textures = Vec::with_capacity(
        asset_path::MAN_ARMOR_LAYERS.len()
            * asset_path::MAN_WEAPON_LAYERS.len()
            * asset_path::MAN_COLOR_LAYERS.len(),
    );
    let mut crew = Vec::with_capacity(asset_path::MAN_COLOR_LAYERS.len());
    let mut image = base.clone();
    for armor_layer in asset_path::MAN_ARMOR_LAYERS {
        for weapon_layer in asset_path::MAN_WEAPON_LAYERS {
            for color_layer in asset_path::MAN_COLOR_LAYERS {
                image.bytes.copy_from_slice(&base.bytes);
                if let Some(path) = armor_layer {
                    overlay_layer(&mut image, &images[path], 0, 0);
                }
                if let Some(path) = weapon_layer {
                    overlay_layer(&mut image, &images[path], 0, 0);
                }
                overlay_layer(&mut image, &images[color_layer], 0, 0);
                textures.push(Texture2D::from_image(&image));
                if armor_layer.is_none() && weapon_layer.is_none() {
                    crew.push(image.clone());
                }
            }
        }
    }
    (textures, crew)
}

fn compose_machine_textures(
    images: &HashMap<String, Image>,
    crew: &[Image],
) -> (Vec<Texture2D>, Vec<Texture2D>) {
    let machine_count = asset_path::MACHINE_CHASSIS_LAYERS.len()
        * asset_path::MACHINE_SHIELDING_LAYERS.len()
        * asset_path::MACHINE_WEAPON_LAYERS.len();
    let mut textures = Vec::with_capacity(machine_count);
    let mut crewed_textures =
        Vec::with_capacity(machine_count * asset_path::MAN_COLOR_LAYERS.len());

    let mut image = images[asset_path::MACHINE_CHASSIS_LAYERS[0]].clone();
    let mut crewed = image.clone();
    for chassis_layer in asset_path::MACHINE_CHASSIS_LAYERS {
        for shielding_layer in asset_path::MACHINE_SHIELDING_LAYERS {
            for weapon_layer in asset_path::MACHINE_WEAPON_LAYERS {
                image.bytes.copy_from_slice(&images[chassis_layer].bytes);
                if let Some(path) = shielding_layer {
                    overlay_layer(&mut image, &images[path], 0, 0);
                }
                if let Some(path) = weapon_layer {
                    overlay_layer(&mut image, &images[path], 0, 0);
                }
                textures.push(Texture2D::from_image(&image));
                for man in crew {
                    crewed.bytes.fill(0);
                    overlay_crew(&mut crewed, man, true);
                    overlay_layer(&mut crewed, &image, 0, 0);
                    overlay_crew(&mut crewed, man, false);
                    crewed_textures.push(Texture2D::from_image(&crewed));
                }
            }
        }
    }
    (textures, crewed_textures)
}

impl AssetManager {
    pub async fn new(font: Font, styles: &Styles) -> Self {
        let mut loading_progress = LoadingProgress {
            progress: 0.0,
            text: "Loading assets...".to_string(),
        };

        let image_path_strs = [
            asset_path::BACKGROUND_DEFAULT,
            asset_path::TILE_DIRT,
            asset_path::TILE_GRASS,
            asset_path::TILE_MOUNTAIN,
            asset_path::TILE_STONE,
            asset_path::TILE_TREE_1,
            asset_path::TILE_TREE_2,
            asset_path::TILE_TREE_3,
            asset_path::TILE_WATER,
            asset_path::INTERACTABLE_GATE,
            asset_path::INTERACTABLE_RUBBLE,
            asset_path::MAN_BASE,
            asset_path::SPRITESHEET_FLAME_BRIGHT,
            asset_path::SPRITESHEET_FLAME_DIM,
            asset_path::SPRITESHEET_LIGHT_STRONG,
            asset_path::SPRITESHEET_LIGHT_MEDIUM,
            asset_path::SPRITESHEET_LIGHT_DIM,
            asset_path::SPRITESHEET_LIGHT_AMBIENT,
        ];
        let mut image_paths = Vec::with_capacity(image_path_strs.len());
        for s in image_path_strs {
            image_paths.push(s.to_string());
        }
        for layer in asset_path::MAN_ARMOR_LAYERS {
            if let Some(path) = layer {
                image_paths.push(path.to_string());
            }
        }
        for layer in asset_path::MAN_WEAPON_LAYERS {
            if let Some(path) = layer {
                image_paths.push(path.to_string());
            }
        }
        for path in asset_path::MAN_COLOR_LAYERS {
            image_paths.push(path.to_string());
        }
        for path in asset_path::MACHINE_CHASSIS_LAYERS {
            image_paths.push(path.to_string());
        }
        for layer in asset_path::MACHINE_SHIELDING_LAYERS {
            if let Some(path) = layer {
                image_paths.push(path.to_string());
            }
        }
        for layer in asset_path::MACHINE_WEAPON_LAYERS {
            if let Some(path) = layer {
                image_paths.push(path.to_string());
            }
        }
        for path in asset_path::STRUCTURE_SPRITES {
            image_paths.push(path.to_string());
        }
        for path in asset_path::RESOURCE_ICONS {
            image_paths.push(path.to_string());
        }
        for path in asset_path::PROJECTILE_SPRITES {
            image_paths.push(path.to_string());
        }
        for path in asset_path::EFFECT_SPRITES {
            image_paths.push(path.to_string());
        }

        let mut images =
            load_images_parallel(image_paths, &mut loading_progress, styles, &font).await;

        let (texture_men, crew) = compose_man_textures(&images);
        let (texture_machines, texture_crewed_machines) = compose_machine_textures(&images, &crew);
        let texture_background_default = take_texture(&mut images, asset_path::BACKGROUND_DEFAULT);
        let texture_tile_dirt = take_texture(&mut images, asset_path::TILE_DIRT);
        let texture_tile_grass = take_texture(&mut images, asset_path::TILE_GRASS);
        let texture_tile_mountain = take_texture(&mut images, asset_path::TILE_MOUNTAIN);
        let texture_tile_stone = take_texture(&mut images, asset_path::TILE_STONE);
        let texture_tile_tree_1 = take_texture(&mut images, asset_path::TILE_TREE_1);
        let texture_tile_tree_2 = take_texture(&mut images, asset_path::TILE_TREE_2);
        let texture_tile_tree_3 = take_texture(&mut images, asset_path::TILE_TREE_3);
        let texture_tile_water = take_texture(&mut images, asset_path::TILE_WATER);
        let texture_interactable_gate = take_texture(&mut images, asset_path::INTERACTABLE_GATE);
        let texture_interactable_rubble =
            take_texture(&mut images, asset_path::INTERACTABLE_RUBBLE);
        let mut texture_structures = Vec::with_capacity(asset_path::STRUCTURE_SPRITES.len());
        for path in asset_path::STRUCTURE_SPRITES {
            texture_structures.push(take_texture(&mut images, path));
        }
        let mut texture_resource_icons = Vec::with_capacity(asset_path::RESOURCE_ICONS.len());
        for path in asset_path::RESOURCE_ICONS {
            texture_resource_icons.push(take_texture(&mut images, path));
        }
        let spritesheet_flame_bright =
            take_texture(&mut images, asset_path::SPRITESHEET_FLAME_BRIGHT);
        let spritesheet_flame_dim = take_texture(&mut images, asset_path::SPRITESHEET_FLAME_DIM);
        let mut spritesheet_projectiles = Vec::with_capacity(asset_path::PROJECTILE_SPRITES.len());
        for path in asset_path::PROJECTILE_SPRITES {
            spritesheet_projectiles.push(take_texture(&mut images, path));
        }
        let mut spritesheet_effects = Vec::with_capacity(asset_path::EFFECT_SPRITES.len());
        for path in asset_path::EFFECT_SPRITES {
            spritesheet_effects.push(take_texture(&mut images, path));
        }
        let spritesheet_light_strong =
            take_texture(&mut images, asset_path::SPRITESHEET_LIGHT_STRONG);
        let spritesheet_light_medium =
            take_texture(&mut images, asset_path::SPRITESHEET_LIGHT_MEDIUM);
        let spritesheet_light_dim = take_texture(&mut images, asset_path::SPRITESHEET_LIGHT_DIM);
        let spritesheet_light_ambient =
            take_texture(&mut images, asset_path::SPRITESHEET_LIGHT_AMBIENT);

        let mut music = Vec::new();
        let mut sfx = Vec::new();
        if !parse_no_audio_flag() {
            let mut requests = Vec::new();
            for track in MusicTrack::all_variants() {
                if let Some(path) = music_track_path(*track) {
                    requests.push((path, NO_PITCH_SHIFT));
                }
            }
            for sfx_i in 0..SFX_PATHS.len() {
                requests.push((SFX_PATHS[sfx_i], SFX_RATES[sfx_i]));
            }

            let mut sounds =
                load_audio_parallel(requests, &mut loading_progress, styles, &font).await;

            for track in MusicTrack::all_variants() {
                let mut sound = None;
                if let Some(path) = music_track_path(*track) {
                    sound = Some(sounds.remove(path).unwrap().remove(0));
                }
                music.push(sound);
            }
            for path in SFX_PATHS {
                sfx.push(sounds.remove(*path).unwrap());
            }
        }

        Self {
            texture_background_default,
            texture_tile_dirt,
            texture_tile_grass,
            texture_tile_mountain,
            texture_tile_stone,
            texture_tile_tree_1,
            texture_tile_tree_2,
            texture_tile_tree_3,
            texture_tile_water,
            texture_interactable_gate,
            texture_interactable_rubble,
            texture_structures,
            texture_resource_icons,
            texture_men,
            texture_machines,
            texture_crewed_machines,
            spritesheet_flame_bright,
            spritesheet_flame_dim,
            spritesheet_projectiles,
            spritesheet_effects,
            spritesheet_light_strong,
            spritesheet_light_medium,
            spritesheet_light_dim,
            spritesheet_light_ambient,
            font,
            music,
            sfx,
        }
    }
}
