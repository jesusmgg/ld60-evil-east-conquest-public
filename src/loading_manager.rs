use crate::constants::*;
use crate::debug::DebugLog;
use crate::debug_control_manager::DebugControlManager;
use crate::history_manager::HistoryManager;
use crate::level::level_manager::LevelManager;
use crate::player_manager::PlayerManager;
use crate::selector_manager::SelectorManager;
use crate::ui_manager::UiManager;
use crate::unit_manager::UnitManager;

pub struct LoadingManager {
    pub is_initialized: bool,
    pub initialization_message: &'static str,
    pub initialization_progress: f32,
}

impl LoadingManager {
    pub fn new() -> Self {
        Self {
            is_initialized: false,
            initialization_message: "",
            initialization_progress: 0.0,
        }
    }

    pub fn update(
        &mut self,
        level_manager: &mut LevelManager,
        unit_manager: &mut UnitManager,
        history_manager: &mut HistoryManager,
        selector_manager: &mut SelectorManager,
        player_manager: &mut PlayerManager,
        debug_control_manager: &mut DebugControlManager,
        ui_manager: &mut UiManager,
        debug_log: &mut DebugLog,
    ) {
        if self.is_initialized {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        history_manager.check_web_boot(debug_log);

        if !level_manager.is_initialized {
            self.initialization_message = "Loading levels...";
            self.initialization_progress = 0.25;
        } else if !unit_manager.is_initialized {
            self.initialization_message = "Initializing units...";
            self.initialization_progress = 0.75;
        }

        #[cfg(target_arch = "wasm32")]
        if level_manager.is_initialized
            && unit_manager.is_initialized
            && !history_manager.is_boot_ready()
        {
            self.initialization_message = "Restoring save...";
            self.initialization_progress = 0.9;
        }

        if level_manager.is_initialized
            && unit_manager.is_initialized
            && history_manager.is_boot_ready()
        {
            self.is_initialized = true;

            history_manager.finish_initialization(level_manager, unit_manager, debug_log);
            if history_manager.is_save_cleared {
                ui_manager.message = Some(String::from(SAVE_CLEARED_MESSAGE));
            }

            selector_manager.select_first_player(unit_manager);

            player_manager.load(unit_manager, level_manager, history_manager);

            debug_control_manager.write_hello(
                unit_manager.turn_count,
                TURN_PERIOD,
                unit_manager,
                level_manager,
            );
        }
    }
}
