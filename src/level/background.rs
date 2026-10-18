use crate::assets::AssetManager;
use crate::simple_enum_serbin;
use macroquad::prelude::*;

simple_enum_serbin!(Background {
    Default => 0,
});

pub fn get_texture_for_background(assets: &AssetManager, background: Background) -> &Texture2D {
    match background {
        Background::Default => &assets.texture_background_default,
    }
}
