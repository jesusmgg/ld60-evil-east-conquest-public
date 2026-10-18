mod asset_loader;
mod asset_manager;
pub mod asset_path;
pub mod sprite_layout;

pub use asset_loader::{ui_loading_screen, LoadingProgress};
pub use asset_manager::{
    crewed_machine_texture_index, machine_texture_index, man_texture_index, AssetManager,
};
