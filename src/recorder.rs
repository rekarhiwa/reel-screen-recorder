use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context as AnyhowContext, Result};
use std::sync::Mutex;
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, ContainerSettingsSubType, VideoEncoder,
        VideoSettingsBuilder, VideoSettingsSubType,
    },
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};

#[derive(Clone)]
pub struct MonitorInfo {
    pub label: String,
    pub is_primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
    Ultra,
    Max,
}

impl Quality {
    pub fn parse(s: &str) -> Self {
        match s {
            "low" => Self::Low,
            "medium" => Self::Medium,
            "ultra" => Self::Ultra,
            "max" | "extreme" => Self::Max,
            _ => Self::High,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
            Self::Max => "max",
        }
    }

    pub fn label_ckb(self) -> &'static str {
        match self {
            Self::Low => "نزم",
            Self::Medium => "ناوەند",
            Self::High => "بەرز · کەم ستۆرەیج",
            Self::Ultra => "زۆر بەرز",
            Self::Max => "بەرزترین · گەیمینگ",
        }
    }

    /// Storage-aware bitrate for sharp screen content (especially 4K + HEVC).
    /// Factor is millibits-per-pixel; HEVC/VP9 need less than H.264 for similar look.
    pub fn bitrate_bps(self, width: u32, height: u32, fps: u32, codec: Codec) -> u32 {
        let pixels = u64::from(width.max(2)) * u64::from(height.max(2));
        let fps = u64::from(fps.max(1));
        let factor = match self {
            Self::Low => 40,
            Self::Medium => 60,
            Self::High => 90,
            Self::Ultra => 140,
            Self::Max => 220,
        };
        let mut bps = pixels
            .saturating_mul(fps)
            .saturating_mul(factor)
            / 1000;
        match codec {
            Codec::H264 => bps = bps.saturating_mul(16) / 10,
            Codec::Vp9 => bps = bps.saturating_mul(9) / 10,
            Codec::Hevc => {}
        }

        let (min_bps, max_bps) = if pixels >= 3_500_000 {
            match self {
                Self::Low => (6_000_000, 16_000_000),
                Self::Medium => (10_000_000, 24_000_000),
                Self::High => (16_000_000, 36_000_000),
                Self::Ultra => (24_000_000, 55_000_000),
                Self::Max => (35_000_000, 80_000_000),
            }
        } else if pixels >= 2_000_000 {
            match self {
                Self::Low => (4_000_000, 10_000_000),
                Self::Medium => (6_000_000, 14_000_000),
                Self::High => (9_000_000, 20_000_000),
                Self::Ultra => (14_000_000, 32_000_000),
                Self::Max => (20_000_000, 48_000_000),
            }
        } else {
            match self {
                Self::Low => (2_500_000, 6_000_000),
                Self::Medium => (3_500_000, 9_000_000),
                Self::High => (5_500_000, 14_000_000),
                Self::Ultra => (8_000_000, 20_000_000),
                Self::Max => (12_000_000, 30_000_000),
            }
        };

        bps.clamp(min_bps, max_bps) as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
    Vp9,
}

impl Codec {
    pub fn parse(s: &str) -> Self {
        match s {
            "hevc" | "h265" => Self::Hevc,
            "vp9" => Self::Vp9,
            _ => Self::H264,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::Hevc => "hevc",
            Self::Vp9 => "vp9",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::Hevc => "HEVC",
            Self::Vp9 => "VP9",
        }
    }

