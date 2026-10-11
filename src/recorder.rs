use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context as AnyhowContext, Result};
use std::sync::Mutex;
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    encoder::{
        AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
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

#[derive(Clone)]
pub struct SharedStats {
    pub frame_count: Arc<AtomicU64>,
    pub stop_flag: Arc<AtomicBool>,
    pub started_at: Arc<Mutex<Option<Instant>>>,
    pub last_error: Arc<Mutex<Option<String>>>,
    pub finished: Arc<AtomicBool>,
}

impl SharedStats {
    pub fn new() -> Self {
        Self {
            frame_count: Arc::new(AtomicU64::new(0)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            started_at: Arc::new(Mutex::new(None)),
            last_error: Arc::new(Mutex::new(None)),
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn reset(&self) {
        self.frame_count.store(0, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);
        self.finished.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.started_at.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.started_at
            .lock()
            .ok()
            .and_then(|guard| (*guard).map(|t| t.elapsed().as_secs()))
            .unwrap_or(0)
    }
}

struct CaptureFlags {
    output: PathBuf,
    width: u32,
    height: u32,
    stats: SharedStats,
}

struct Recorder {
    encoder: Option<VideoEncoder>,
    stats: SharedStats,
}

impl GraphicsCaptureApiHandler for Recorder {
    type Flags = CaptureFlags;
    type Error = anyhow::Error;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let flags = ctx.flags;
        let encoder = VideoEncoder::new(
            VideoSettingsBuilder::new(flags.width, flags.height),
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            &flags.output,
        )
        .with_context(|| format!("failed to create encoder for {}", flags.output.display()))?;

        if let Ok(mut guard) = flags.stats.started_at.lock() {
            *guard = Some(Instant::now());
        }

        Ok(Self {
            encoder: Some(encoder),
            stats: flags.stats,
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

        if let Some(encoder) = self.encoder.as_mut() {
            encoder
                .send_frame(frame)
                .context("failed to encode frame")?;
            self.stats.frame_count.fetch_add(1, Ordering::SeqCst);
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

pub fn default_output_path() -> Result<PathBuf> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("recordings");
    std::fs::create_dir_all(&dir).context("failed to create recordings directory")?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    Ok(dir.join(format!("recording_{stamp}.mp4")))
}

pub struct ActiveCapture {
    control: CaptureControl<Recorder, anyhow::Error>,
    pub output: PathBuf,
    pub stats: SharedStats,
}

impl ActiveCapture {
    pub fn start(monitor_index: usize, show_cursor: bool, output: PathBuf) -> Result<Self> {
        if output
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("mp4"))
            != Some(true)
        {
            bail!("output file must end with .mp4");
        }

        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
        }

        let monitors = Monitor::enumerate().context("failed to enumerate monitors")?;
        let monitor = monitors.get(monitor_index).with_context(|| {
            format!(
                "monitor index {monitor_index} not found (found {} monitor(s))",
                monitors.len()
            )
        })?;

        let width = monitor.width().context("failed to read monitor width")? & !1;
        let height = monitor.height().context("failed to read monitor height")? & !1;

        let stats = SharedStats::new();
        stats.reset();

        let cursor = if show_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        };

        let flags = CaptureFlags {
            output: output.clone(),
            width,
            height,
            stats: stats.clone(),
        };

        let settings = Settings::new(
            *monitor,
            cursor,
            DrawBorderSettings::Default,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            flags,
        );

        let control = Recorder::start_free_threaded(settings).context("failed to start capture")?;

        Ok(Self {
            control,
            output,
            stats,
        })
    }

    pub fn is_finished(&self) -> bool {
        self.control.is_finished() || self.stats.finished.load(Ordering::SeqCst)
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.stats.stop_flag.store(true, Ordering::SeqCst);
        let output = self.output.clone();
        // Prefer graceful stop; if the capture already ended, wait/join.
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
