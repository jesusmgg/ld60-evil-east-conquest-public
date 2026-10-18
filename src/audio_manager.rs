use macroquad::audio::{play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use macroquad::math::IVec2;
use macroquad::rand::gen_range;

use crate::args::parse_silent_flag;
use crate::assets::AssetManager;
use crate::constants::TURN_PERIOD;
use crate::history_manager::HistoryManager;
use crate::level::level_manager::LevelManager;
use crate::level::music_track::MusicTrack;
use crate::loading_manager::LoadingManager;
use crate::sfx::{SfxEvent, SFX_COUNT};
use crate::unit_manager::UnitManager;

const MUSIC_FADE_SPEED: f32 = 0.5;
const MUSIC_START_DELAY: f32 = 3.0;

pub const VOLUME_LEVEL_MAX: u8 = 10;

pub struct AudioManager {
    pub current_music: Option<MusicTrack>,
    pub music_volume: f32,
    pub music_target_volume: f32,
    music_start_delay: f32,
    pub is_silent: bool,
    pub is_music_muted: bool,
    pub is_sfx_muted: bool,
    pub master_level: u8,
    pub music_level: u8,
    pub sfx_level: u8,
    pub pending_sfx: Vec<SfxEvent>,
    pub spread_sfx: Vec<SpreadSfx>,
}

pub struct SpreadSfx {
    event: SfxEvent,
    delay: f32,
}

fn music_sound(asset_manager: &AssetManager, track: MusicTrack) -> Option<&Sound> {
    let Some(sound) = asset_manager.music.get(track.to_sequential_index()) else {
        return None;
    };
    sound.as_ref()
}

fn level_scale(level: u8) -> f32 {
    level as f32 / VOLUME_LEVEL_MAX as f32
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            current_music: None,
            music_volume: 0.0,
            music_target_volume: 0.0,
            music_start_delay: MUSIC_START_DELAY,
            is_silent: parse_silent_flag(),
            is_music_muted: false,
            is_sfx_muted: false,
            master_level: VOLUME_LEVEL_MAX,
            music_level: VOLUME_LEVEL_MAX,
            sfx_level: VOLUME_LEVEL_MAX,
            pending_sfx: Vec::new(),
            spread_sfx: Vec::new(),
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        asset_manager: &AssetManager,
        unit_manager: &UnitManager,
        level_manager: &LevelManager,
        loading_manager: &LoadingManager,
        history: &HistoryManager,
    ) {
        if loading_manager.is_initialized {
            self.update_music(dt, asset_manager, level_manager, unit_manager, history);
        }

        self.drain_sfx(dt, asset_manager);
    }

    fn update_music(
        &mut self,
        dt: f32,
        asset_manager: &AssetManager,
        level_manager: &LevelManager,
        unit_manager: &UnitManager,
        history: &HistoryManager,
    ) {
        let mut wanted_track = None;
        if let Some(level_pos) = level_manager.view_sector_pos {
            if let Some(level) = level_manager.get_level_at_pos(level_pos) {
                if unit_manager.has_live_enemy(level_pos)
                    && level.music_track != MusicTrack::NoMusic
                {
                    wanted_track = Some(level.music_track);
                }
            }
        }

        if unit_manager.transfer_arrival.is_some() || history.is_snapshot_loaded {
            let is_fading_out = self.music_volume > self.music_target_volume;
            if self.current_music.is_none() || wanted_track != self.current_music || is_fading_out {
                self.music_start_delay = MUSIC_START_DELAY;
            }
        }
        self.music_start_delay = (self.music_start_delay - dt).max(0.0);

        let is_playing_wanted = self.current_music.is_some()
            && wanted_track == self.current_music
            && self.music_start_delay == 0.0;
        self.music_target_volume = 0.0;
        if is_playing_wanted && !self.is_music_muted && !self.is_silent {
            self.music_target_volume =
                level_scale(self.music_level) * level_scale(self.master_level);
        }

        if self.music_volume < self.music_target_volume {
            self.music_volume =
                (self.music_volume + MUSIC_FADE_SPEED * dt).min(self.music_target_volume);
        } else if self.music_volume > self.music_target_volume {
            self.music_volume =
                (self.music_volume - MUSIC_FADE_SPEED * dt).max(self.music_target_volume);
        }

        if self.music_volume == 0.0 && (!is_playing_wanted || self.is_music_muted) {
            if let Some(track) = self.current_music {
                if let Some(music) = music_sound(asset_manager, track) {
                    stop_sound(music);
                }
                self.current_music = None;
            }
        }

        if self.current_music.is_none() && !self.is_music_muted && self.music_start_delay == 0.0 {
            if let Some(track) = wanted_track {
                if let Some(music) = music_sound(asset_manager, track) {
                    play_sound(
                        music,
                        PlaySoundParams {
                            looped: true,
                            volume: self.music_volume,
                        },
                    );
                    self.current_music = Some(track);
                }
            }
        }

        let Some(track) = self.current_music else {
            return;
        };
        if let Some(music) = music_sound(asset_manager, track) {
            set_sound_volume(music, self.music_volume);
        }
    }

    pub fn push_sfx_in_view(
        &mut self,
        level_manager: &LevelManager,
        level_pos: IVec2,
        event: SfxEvent,
    ) {
        if level_manager.view_sector_pos == Some(level_pos) {
            self.pending_sfx.push(event);
        }
    }

    fn drain_sfx(&mut self, dt: f32, asset_manager: &AssetManager) {
        let mut copies = [0u16; SFX_COUNT];
        for event_i in 0..self.pending_sfx.len() {
            copies[self.pending_sfx[event_i] as usize] += 1;
        }
        let mut copies_spread = [0u16; SFX_COUNT];
        for event_i in 0..self.pending_sfx.len() {
            let event = self.pending_sfx[event_i];
            let copy_i = copies_spread[event as usize];
            copies_spread[event as usize] += 1;
            self.spread_sfx.push(SpreadSfx {
                event,
                delay: TURN_PERIOD * copy_i as f32 / copies[event as usize] as f32,
            });
        }
        self.pending_sfx.clear();

        let mut spread_i = 0;
        while spread_i < self.spread_sfx.len() {
            let spread = &mut self.spread_sfx[spread_i];
            spread.delay -= dt;
            if spread.delay > 0.0 {
                spread_i += 1;
                continue;
            }
            let event = spread.event;
            self.spread_sfx.swap_remove(spread_i);
            let Some(sounds) = asset_manager.sfx.get(event as usize) else {
                continue;
            };
            let sound = &sounds[gen_range(0, sounds.len())];
            let mut volume = level_scale(self.sfx_level) * level_scale(self.master_level);
            if self.is_silent || self.is_sfx_muted {
                volume = 0.0;
            }
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume,
                },
            );
        }
    }
}
