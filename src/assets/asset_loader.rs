use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Cursor, Read};
#[cfg(not(target_arch = "wasm32"))]
use std::mem;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{channel, Receiver};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use macroquad::{prelude::*, window::next_frame};

#[cfg(target_arch = "wasm32")]
use macroquad::{experimental::coroutines::start_coroutine, file::load_file, texture::load_image};

use macroquad::audio::load_sound_from_bytes;

#[cfg(not(target_arch = "wasm32"))]
use image::GenericImageView;
#[cfg(not(target_arch = "wasm32"))]
use lewton::inside_ogg::OggStreamReader;

use crate::{camera_manager::view_layout, constants::*, styles::Styles, text::draw_scaled_text};

fn deduplicate_paths(mut paths: Vec<String>) -> Vec<String> {
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(not(target_arch = "wasm32"))]
fn get_max_threads() -> usize {
    match thread::available_parallelism() {
        Ok(count) => (count.get() / 2).max(1),
        Err(_) => 2,
    }
}

pub struct LoadingProgress {
    pub progress: f32,
    pub text: String,
}

pub fn ui_loading_screen(
    progress: &LoadingProgress,
    styles: &Styles,
    font: &macroquad::text::Font,
) {
    set_default_camera();
    clear_background(styles.colors.black);

    let (zoom, viewport) = view_layout();
    let x_offset = viewport.0 as f32;
    let y_offset = viewport.1 as f32;

    let font_size = 16.0;
    let box_width = 148.0;
    let box_height = 32.0;
    let box_x = (VIEW_W - box_width) / 2.0;
    let box_y = (VIEW_H - box_height) / 2.0;

    let screen_box_x = x_offset + (box_x * zoom);
    let screen_box_y = y_offset + (box_y * zoom);

    draw_rectangle(
        screen_box_x - 2.0 * zoom,
        screen_box_y - 2.0 * zoom,
        (box_width + 4.0) * zoom,
        (box_height + 4.0) * zoom,
        styles.colors.cyan_med,
    );

    draw_rectangle(
        screen_box_x,
        screen_box_y,
        box_width * zoom,
        box_height * zoom,
        styles.colors.blue_shadow,
    );

    let text_dims = measure_text(&progress.text, Some(font), font_size as u16, 1.0);
    let text_x = box_x + (box_width - text_dims.width) / 2.0;
    let text_y = box_y + 6.0 + text_dims.offset_y;
    let screen_text_x = x_offset + (text_x * zoom);
    let screen_text_y = y_offset + (text_y * zoom);

    draw_scaled_text(
        &progress.text,
        screen_text_x,
        screen_text_y,
        font_size * zoom,
        &styles.colors.cyan_bright,
        font,
    );

    let bar_width = box_width - 8.0;
    let bar_height = 8.0;
    let bar_x = box_x + 4.0;
    let bar_y = box_y + box_height - bar_height - 4.0;

    let screen_bar_x = x_offset + (bar_x * zoom);
    let screen_bar_y = y_offset + (bar_y * zoom);

    draw_rectangle(
        screen_bar_x,
        screen_bar_y,
        bar_width * zoom,
        bar_height * zoom,
        styles.colors.gray_ui,
    );

    let fill_width = bar_width * progress.progress;
    draw_rectangle(
        screen_bar_x,
        screen_bar_y,
        fill_width * zoom,
        bar_height * zoom,
        styles.colors.parasite_toxic,
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_image_to_rgba8(path: &str) -> Result<Image, String> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return Err(format!("Failed to open {}: {}", path, error)),
    };
    let mut image_data = Vec::new();
    if let Err(error) = file.read_to_end(&mut image_data) {
        return Err(format!("Failed to read {}: {}", path, error));
    }

    let img = match image::load_from_memory(&image_data) {
        Ok(img) => img.to_rgba8(),
        Err(error) => return Err(format!("Failed to decode image {}: {}", path, error)),
    };

    let width = img.width() as u16;
    let height = img.height() as u16;
    let bytes = img.into_raw();

    Ok(Image {
        width,
        height,
        bytes,
    })
}

