mod ai_manager;
mod args;
mod assets;
mod audio_manager;
mod camera_manager;
mod combat_manager;
mod constants;
mod debug;
mod debug_control_manager;
mod debug_overlay_manager;
mod effect_manager;
mod enums;
mod flame;
mod game_data;
mod game_state;
mod gpu_path_manager;
mod history_manager;
mod level;
mod lighting_manager;
mod loading_manager;
mod minimap_manager;
mod player_manager;
mod selector_manager;
mod sfx;
mod sprite_sheet;
mod styles;
mod text;
mod ui_draw;
mod ui_manager;
mod unit_design;
mod unit_manager;

use assets::asset_path;
use constants::*;
use game_state::GameState;
use macroquad::experimental::coroutines::start_coroutine;
#[cfg(not(target_arch = "wasm32"))]
use macroquad::input::{is_quit_requested, prevent_quit};
use macroquad::miniquad::conf::{Platform, WebGLVersion};
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: WINDOW_TITLE.to_owned(),
        platform: Platform {
            webgl_version: WebGLVersion::WebGL2,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    configure();

    let font_loader =
        start_coroutine(async move { load_ttf_font(asset_path::FONT_KENNEY_PIXEL).await.unwrap() });

    while !font_loader.is_done() {
        clear_background(BLACK);
        next_frame().await;
    }

    let font = font_loader.retrieve().unwrap();

    let mut game_state = GameState::new(font).await;

    game_state.init();

    #[cfg(not(target_arch = "wasm32"))]
    prevent_quit();

    loop {
        #[cfg(not(target_arch = "wasm32"))]
        if is_quit_requested() {
            game_state.shutdown();
            break;
        }

        let dt = get_frame_time();

        game_state.input();

        game_state.update(dt);

        set_camera(&game_state.camera_manager.camera);
        clear_background(game_state.styles.colors.black);
        game_state.render();

        set_default_camera();
        game_state.ui();

        next_frame().await
    }
}

fn configure() {
    set_default_filter_mode(FilterMode::Nearest);

    #[cfg(not(target_arch = "wasm32"))]
    macroquad::miniquad::window::set_window_size(1280, 720);
}
