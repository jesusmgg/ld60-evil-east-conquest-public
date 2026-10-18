use crate::text::draw_scaled_text;
use macroquad::prelude::*;
use macroquad::text::Font;

const MAX_LOG_ENTRIES: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Log,
    Warning,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

pub struct DebugLog {
    entries: Vec<LogEntry>,

    pub pending_mirror: Vec<(LogLevel, String)>,
}

impl DebugLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            pending_mirror: Vec::new(),
        }
    }

    pub fn log(&mut self, message: String) {
        println!("[LOG] {}", message);
        self.add_entry(LogLevel::Log, message);
    }

    pub fn warning(&mut self, message: String) {
        println!("[WARNING] {}", message);
        self.add_entry(LogLevel::Warning, message);
    }

    fn add_entry(&mut self, level: LogLevel, message: String) {
        self.pending_mirror.push((level, message.clone()));
        self.entries.push(LogEntry { level, message });

        if self.entries.len() > MAX_LOG_ENTRIES {
            self.entries.remove(0);
        }
    }

    pub fn ui(&self, visible: bool, font: &Font, zoom: f32, x_offset: f32, y_offset: f32) {
        if !visible || self.entries.is_empty() {
            return;
        }

        let x = 4.0;
        let mut y = 4.0;
        let line_height = 8.0;
        let font_size = 8.0;
        let padding = 4.0;

        let mut max_width = 0.0;
        for entry in &self.entries {
            let text_dimensions = measure_text(&entry.message, Some(font), font_size as u16, 1.0);
            if text_dimensions.width > max_width {
                max_width = text_dimensions.width;
            }
        }

        let bg_x = x - padding;
        let bg_y = y - padding;
        let bg_width = max_width + padding * 2.0;
        let bg_height = ((self.entries.len() - 1) as f32 * line_height) + padding * 2.0;

        let screen_bg_x = x_offset + (bg_x * zoom);
        let screen_bg_y = y_offset + (bg_y * zoom);
        let screen_bg_width = bg_width * zoom;
        let screen_bg_height = bg_height * zoom;

        let bg_color = Color::from_rgba(0, 0, 0, 128);
        draw_rectangle(
            screen_bg_x,
            screen_bg_y,
            screen_bg_width,
            screen_bg_height,
            bg_color,
        );

        for entry in &self.entries {
            let mut color = match entry.level {
                LogLevel::Log => WHITE,
                LogLevel::Warning => Color::from_rgba(255, 220, 0, 255),
            };
            color.a = 1.0;

            let screen_x = x_offset + (x * zoom);
            let screen_y = y_offset + (y * zoom);

            draw_scaled_text(
                &entry.message,
                screen_x,
                screen_y,
                font_size * zoom,
                &color,
                font,
            );
            y += line_height;
        }
    }
}

pub struct DebugLogPtr(*mut DebugLog);
unsafe impl Send for DebugLogPtr {}

impl DebugLogPtr {
    pub fn new(debug_log: &mut DebugLog) -> Self {
        Self(debug_log as *mut DebugLog)
    }

    pub unsafe fn as_mut(&self) -> &mut DebugLog {
        &mut *self.0
    }
}