pub async fn load_images_parallel(
    paths: Vec<String>,
    progress: &mut LoadingProgress,
    styles: &Styles,
    font: &macroquad::text::Font,
) -> HashMap<String, Image> {
    let unique_paths = deduplicate_paths(paths);

    let total = unique_paths.len();
    progress.text = "Loading graphics...".to_string();
    progress.progress = 0.0;

    ui_loading_screen(progress, styles, font);
    next_frame().await;

    #[cfg(not(target_arch = "wasm32"))]
    {
        let max_threads = get_max_threads();
        let mut pending_paths: Vec<String> = unique_paths;
        let mut active_threads: Vec<(String, Receiver<Result<Image, String>>)> = Vec::new();
        let mut results: HashMap<String, Image> = HashMap::new();

        loop {
            while active_threads.len() < max_threads && !pending_paths.is_empty() {
                let path = pending_paths.remove(0);
                let (tx, rx) = channel();
                let path_clone = path.clone();
                thread::spawn(move || {
                    let result = decode_image_to_rgba8(&path_clone);
                    tx.send(result).unwrap();
                });
                active_threads.push((path, rx));
            }

            let mut i = 0;
            while i < active_threads.len() {
                if let Ok(result) = active_threads[i].1.try_recv() {
                    let (path, _) = active_threads.remove(i);
                    let decoded = result.unwrap();
                    results.insert(path, decoded);
                } else {
                    i += 1;
                }
            }

            progress.progress = results.len() as f32 / total as f32;
            ui_loading_screen(progress, styles, font);

            if results.len() == total {
                break;
            }
            next_frame().await;
        }

        progress.progress = 1.0;
        ui_loading_screen(progress, styles, font);
        next_frame().await;

        results
    }

    #[cfg(target_arch = "wasm32")]
    {
        let mut loaders = Vec::new();
        for path in unique_paths {
            let handle = start_coroutine(async move {
                let asset = load_image(&path).await.unwrap();
                (path, asset)
            });
            loaders.push(handle);
        }

        loop {
            let mut completed = 0;
            let mut all_done = true;
            for handle in &loaders {
                if handle.is_done() {
                    completed += 1;
                } else {
                    all_done = false;
                }
            }
            progress.progress = completed as f32 / total as f32;

            ui_loading_screen(progress, styles, font);

            if all_done {
                progress.progress = 1.0;
                ui_loading_screen(progress, styles, font);
                next_frame().await;
                break;
            }
            next_frame().await;
        }

        let mut assets = HashMap::new();
        for handle in loaders {
            let (path, asset) = handle.retrieve().unwrap();
            assets.insert(path, asset);
        }
        assets
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_ogg_to_wavs(path: &str, rates: &[f32]) -> Result<Vec<Vec<u8>>, String> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return Err(format!("Failed to open {}: {}", path, error)),
    };
    let mut ogg_data = Vec::new();
    if let Err(error) = file.read_to_end(&mut ogg_data) {
        return Err(format!("Failed to read {}: {}", path, error));
    }

    let mut reader = match OggStreamReader::new(Cursor::new(ogg_data)) {
        Ok(reader) => reader,
        Err(error) => return Err(format!("Failed to decode OGG {}: {:?}", path, error)),
    };

    let mut samples: Vec<i16> = Vec::new();
    loop {
        match reader.read_dec_packet_itl() {
            Ok(Some(packet)) => samples.extend(packet),
            Ok(None) => break,
            Err(error) => return Err(format!("Failed to decode packet: {:?}", error)),
        }
    }

    let channels = reader.ident_hdr.audio_channels as u16;
    let sample_rate = reader.ident_hdr.audio_sample_rate;

    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * channels as u32 * 2;
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;

    let mut wav_data = Vec::with_capacity((44 + data_size) as usize);

    wav_data.extend_from_slice(b"RIFF");
    wav_data.extend_from_slice(&file_size.to_le_bytes());
    wav_data.extend_from_slice(b"WAVE");

    wav_data.extend_from_slice(b"fmt ");
    wav_data.extend_from_slice(&16u32.to_le_bytes());
    wav_data.extend_from_slice(&1u16.to_le_bytes());
    wav_data.extend_from_slice(&channels.to_le_bytes());
    wav_data.extend_from_slice(&sample_rate.to_le_bytes());
    wav_data.extend_from_slice(&byte_rate.to_le_bytes());
    wav_data.extend_from_slice(&(channels * 2).to_le_bytes());
    wav_data.extend_from_slice(&16u16.to_le_bytes());

    wav_data.extend_from_slice(b"data");
    wav_data.extend_from_slice(&data_size.to_le_bytes());

    for sample in samples {
        wav_data.extend_from_slice(&sample.to_le_bytes());
    }

    const SAMPLE_RATE_OFFSET: usize = 24;
    const BYTE_RATE_OFFSET: usize = 28;
    let mut wavs = Vec::with_capacity(rates.len());
    for rate_i in 0..rates.len() {
        let mut wav = if rate_i + 1 < rates.len() {
            wav_data.clone()
        } else {
            mem::take(&mut wav_data)
        };
        let scaled_rate = (sample_rate as f32 * rates[rate_i]).round() as u32;
        let scaled_byte_rate = scaled_rate * channels as u32 * 2;
        wav[SAMPLE_RATE_OFFSET..SAMPLE_RATE_OFFSET + 4].copy_from_slice(&scaled_rate.to_le_bytes());
        wav[BYTE_RATE_OFFSET..BYTE_RATE_OFFSET + 4]
            .copy_from_slice(&scaled_byte_rate.to_le_bytes());
        wavs.push(wav);
    }

    Ok(wavs)
}

