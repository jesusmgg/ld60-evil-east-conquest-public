use macroquad::{math::f32, text::Font};

use crate::{
    ai_manager::AiManager,
    assets::AssetManager,
    audio_manager::AudioManager,
    camera_manager::CameraManager,
    combat_manager::CombatManager,
    debug_control_manager::DebugControlManager,
    debug_overlay_manager::DebugOverlayManager,
    effect_manager::EffectManager,
    gpu_path_manager::GpuPathManager,
    history_manager::HistoryManager,
    level::{level_editor_manager::LevelEditorManager, level_manager::LevelManager},
    lighting_manager::LightingManager,
    loading_manager::LoadingManager,
    minimap_manager::MinimapManager,
    player_manager::PlayerManager,
    selector_manager::SelectorManager,
    styles::Styles,
    ui_manager::UiManager,
    unit_manager::UnitManager,
};

pub struct GameState {
    pub styles: Styles,

    pub camera_manager: CameraManager,
    pub audio_manager: AudioManager,
    pub ui_manager: UiManager,
    pub debug_overlay_manager: DebugOverlayManager,

    pub asset_manager: AssetManager,

    pub level_manager: LevelManager,
    pub level_editor_manager: LevelEditorManager,
    pub unit_manager: UnitManager,
    pub lighting_manager: LightingManager,

    pub selector_manager: SelectorManager,

    pub minimap_manager: MinimapManager,

    pub ai_manager: AiManager,

    pub combat_manager: CombatManager,

    pub effect_manager: EffectManager,

    pub player_manager: PlayerManager,

    pub gpu_path_manager: GpuPathManager,

    pub debug_control_manager: DebugControlManager,

    pub loading_manager: LoadingManager,

    pub history_manager: HistoryManager,
}

