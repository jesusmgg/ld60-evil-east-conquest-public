use crate::assets::asset_path::asset_path;
use crate::simple_enum_serbin;

simple_enum_serbin!(MusicTrack {
    NoMusic => 0,
    BattleMain => 200,
});

pub fn music_track_path(track: MusicTrack) -> Option<&'static str> {
    match track {
        MusicTrack::NoMusic => None,
        MusicTrack::BattleMain => Some(asset_path!("music/battle_main.ogg")),
    }
}
