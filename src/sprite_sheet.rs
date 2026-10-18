use macroquad::prelude::*;

pub fn sheet_frame_count(texture: &Texture2D) -> usize {
    (texture.width() / texture.height()) as usize
}

pub fn sheet_frame_rect(texture: &Texture2D, frames_shown: usize) -> Rect {
    let frame_size = texture.height();
    let frame = frames_shown % sheet_frame_count(texture);
    Rect::new(frame as f32 * frame_size, 0.0, frame_size, frame_size)
}