impl GameState {
    pub async fn new(font: Font) -> Self {
        let styles = Styles::new();

        let camera_manager = CameraManager::new();
        let audio_manager = AudioManager::new();
        let ui_manager = UiManager::new();
        let debug_overlay_manager = DebugOverlayManager::new();

        let asset_manager = AssetManager::new(font, &styles).await;

        let level_manager = LevelManager::new();
        let level_editor_manager = LevelEditorManager::new();
        let unit_manager = UnitManager::new();
        let lighting_manager = LightingManager::new();
        let loading_manager = LoadingManager::new();

        let selector_manager = SelectorManager::new();
        let minimap_manager = MinimapManager::new();
        let ai_manager = AiManager::new();
        let combat_manager = CombatManager::new();
        let effect_manager = EffectManager::new();
        let player_manager = PlayerManager::new();
        let gpu_path_manager = GpuPathManager::new();
        let debug_control_manager = DebugControlManager::new();

        Self {
            styles,

            camera_manager,
            audio_manager,
            ui_manager,
            debug_overlay_manager,

            asset_manager,
            level_manager,
            level_editor_manager,
            unit_manager,
            lighting_manager,
            loading_manager,
            selector_manager,
            minimap_manager,
            ai_manager,
            combat_manager,
            effect_manager,
            player_manager,
            gpu_path_manager,
            debug_control_manager,

            history_manager: HistoryManager::new_disabled(HistoryManager::parse_no_history_flag()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn shutdown(&mut self) {
        self.history_manager
            .shutdown(&self.unit_manager, &self.level_manager);
    }

    pub fn init(&mut self) {
        self.level_manager
            .init(&mut self.debug_overlay_manager.debug_log);
        self.unit_manager
            .init(&mut self.debug_overlay_manager.debug_log);
    }

    pub fn input(&mut self) {
        self.camera_manager.input();
        self.player_manager.input(
            &self.camera_manager,
            &self.level_manager,
            &self.unit_manager,
            &self.selector_manager,
            &self.level_editor_manager,
        );
        self.ui_manager.input();
        self.history_manager.input();
        #[cfg(debug_assertions)]
        self.debug_overlay_manager
            .input(&self.level_editor_manager, &self.player_manager);

        self.debug_control_manager.input(&self.unit_manager);

        self.minimap_manager.input(
            &self.level_manager,
            &self.level_editor_manager,
            &self.camera_manager,
            &self.player_manager,
        );

        self.selector_manager.input(
            &self.camera_manager,
            &self.unit_manager,
            &self.level_manager,
            &self.level_editor_manager,
            &self.minimap_manager,
            &self.player_manager,
        );

        self.unit_manager.input(
            &self.selector_manager,
            &self.level_editor_manager,
            &self.level_manager,
            &self.minimap_manager,
            &self.camera_manager,
            &self.player_manager,
        );

        #[cfg(debug_assertions)]
        self.level_editor_manager
            .input(&self.minimap_manager, &self.camera_manager);
    }

    pub fn update(&mut self, dt: f32) {
        self.camera_manager.update(dt, &self.loading_manager);
        self.ui_manager.update();
        self.debug_overlay_manager.update(dt);

        self.debug_control_manager.apply_commands(
            &mut self.unit_manager,
            &mut self.selector_manager,
            &mut self.level_manager,
            &mut self.player_manager,
            &mut self.history_manager,
            &mut self.gpu_path_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.level_manager
            .update(&mut self.debug_overlay_manager.debug_log);

        self.history_manager.pre_update(
            &mut self.level_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.minimap_manager.update(&mut self.level_manager);

        self.history_manager.update_snapshots(
            &mut self.unit_manager,
            &mut self.level_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.unit_manager.update_clock(
            dt,
            &self.loading_manager,
            &mut self.debug_control_manager,
            &self.level_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.ai_manager.update(&mut self.unit_manager);

        self.unit_manager.update(
            &mut self.level_manager,
            &mut self.selector_manager,
            &mut self.gpu_path_manager,
            &mut self.history_manager,
            &mut self.audio_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.combat_manager.update(
            &mut self.unit_manager,
            &mut self.level_manager,
            &mut self.history_manager,
            &mut self.audio_manager,
        );

        self.unit_manager.end_turn(&mut self.history_manager);

        self.player_manager.update(
            dt,
            &self.loading_manager,
            &mut self.unit_manager,
            &mut self.level_manager,
            &self.selector_manager,
            &self.level_editor_manager,
            &mut self.audio_manager,
            &mut self.history_manager,
        );

        self.history_manager.post_update(
            &self.unit_manager,
            &self.level_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.debug_control_manager.tick_rewinds(
            &self.loading_manager,
            &mut self.unit_manager,
            &mut self.selector_manager,
            &mut self.level_manager,
            &mut self.history_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.lighting_manager.update(dt);
        self.effect_manager.update(
            dt,
            &mut self.unit_manager,
            &self.level_manager,
            &self.asset_manager,
            &mut self.audio_manager,
            &mut self.history_manager,
        );

        self.selector_manager.update(
            &self.unit_manager,
            &self.level_manager,
            &self.level_editor_manager,
            dt,
        );

        self.debug_control_manager.post_turn(
            &self.unit_manager,
            &self.selector_manager,
            &self.level_manager,
            &self.history_manager,
            &mut self.debug_overlay_manager.debug_log,
        );
        self.debug_control_manager.write_logs(
            &self.unit_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        #[cfg(debug_assertions)]
        self.level_editor_manager.update(
            dt,
            &mut self.level_manager,
            &mut self.history_manager,
            &mut self.debug_overlay_manager.debug_log,
            &self.camera_manager,
        );
        #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
        self.history_manager.persist_sector_growth(
            &self.level_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        self.loading_manager.update(
            &mut self.level_manager,
            &mut self.unit_manager,
            &mut self.history_manager,
            &mut self.selector_manager,
            &mut self.player_manager,
            &mut self.debug_control_manager,
            &mut self.ui_manager,
            &mut self.debug_overlay_manager.debug_log,
        );
        self.unit_manager
            .update_transfer_tooltip(&self.level_manager);

        self.audio_manager.update(
            dt,
            &self.asset_manager,
            &self.unit_manager,
            &self.level_manager,
            &self.loading_manager,
            &self.history_manager,
        );
    }

    pub fn render(&mut self) {
        if !self.loading_manager.is_initialized {
            return;
        }

        #[cfg(debug_assertions)]
        self.debug_overlay_manager
            .render_world(&self.styles, &self.level_manager);

        self.level_manager
            .render(&self.asset_manager, &mut self.camera_manager);
        self.unit_manager.render(
            &self.asset_manager,
            &self.level_manager,
            &self.camera_manager,
        );
        self.effect_manager
            .render(&self.asset_manager, &self.camera_manager);
        self.unit_manager.render_overlays(
            &self.asset_manager,
            &self.styles,
            &self.level_manager,
            &self.selector_manager,
            &self.debug_overlay_manager,
            &self.camera_manager,
        );
        self.player_manager
            .render(&self.level_manager, &self.asset_manager, &self.styles);

        self.lighting_manager.render(
            &self.level_manager,
            &self.unit_manager,
            &self.asset_manager,
            &self.camera_manager,
            &self.debug_overlay_manager,
        );

        self.selector_manager
            .render_interactable_highlight(&self.level_manager, &self.styles);
        self.selector_manager.render(&self.styles);

        #[cfg(debug_assertions)]
        self.level_editor_manager
            .render(&self.styles, &self.level_manager, &self.asset_manager);
    }

    pub fn ui(&mut self) {
        if !self.loading_manager.is_initialized {
            self.ui_manager
                .ui_loading(&self.loading_manager, &self.styles, &self.asset_manager);
            return;
        }

        #[cfg(debug_assertions)]
        self.debug_overlay_manager.ui_tile_indices(
            &self.asset_manager,
            &self.camera_manager,
            &self.level_manager,
        );

        self.minimap_manager.ui(
            &self.level_manager,
            &self.unit_manager,
            &self.player_manager,
            &self.level_editor_manager,
            &self.styles,
            &self.camera_manager,
        );

        self.unit_manager
            .ui(&self.styles, &self.asset_manager, &self.camera_manager);

        self.player_manager.ui(
            &self.unit_manager,
            &self.level_manager,
            &self.selector_manager,
            &self.history_manager,
            &self.audio_manager,
            &self.level_editor_manager,
            &self.styles,
            &self.asset_manager,
            &self.camera_manager,
        );

        #[cfg(debug_assertions)]
        self.level_editor_manager.ui_palette(
            &self.asset_manager,
            &self.styles,
            &self.camera_manager,
        );

        #[cfg(debug_assertions)]
        self.level_editor_manager.ui(
            &mut self.level_manager,
            &mut self.history_manager,
            &mut self.debug_overlay_manager.debug_log,
        );

        #[cfg(debug_assertions)]
        self.debug_overlay_manager
            .ui_debug_log(&self.asset_manager, &self.camera_manager);

        self.ui_manager
            .ui(&self.styles, &self.asset_manager, &self.camera_manager);

        self.debug_overlay_manager.ui_diagnostics(
            &self.styles,
            &self.asset_manager,
            &self.camera_manager,
            &self.level_manager,
            &self.unit_manager,
            &self.level_editor_manager,
        );
    }
}
