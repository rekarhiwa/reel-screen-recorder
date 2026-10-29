//! Live monitor preview for the UI stage (OBS-like smoothness).
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use base64::Engine;
use image::imageops::FilterType;
use xcap::Monitor;

pub struct PreviewFrame {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PreviewCrop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PreviewCrop {
    pub fn is_valid(self) -> bool {
        self.w >= 32 && self.h >= 32
    }
}

pub struct PreviewHub {
    index: Arc<AtomicUsize>,
    enabled: Arc<AtomicBool>,
    /// When true (recording), slightly lower res/FPS so capture stays smooth.
    recording: Arc<AtomicBool>,
    crop_on: Arc<AtomicBool>,
    crop_x: Arc<AtomicU32>,
    crop_y: Arc<AtomicU32>,
    crop_w: Arc<AtomicU32>,
    crop_h: Arc<AtomicU32>,
    latest: Arc<Mutex<Option<PreviewFrame>>>,
}

impl PreviewHub {
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let index = Arc::new(AtomicUsize::new(0));
        let enabled = Arc::new(AtomicBool::new(true));
        let recording = Arc::new(AtomicBool::new(false));
        let crop_on = Arc::new(AtomicBool::new(false));
        let crop_x = Arc::new(AtomicU32::new(0));
        let crop_y = Arc::new(AtomicU32::new(0));
        let crop_w = Arc::new(AtomicU32::new(0));
        let crop_h = Arc::new(AtomicU32::new(0));
        let latest = Arc::new(Mutex::new(None));

        let index_t = Arc::clone(&index);
        let enabled_t = Arc::clone(&enabled);
        let recording_t = Arc::clone(&recording);
        let crop_on_t = Arc::clone(&crop_on);
        let crop_x_t = Arc::clone(&crop_x);
        let crop_y_t = Arc::clone(&crop_y);
        let crop_w_t = Arc::clone(&crop_w);
        let crop_h_t = Arc::clone(&crop_h);
        let latest_t = Arc::clone(&latest);
        thread::spawn(move || {
            preview_loop(
                index_t,
                enabled_t,
                recording_t,
                crop_on_t,
                crop_x_t,
                crop_y_t,
                crop_w_t,
                crop_h_t,
                latest_t,
                wake,
            )
        });

        Self {
            index,
            enabled,
            recording,
            crop_on,
            crop_x,
            crop_y,
            crop_w,
            crop_h,
            latest,
        }
    }

    pub fn set_monitor_index(&self, index: usize) {
        self.index.store(index, Ordering::SeqCst);
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::SeqCst);
    }

    pub fn set_recording(&self, on: bool) {
        self.recording.store(on, Ordering::SeqCst);
    }

    pub fn set_crop(&self, crop: Option<PreviewCrop>) {
        if let Some(c) = crop.filter(|c| c.is_valid()) {
            self.crop_x.store(c.x, Ordering::SeqCst);
            self.crop_y.store(c.y, Ordering::SeqCst);
            self.crop_w.store(c.w, Ordering::SeqCst);
            self.crop_h.store(c.h, Ordering::SeqCst);
            self.crop_on.store(true, Ordering::SeqCst);
        } else {
            self.crop_on.store(false, Ordering::SeqCst);
        }
    }

    pub fn take(&self) -> Option<PreviewFrame> {
        self.latest.lock().ok().and_then(|mut g| g.take())
    }
}

fn preview_loop(
    index: Arc<AtomicUsize>,
    enabled: Arc<AtomicBool>,
    recording: Arc<AtomicBool>,
    crop_on: Arc<AtomicBool>,
    crop_x: Arc<AtomicU32>,
    crop_y: Arc<AtomicU32>,
    crop_w: Arc<AtomicU32>,
    crop_h: Arc<AtomicU32>,
    latest: Arc<Mutex<Option<PreviewFrame>>>,
    wake: impl Fn(),
) {
    let mut cached: Option<(usize, Monitor, Instant)> = None;

    loop {
        if !enabled.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(200));
            continue;
        }

        let rec = recording.load(Ordering::SeqCst);
        // Idle: sharp ~24fps. Recording: still fluid ~15fps, lighter encode.
        let (max_w, quality, target_fps) = if rec {
            (960u32, 72u8, 15u32)
        } else {
            (1440u32, 88u8, 24u32)
        };
        let target = Duration::from_secs_f64(1.0 / f64::from(target_fps.max(1)));
        let tick = Instant::now();

        let idx = index.load(Ordering::SeqCst);
        let need_refresh = cached
            .as_ref()
            .map(|(i, _, at)| *i != idx || at.elapsed() > Duration::from_secs(3))
            .unwrap_or(true);
        if need_refresh {
            match Monitor::all() {
                Ok(list) if !list.is_empty() => {
                    let mon = list
                        .get(idx)
                        .cloned()
                        .or_else(|| list.first().cloned());
                    if let Some(m) = mon {
                        cached = Some((idx, m, Instant::now()));
                    }
                }
                _ => {
                    thread::sleep(Duration::from_millis(300));
                    continue;
                }
            }
        }

        let Some((_, ref monitor, _)) = cached else {
            thread::sleep(Duration::from_millis(300));
            continue;
        };

        let crop = if crop_on.load(Ordering::SeqCst) {
            Some(PreviewCrop {
                x: crop_x.load(Ordering::SeqCst),
                y: crop_y.load(Ordering::SeqCst),
                w: crop_w.load(Ordering::SeqCst),
                h: crop_h.load(Ordering::SeqCst),
            })
        } else {
            None
        };

        match capture_jpeg_data_url(monitor, crop, max_w, quality) {
            Ok(frame) => {
                if let Ok(mut g) = latest.lock() {
                    *g = Some(frame);
                }
                wake();
            }
            Err(_) => {
                // Force re-resolve monitor next loop.
                cached = None;
                thread::sleep(Duration::from_millis(200));
                continue;
            }
        }

        let elapsed = tick.elapsed();
        if elapsed < target {
            thread::sleep(target - elapsed);
        }
    }
}

fn capture_jpeg_data_url(
    monitor: &Monitor,
    crop: Option<PreviewCrop>,
    max_w: u32,
    quality: u8,
) -> Result<PreviewFrame> {
    let img = monitor
        .capture_image()
        .context("failed to capture monitor image")?;
    let img = if let Some(c) = crop.filter(|c| c.is_valid()) {
        let iw = img.width();
        let ih = img.height();
        let x = c.x.min(iw.saturating_sub(1));
        let y = c.y.min(ih.saturating_sub(1));
        let w = c.w.min(iw.saturating_sub(x)).max(1);
        let h = c.h.min(ih.saturating_sub(y)).max(1);
        image::imageops::crop_imm(&img, x, y, w, h).to_image()
    } else {
        img
    };
    let (ow, oh) = (img.width(), img.height());
    let scaled = if ow > max_w {
        let nh = ((oh as f32) * (max_w as f32 / ow as f32)).round() as u32;
        // Triangle is fast enough for realtime; looks sharper than Nearest.
        image::imageops::resize(&img, max_w.max(1), nh.max(1), FilterType::Triangle)
    } else {
        img
    };

    let rgb = image::DynamicImage::ImageRgba8(scaled).into_rgb8();
    let mut bytes: Vec<u8> = Vec::with_capacity(256 * 1024);
    {
        let mut cursor = Cursor::new(&mut bytes);
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
        encoder
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .context("jpeg encode")?;
    }

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(PreviewFrame {
        data_url: format!("data:image/jpeg;base64,{b64}"),
        width: ow,
        height: oh,
    })
}