    fn subtype(self) -> VideoSettingsSubType {
        match self {
            Self::H264 => VideoSettingsSubType::H264,
            Self::Hevc => VideoSettingsSubType::HEVC,
            Self::Vp9 => VideoSettingsSubType::VP9,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerFormat {
    Mp4,
    Avi,
    Asf,
}

impl ContainerFormat {
    pub fn parse(s: &str) -> Self {
        match s {
            "avi" => Self::Avi,
            "asf" | "wmv" => Self::Asf,
            _ => Self::Mp4,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Avi => "avi",
            Self::Asf => "asf",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Avi => "avi",
            Self::Asf => "wmv",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mp4 => "MP4",
            Self::Avi => "AVI",
            Self::Asf => "WMV/ASF",
        }
    }

    fn subtype(self) -> ContainerSettingsSubType {
        match self {
            Self::Mp4 => ContainerSettingsSubType::MPEG4,
            Self::Avi => ContainerSettingsSubType::AVI,
            Self::Asf => ContainerSettingsSubType::ASF,
        }
    }
}

pub fn allowed_fps(fps: u32) -> bool {
    matches!(fps, 24 | 30 | 48 | 60 | 90 | 120)
}

#[derive(Clone, Debug)]
pub struct RecordConfig {
    pub monitor_index: usize,
    pub show_cursor: bool,
    pub quality: Quality,
    pub fps: u32,
    pub codec: Codec,
    pub container: ContainerFormat,
    pub mic_enabled: bool,
}

impl Default for RecordConfig {
    fn default() -> Self {
        Self {
            monitor_index: 0,
            show_cursor: true,
            quality: Quality::High,
            fps: 30,
            codec: Codec::Hevc,
            container: ContainerFormat::Mp4,
            mic_enabled: true,
        }
    }
}

impl RecordConfig {
    pub fn normalize(mut self) -> Self {
        if !allowed_fps(self.fps) {
            self.fps = 60;
        }
        self
    }

    pub fn summary_ckb(&self) -> String {
        let mic = if self.mic_enabled { "مایک" } else { "بێدەنگ" };
        format!(
            "{} · {}fps · {} · {} · {}",
            self.quality.label_ckb(),
            self.fps,
            self.codec.label(),
            self.container.label(),
            mic
        )
    }
}

#[derive(Clone)]
pub struct SharedStats {
    pub frame_count: Arc<AtomicU64>,
    pub stop_flag: Arc<AtomicBool>,
    pub pause_flag: Arc<AtomicBool>,
    pub started_at: Arc<Mutex<Option<Instant>>>,
    pub paused_at: Arc<Mutex<Option<Instant>>>,
    pub paused_total: Arc<Mutex<Duration>>,
    pub last_error: Arc<Mutex<Option<String>>>,
    pub finished: Arc<AtomicBool>,
}

impl SharedStats {
    pub fn new() -> Self {
        Self {
            frame_count: Arc::new(AtomicU64::new(0)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            started_at: Arc::new(Mutex::new(None)),
            paused_at: Arc::new(Mutex::new(None)),
            paused_total: Arc::new(Mutex::new(Duration::ZERO)),
            last_error: Arc::new(Mutex::new(None)),
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn reset(&self) {
        self.frame_count.store(0, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);
        self.pause_flag.store(false, Ordering::SeqCst);
        self.finished.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.started_at.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.paused_at.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.paused_total.lock() {
            *guard = Duration::ZERO;
        }
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.pause_flag.load(Ordering::SeqCst)
    }

    pub fn set_paused(&self, paused: bool) {
        if paused {
            if !self.pause_flag.swap(true, Ordering::SeqCst) {
                if let Ok(mut guard) = self.paused_at.lock() {
                    *guard = Some(Instant::now());
                }
            }
        } else if self.pause_flag.swap(false, Ordering::SeqCst) {
            let paused_for = self
                .paused_at
                .lock()
                .ok()
                .and_then(|mut g| g.take())
                .map(|t| t.elapsed())
                .unwrap_or(Duration::ZERO);
            if let Ok(mut total) = self.paused_total.lock() {
                *total += paused_for;
            }
        }
    }

    pub fn elapsed_secs(&self) -> u64 {
        let started = self
            .started_at
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);
        let paused_total = self
            .paused_total
            .lock()
            .ok()
            .map(|g| *g)
            .unwrap_or(Duration::ZERO);
        let current_pause = if self.is_paused() {
            self.paused_at
                .lock()
                .ok()
                .and_then(|g| *g)
                .map(|t| t.elapsed())
                .unwrap_or(Duration::ZERO)
        } else {
            Duration::ZERO
        };
        started
            .saturating_sub(paused_total)
            .saturating_sub(current_pause)
            .as_secs()
    }
}

struct CaptureFlags {
    output: PathBuf,
    width: u32,
    height: u32,
    bitrate: u32,
    fps: u32,
    codec: Codec,
    container: ContainerFormat,
    mic_enabled: bool,
    mic_buffer: Option<crate::audio::MicBuffer>,
    stats: SharedStats,
}

struct Recorder {
    encoder: Option<VideoEncoder>,
    stats: SharedStats,
    min_frame_interval: Duration,
    last_sent: Option<Instant>,
    mic_enabled: bool,
    mic_buffer: Option<crate::audio::MicBuffer>,
    fps: u32,
}

impl GraphicsCaptureApiHandler for Recorder {
    type Flags = CaptureFlags;
    type Error = anyhow::Error;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let flags = ctx.flags;
        let video = VideoSettingsBuilder::new(flags.width, flags.height)
            .bitrate(flags.bitrate)
            .frame_rate(flags.fps)
            .sub_type(flags.codec.subtype());

        let audio = if flags.mic_enabled {
            AudioSettingsBuilder::new()
                .sample_rate(48_000)
                .channel_count(2)
                .bit_per_sample(16)
        } else {
            AudioSettingsBuilder::default().disabled(true)
        };

        let container = ContainerSettingsBuilder::new().sub_type(flags.container.subtype());
        let encoder = VideoEncoder::new(video, audio, container, &flags.output)
            .with_context(|| format!("failed to create encoder for {}", flags.output.display()))?;

        if let Ok(mut guard) = flags.stats.started_at.lock() {
            *guard = Some(Instant::now());
        }

        let fps = flags.fps.max(1);
        Ok(Self {
            encoder: Some(encoder),
            stats: flags.stats,
            min_frame_interval: Duration::from_secs_f64(1.0 / f64::from(fps)),
            last_sent: None,
            mic_enabled: flags.mic_enabled,
            mic_buffer: flags.mic_buffer,
            fps,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.stats.stop_flag.load(Ordering::SeqCst) {
            if let Some(encoder) = self.encoder.take() {
                encoder.finish().context("failed to finish encoder")?;
            }
            self.stats.finished.store(true, Ordering::SeqCst);
            capture_control.stop();
            return Ok(());
        }

        if self.stats.is_paused() {
            // Discard mic backlog so resume doesn't dump paused audio.
            if let Some(buf) = &self.mic_buffer {
                let _ = buf.drain(usize::MAX / 4);
            }
            return Ok(());
        }

        // Pace to target FPS, but allow slight early frames so motion stays smooth.
        let now = Instant::now();
        if let Some(last) = self.last_sent {
            let early_slack = self.min_frame_interval / 5;
            if now.duration_since(last) + early_slack < self.min_frame_interval {
                return Ok(());
            }
        }

        if let Some(encoder) = self.encoder.as_mut() {
            self.last_sent = Some(now);
            self.stats.frame_count.fetch_add(1, Ordering::SeqCst);

            if self.mic_enabled {
                // Exactly one video-frame of stereo i16 @ 48 kHz (channels*bytes).
                // Previous code asked for 2× and padded silence — audio ran ahead and sounded empty.
                let frames = (48_000 / self.fps.max(1)).max(1) as usize;
                let want = frames * 4;
                // Allow catching up a bit if mic backlog grew (max ~2 frames).
                let take = want.saturating_mul(2);
                let mut pcm = self
                    .mic_buffer
                    .as_ref()
                    .map(|b| b.drain(take))
                    .unwrap_or_default();
                // Keep A/V lock: never send less than one frame (pad silence if quiet).
                if pcm.len() < want {
                    pcm.resize(want, 0);
                } else {
                    // Trim to whole frames; prefer ~1 frame unless backlog is large.
                    let keep = if pcm.len() > want + want / 2 {
                        ((pcm.len() / 4) * 4).min(want * 2)
                    } else {
                        want
                    };
                    pcm.truncate(keep);
                }
                encoder
                    .send_frame_with_audio(frame, &pcm)
                    .context("failed to encode frame+audio")?;
            } else {
                encoder
                    .send_frame(frame)
                    .context("failed to encode frame")?;
            }
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        if let Some(encoder) = self.encoder.take() {
            encoder
                .finish()
                .context("failed to finish encoder on close")?;
        }
        self.stats.finished.store(true, Ordering::SeqCst);
        Ok(())
    }
}

pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    let monitors = Monitor::enumerate().context("failed to enumerate monitors")?;
    if monitors.is_empty() {
        bail!("no monitors found");
    }

    let primary = Monitor::primary().ok();
    let mut out = Vec::with_capacity(monitors.len());

    for (index, monitor) in monitors.iter().enumerate() {
        let name = monitor
            .name()
            .ok()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| format!("مۆنیتەر {}", index + 1));
        let width = monitor.width().unwrap_or(0);
        let height = monitor.height().unwrap_or(0);
        let is_primary = primary.as_ref() == Some(monitor);
        let label = if is_primary {
            format!("{name}  ·  {width}×{height}  ·  سەرەکی")
        } else {
            format!("{name}  ·  {width}×{height}")
        };
        out.push(MonitorInfo {
            label,
            is_primary,
        });
    }

    Ok(out)
}

pub fn recordings_dir() -> Result<PathBuf> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("recordings");
    std::fs::create_dir_all(&dir).context("failed to create recordings directory")?;
    Ok(dir)
}

pub fn default_output_path() -> Result<PathBuf> {
    output_paths(1, ContainerFormat::Mp4).map(|mut v| v.remove(0))
}

/// One file for single-screen, or `*_s1.ext` / `*_s2.ext` for dual.
pub fn output_paths(count: usize, container: ContainerFormat) -> Result<Vec<PathBuf>> {
    let dir = recordings_dir()?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let ext = container.extension();
    let count = count.max(1);
    if count == 1 {
        Ok(vec![dir.join(format!("recording_{stamp}.{ext}"))])
    } else {
        Ok((1..=count)
            .map(|i| dir.join(format!("recording_{stamp}_s{i}.{ext}")))
            .collect())
    }
}

pub struct ActiveCapture {
    control: CaptureControl<Recorder, anyhow::Error>,
    pub output: PathBuf,
    pub stats: SharedStats,
    pub config: RecordConfig,
    pub bitrate_bps: u32,
    pub width: u32,
    pub height: u32,
}

impl ActiveCapture {
    pub fn start(
        config: RecordConfig,
        output: PathBuf,
        mic_buffer: Option<crate::audio::MicBuffer>,
    ) -> Result<Self> {
        let config = config.normalize();

        let ext_ok = output
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                let e = e.to_ascii_lowercase();
                e == "mp4" || e == "avi" || e == "wmv" || e == "asf"
            })
            .unwrap_or(false);
        if !ext_ok {
            bail!("output file must end with .mp4 / .avi / .wmv");
        }

        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
        }

        let monitors = Monitor::enumerate().context("failed to enumerate monitors")?;
        let monitor = monitors.get(config.monitor_index).with_context(|| {
            format!(
                "monitor index {} not found (found {} monitor(s))",
                config.monitor_index,
                monitors.len()
            )
        })?;

        let width = monitor.width().context("failed to read monitor width")? & !1;
        let height = monitor.height().context("failed to read monitor height")? & !1;

        let stats = SharedStats::new();
        stats.reset();

        let cursor = if config.show_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        };

        let bitrate = config.quality.bitrate_bps(
            width,
            height,
            config.fps,
            config.codec,
        );

        let flags = CaptureFlags {
            output: output.clone(),
            width,
            height,
            bitrate,
            fps: config.fps,
            codec: config.codec,
            container: config.container,
            mic_enabled: config.mic_enabled,
            mic_buffer,
            stats: stats.clone(),
        };

        let interval = Duration::from_secs_f64(1.0 / f64::from(config.fps.max(1)));
        let settings = Settings::new(
            *monitor,
            cursor,
            DrawBorderSettings::Default,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Custom(interval),
            DirtyRegionSettings::Default,
            // Match Media Foundation encoder input (avoids color conversion softness).
            ColorFormat::Bgra8,
            flags,
        );

        let control = Recorder::start_free_threaded(settings).context("failed to start capture")?;

        Ok(Self {
            control,
            output,
            stats,
            config,
            bitrate_bps: bitrate,
            width,
            height,
        })
    }