#[cfg(any(target_arch = "wasm32", test))]
fn ogg_with_scaled_sample_rate(ogg: &[u8], rate: f32) -> Vec<u8> {
    const CHECKSUM_OFFSET: usize = 22;
    const SEGMENT_COUNT_OFFSET: usize = 26;
    const PAGE_HEADER_SIZE: usize = 27;
    const IDENTIFICATION_RATE_OFFSET: usize = 12;

    let mut patched = ogg.to_vec();
    let segment_count = patched[SEGMENT_COUNT_OFFSET] as usize;
    let mut body_size = 0;
    for segment_i in 0..segment_count {
        body_size += patched[PAGE_HEADER_SIZE + segment_i] as usize;
    }
    let body_start = PAGE_HEADER_SIZE + segment_count;
    let page_end = body_start + body_size;

    let rate_offset = body_start + IDENTIFICATION_RATE_OFFSET;
    let mut rate_bytes = [0u8; 4];
    rate_bytes.copy_from_slice(&patched[rate_offset..rate_offset + 4]);
    let sample_rate = (u32::from_le_bytes(rate_bytes) as f32 * rate).round() as u32;
    patched[rate_offset..rate_offset + 4].copy_from_slice(&sample_rate.to_le_bytes());

    patched[CHECKSUM_OFFSET..CHECKSUM_OFFSET + 4].copy_from_slice(&[0; 4]);
    let checksum = ogg_checksum(&patched[..page_end]);
    patched[CHECKSUM_OFFSET..CHECKSUM_OFFSET + 4].copy_from_slice(&checksum.to_le_bytes());
    patched
}

#[cfg(any(target_arch = "wasm32", test))]
fn ogg_checksum(bytes: &[u8]) -> u32 {
    let mut checksum: u32 = 0;
    for byte in bytes {
        checksum ^= (*byte as u32) << 24;
        for _ in 0..8 {
            if checksum & 0x8000_0000 != 0 {
                checksum = (checksum << 1) ^ 0x04c1_1db7;
            } else {
                checksum <<= 1;
            }
        }
    }
    checksum
}

