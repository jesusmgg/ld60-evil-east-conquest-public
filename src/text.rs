use macroquad::{
    color::Color,
    text::{camera_font_scale, draw_text_ex, Font, TextParams},
};

pub fn draw_scaled_text(text: &str, x: f32, y: f32, font_size: f32, color: &Color, font: &Font) {
    let (font_size, font_scale, font_aspect) = camera_font_scale(font_size);
    let text_params = TextParams {
        font: Some(font),
        font_size,
        font_scale,
        font_scale_aspect: font_aspect,
        color: *color,
        ..Default::default()
    };
    draw_text_ex(text, x, y, text_params);
}