    pub fn is_finished(&self) -> bool {
        self.control.is_finished() || self.stats.finished.load(Ordering::SeqCst)
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.stats.stop_flag.store(true, Ordering::SeqCst);
        let output = self.output.clone();
        if self.control.is_finished() {
            let _ = self.control.wait();
        } else {
            self.control.stop().context("failed to stop capture")?;
        }

        if !output.exists() {
            bail!("recording finished but output file was not created");
        }
        Ok(output)
    }
}

/// One or more concurrent screen captures (dual-screen capable).
pub struct RecordingSession {
    pub captures: Vec<ActiveCapture>,
}

impl RecordingSession {
    pub fn is_recording(&self) -> bool {
        !self.captures.is_empty()
    }

    pub fn is_finished(&self) -> bool {
        self.captures.iter().any(|c| c.is_finished())
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.captures
            .first()
            .map(|c| c.stats.elapsed_secs())
            .unwrap_or(0)
    }

    pub fn frame_count(&self) -> u64 {
        self.captures
            .iter()
            .map(|c| c.stats.frame_count.load(Ordering::SeqCst))
            .sum()
    }

    pub fn summary_ckb(&self) -> String {
        let n = self.captures.len();
        let base = self
            .captures
            .first()
            .map(|c| c.config.summary_ckb())
            .unwrap_or_default();
        if n > 1 {
            format!("{base} · دوو سکرین")
        } else {
            base
        }
    }

    pub fn is_paused(&self) -> bool {
        self.captures
            .first()
            .map(|c| c.stats.is_paused())
            .unwrap_or(false)
    }

    pub fn set_paused(&self, paused: bool) {
        for c in &self.captures {
            c.stats.set_paused(paused);
        }
    }

    pub fn stop(self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::with_capacity(self.captures.len());
        let mut errors = Vec::new();
        for capture in self.captures {
            match capture.stop() {
                Ok(p) => paths.push(p),
                Err(e) => errors.push(e.to_string()),
            }
        }
        if paths.is_empty() {
            bail!(errors.join("; "));
        }
        Ok(paths)
    }
}