pub async fn load_audio_parallel(
    requests: Vec<(&'static str, &'static [f32])>,
    progress: &mut LoadingProgress,
    styles: &Styles,
    font: &macroquad::text::Font,
) -> HashMap<&'static str, Vec<macroquad::audio::Sound>> {
    let total = requests.len();
    progress.text = "Loading audio...".to_string();
    progress.progress = 0.0;

    ui_loading_screen(progress, styles, font);
    next_frame().await;

    #[cfg(not(target_arch = "wasm32"))]
    {
        let max_threads = get_max_threads();
        let mut pending_requests = requests;
        let mut active_threads: Vec<(&'static str, Receiver<Result<Vec<Vec<u8>>, String>>)> =
            Vec::new();
        let mut results: HashMap<&'static str, Vec<Vec<u8>>> = HashMap::new();

        loop {
            while active_threads.len() < max_threads && !pending_requests.is_empty() {
                let (path, rates) = pending_requests.remove(0);
                let (tx, rx) = channel();
                thread::spawn(move || {
                    let result = decode_ogg_to_wavs(path, rates);
                    tx.send(result).unwrap();
                });
                active_threads.push((path, rx));
            }

            let mut i = 0;
            while i < active_threads.len() {
                if let Ok(result) = active_threads[i].1.try_recv() {
                    let (path, _) = active_threads.remove(i);
                    let wavs = result.unwrap();
                    results.insert(path, wavs);
                } else {
                    i += 1;
                }
            }

            progress.progress = results.len() as f32 / total as f32;
            ui_loading_screen(progress, styles, font);

            if results.len() == total {
                break;
            }
            next_frame().await;
        }

        let mut sounds = HashMap::new();
        for (path, wavs) in results {
            let mut path_sounds = Vec::with_capacity(wavs.len());
            for wav_data in wavs {
                path_sounds.push(load_sound_from_bytes(&wav_data).await.unwrap());
            }
            sounds.insert(path, path_sounds);
        }

        progress.progress = 1.0;
        ui_loading_screen(progress, styles, font);
        next_frame().await;

        sounds
    }

    #[cfg(target_arch = "wasm32")]
    {
        let mut loaders = Vec::new();
        for (path, rates) in requests {
            let handle = start_coroutine(async move {
                let ogg = load_file(path).await.unwrap();
                let mut path_sounds = Vec::with_capacity(rates.len());
                for rate in rates {
                    if *rate == 1.0 {
                        path_sounds.push(load_sound_from_bytes(&ogg).await.unwrap());
                        continue;
                    }
                    let bytes = ogg_with_scaled_sample_rate(&ogg, *rate);
                    path_sounds.push(load_sound_from_bytes(&bytes).await.unwrap());
                }
                (path, path_sounds)
            });
            loaders.push(handle);
        }

        loop {
            let mut completed = 0;
            let mut all_done = true;
            for handle in &loaders {
                if handle.is_done() {
                    completed += 1;
                } else {
                    all_done = false;
                }
            }
            progress.progress = completed as f32 / total as f32;

            ui_loading_screen(progress, styles, font);

            if all_done {
                progress.progress = 1.0;
                ui_loading_screen(progress, styles, font);
                next_frame().await;
                break;
            }
            next_frame().await;
        }

        let mut assets = HashMap::new();
        for handle in loaders {
            let (path, asset) = handle.retrieve().unwrap();
            assets.insert(path, asset);
        }
        assets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfx::{PITCH_SHIFT_RATES, SFX_PATHS};
    use std::fs;

    #[test]
    fn scaled_ogg_decodes_with_the_scaled_rate() {
        let ogg = fs::read(SFX_PATHS[0]).unwrap();
        let original = OggStreamReader::new(Cursor::new(ogg.clone())).unwrap();
        let original_rate = original.ident_hdr.audio_sample_rate;
        for rate in PITCH_SHIFT_RATES {
            let patched = ogg_with_scaled_sample_rate(&ogg, *rate);
            let mut reader = OggStreamReader::new(Cursor::new(patched)).unwrap();
            let expected_rate = (original_rate as f32 * rate).round() as u32;
            assert_eq!(reader.ident_hdr.audio_sample_rate, expected_rate);
            while reader.read_dec_packet_itl().unwrap().is_some() {}
        }
    }
}
