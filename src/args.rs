pub fn parse_no_audio_flag() -> bool {
    if cfg!(target_arch = "wasm32") {
        return false;
    }
    for arg in std::env::args() {
        if arg == "--no-audio" {
            return true;
        }
    }
    false
}

pub fn parse_silent_flag() -> bool {
    if cfg!(target_arch = "wasm32") {
        return false;
    }
    for arg in std::env::args() {
        if arg == "--no-sound" || arg == "--control" || arg.starts_with("--control=") {
            return true;
        }
    }
    false
}
