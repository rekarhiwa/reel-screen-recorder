#![windows_subsystem = "windows"]
#![recursion_limit = "256"]

mod audio;
mod config;
mod hotkeys;
mod icon_data;
mod preview;
mod recorder;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;
use tao::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Window, WindowBuilder},
};
#[cfg(windows)]
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use wry::WebViewBuilder;
#[cfg(windows)]
use wry::WebViewExtWindows;

#[cfg(windows)]
mod win_capture_exclude {
    /// Visible on your screen, but omitted from screen capture / recordings.
    pub const WDA_NONE: u32 = 0;
    pub const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;

    #[link(name = "user32")]
    extern "system" {
        pub fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
    }
}

use audio::{MicCapture, NoiseReduceConfig};
use config::AppConfig;
use hotkeys::{HotAction, Hotkeys};
use recorder::{CaptureMode, RecordConfig, RecordingSession, RegionRect};

#[derive(Debug, Deserialize)]
struct IpcMsg {
    cmd: String,
    #[serde(default)]
    monitor: usize,
    #[serde(default)]
    monitor_b: Option<usize>,
    #[serde(default)]
    dual: Option<bool>,
    #[serde(default)]
    capture_mode: Option<String>,
    #[serde(default)]
    region_x: Option<u32>,
    #[serde(default)]
    region_y: Option<u32>,
    #[serde(default)]
    region_w: Option<u32>,
    #[serde(default)]
    region_h: Option<u32>,
    #[serde(default)]
    game_hwnd: Option<String>,
    #[serde(default)]
    game_title: Option<String>,
    #[serde(default)]
    cursor: bool,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    fps: Option<u32>,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    container: Option<String>,
    #[serde(default)]
    mic: Option<bool>,
    #[serde(default)]
    #[allow(non_snake_case)]
    micDevice: Option<String>,
    #[serde(default)]
    noise_reduce: Option<bool>,
    #[serde(default)]
    #[allow(non_snake_case)]
    noiseStrength: Option<u8>,
    #[serde(default)]
    camera: Option<bool>,
    #[serde(default)]
    shape: Option<String>,
    #[serde(default)]
    greenscreen: Option<bool>,
    #[serde(default)]
    strength: Option<u8>,
    #[serde(default)]
    #[allow(non_snake_case)]
    camSize: Option<u32>,
    #[serde(default)]
    #[allow(non_snake_case)]
    camDevice: Option<String>,
    #[serde(default)]
    #[allow(non_snake_case)]
    camFilter: Option<String>,
    #[serde(default)]
    #[allow(non_snake_case)]
    gsColor: Option<String>,
    #[serde(default)]
    #[allow(non_snake_case)]
    gsSoftness: Option<u8>,
    #[serde(default)]
    hotkey_start_stop: Option<String>,
    #[serde(default)]
    hotkey_mic: Option<String>,
    #[serde(default)]
    hotkey_camera: Option<String>,
    #[serde(default)]
    hotkey_pause: Option<String>,
    #[serde(default)]
    hotkey_annotate: Option<String>,
    #[serde(default)]
    annotate: Option<bool>,
    #[serde(default)]
    pass: Option<bool>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    size: Option<u8>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    dx: Option<f64>,
    #[serde(default)]
    dy: Option<f64>,
    #[serde(default)]
    devices: Option<Vec<CamDeviceInfo>>,
    #[serde(default)]
    current: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CamDeviceInfo {
    id: String,
    #[serde(default)]
    label: String,
}

struct AppState {
    session: Option<RecordingSession>,
    mic: Option<MicCapture>,
    saving: bool,
    last_paths: Vec<PathBuf>,
    status: String,
    error: Option<String>,
    toast: Option<String>,
    toast_error: bool,
    monitors: Vec<String>,
    mic_devices: Vec<(String, String)>,
    config: AppConfig,
    camera_visible: bool,
    camera_devices: Vec<(String, String)>,
    annotate_visible: bool,
    annotate_pass: bool,
    annotate_focus_toolbar: bool,
    annotate_clear: bool,
    annotate_toolbar_hidden: bool,
    annotate_cmd: Option<serde_json::Value>,
    rebind_hotkeys: bool,
    suspend_hotkeys: bool,
    pending_drag: Option<(f64, f64)>,
    region_picker_visible: bool,
    game_windows: Vec<recorder::WindowInfo>,
}

impl AppState {
    fn new() -> Self {
        let infos = recorder::list_monitors().unwrap_or_default();
        let selected = infos.iter().position(|m| m.is_primary).unwrap_or(0);
        let monitors: Vec<String> = infos.into_iter().map(|m| m.label).collect();
        let mic_list = audio::list_input_devices();
        let mic_devices: Vec<(String, String)> = mic_list
            .iter()
            .map(|d| (d.id.clone(), d.label.clone()))
            .collect();
        let mut config = AppConfig::load();
        if !config.mic_device_id.is_empty()
            && !mic_devices.iter().any(|(id, _)| id == &config.mic_device_id)
        {
            config.mic_device_id.clear();
        }
        if config.monitor_index >= monitors.len() {
            config.monitor_index = selected;
        }
        if config.monitor_index_b >= monitors.len() || config.monitor_index_b == config.monitor_index
        {
            config.monitor_index_b = (0..monitors.len())
                .find(|&i| i != config.monitor_index)
                .unwrap_or(config.monitor_index);
        }
        if monitors.len() < 2 {
            config.dual_screen = false;
            if config.capture_mode == "dual" {
                config.capture_mode = "screen".into();
            }
        }
        if config.dual_screen && config.capture_mode == "screen" {
            config.capture_mode = "dual".into();
        }
        let game_windows = recorder::list_windows().unwrap_or_default();
        Self {
            session: None,
            mic: None,
            saving: false,
            last_paths: Vec::new(),
            status: "ئامادەیە بۆ تۆمارکردنی شاشەکەت".into(),
            error: None,
            toast: None,
            toast_error: false,
            monitors,
            mic_devices,
            config,
            camera_visible: false,
            camera_devices: Vec::new(),
            annotate_visible: false,
            annotate_pass: false,
            annotate_focus_toolbar: false,
            annotate_clear: false,
            annotate_toolbar_hidden: false,
            annotate_cmd: None,
            rebind_hotkeys: false,
            suspend_hotkeys: false,
            pending_drag: None,
            region_picker_visible: false,
            game_windows,
        }
    }

    fn capture_mode(&self) -> CaptureMode {
        CaptureMode::parse(&self.config.capture_mode)
    }

    fn refresh_game_windows(&mut self) {
        self.game_windows = recorder::list_windows().unwrap_or_default();
    }

    fn is_recording(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.is_recording())
    }

    fn apply_ipc_settings(&mut self, msg: &IpcMsg) {
        self.config.monitor_index = msg.monitor;
        if let Some(b) = msg.monitor_b {
            self.config.monitor_index_b = b;
        }
        if let Some(mode) = &msg.capture_mode {
            let mode = CaptureMode::parse(mode);
            self.config.capture_mode = mode.as_str().into();
            self.config.dual_screen = mode == CaptureMode::Dual && self.monitors.len() >= 2;
            if mode == CaptureMode::Dual && self.monitors.len() < 2 {
                self.config.capture_mode = CaptureMode::Screen.as_str().into();
            }
        } else if let Some(d) = msg.dual {
            self.config.dual_screen = d && self.monitors.len() >= 2;
            self.config.capture_mode = if self.config.dual_screen {
                CaptureMode::Dual.as_str().into()
            } else if self.capture_mode() == CaptureMode::Dual {
                CaptureMode::Screen.as_str().into()
            } else {
                self.config.capture_mode.clone()
            };
        }
        if self.config.dual_screen && self.config.monitor_index_b == self.config.monitor_index {
            self.config.monitor_index_b = (0..self.monitors.len())
                .find(|&i| i != self.config.monitor_index)
                .unwrap_or(self.config.monitor_index);
        }
        if let (Some(x), Some(y), Some(w), Some(h)) =
            (msg.region_x, msg.region_y, msg.region_w, msg.region_h)
        {
            self.config.region_x = x;
            self.config.region_y = y;
            self.config.region_w = w;
            self.config.region_h = h;
        }
        if let Some(h) = &msg.game_hwnd {
            self.config.game_hwnd = h.clone();
        }
        if let Some(t) = &msg.game_title {
            self.config.game_title = t.clone();
        }
        self.config.show_cursor = msg.cursor;
        if let Some(q) = &msg.quality {
            self.config.quality = q.clone();
        }
        if let Some(fps) = msg.fps {
            self.config.fps = fps;
        }
        if let Some(c) = &msg.codec {
            self.config.codec = c.clone();
        }
        if let Some(c) = &msg.container {
            self.config.container = c.clone();
        }
        if let Some(m) = msg.mic {
            self.config.mic_enabled = m;
        }
        if let Some(d) = &msg.micDevice {
            self.config.mic_device_id = d.clone();
        }
        if let Some(n) = msg.noise_reduce {
            self.config.noise_reduce = n;
        }
        if let Some(s) = msg.noiseStrength {
            self.config.noise_strength = s.clamp(0, 100);
        }
        if let Some(c) = msg.camera {
            self.config.camera_enabled = c;
        }
        if let Some(s) = &msg.shape {
            self.config.camera_shape = s.clone();
        }
        if let Some(g) = msg.greenscreen {
            self.config.greenscreen = g;
        }
        if let Some(s) = msg.strength {
            self.config.greenscreen_strength = s.clamp(5, 95);
        }
        if let Some(s) = msg.camSize {
            self.config.camera_size = s.clamp(120, 480);
        }
        if let Some(d) = &msg.camDevice {
            self.config.camera_device_id = d.clone();
        }
        if let Some(f) = &msg.camFilter {
            self.config.camera_filter = f.clone();
        }
        if let Some(c) = &msg.gsColor {
            self.config.greenscreen_color = normalize_hex_color(c);
        }
        if let Some(s) = msg.gsSoftness {
            self.config.greenscreen_softness = s.clamp(0, 90);
        }
        if let Some(h) = &msg.hotkey_start_stop {
            self.config.hotkey_start_stop = h.clone();
        }
        if let Some(h) = &msg.hotkey_mic {
            self.config.hotkey_mic = h.clone();
        }
        if let Some(h) = &msg.hotkey_camera {
            self.config.hotkey_camera = h.clone();
        }
        if let Some(h) = &msg.hotkey_pause {
            self.config.hotkey_pause = h.clone();
        }
        if let Some(h) = &msg.hotkey_annotate {
            self.config.hotkey_annotate = h.clone();
        }
        if let Some(a) = msg.annotate {
            self.config.annotate_enabled = a;
        }
        if !recorder::allowed_fps(self.config.fps) {
            self.config.fps = 60;
        }
        self.config.save();
    }

    fn toggle_pause(&mut self) -> bool {
        let Some(session) = &self.session else {
            return false;
        };
        let next = !session.is_paused();
        session.set_paused(next);
        if next {
            self.status = "تۆمار وەستێنرا (پاوس) — دەتوانیت بەردەوام بیت".into();
            self.push_toast("پاوس", false);
        } else {
            self.status = "بەردەوامبوونەوەی تۆمار".into();
            self.push_toast("بەردەوامبوونەوە", false);
        }
        true
    }

    fn apply_camera_visuals(&mut self, msg: &IpcMsg) {
        if let Some(s) = &msg.shape {
            self.config.camera_shape = s.clone();
        }
        if let Some(g) = msg.greenscreen {
            self.config.greenscreen = g;
        }
        if let Some(s) = msg.strength {
            self.config.greenscreen_strength = s.clamp(5, 95);
        }
        if let Some(s) = msg.camSize {
            self.config.camera_size = s.clamp(120, 480);
        }
        if let Some(d) = &msg.camDevice {
            self.config.camera_device_id = d.clone();
        }
        if let Some(f) = &msg.camFilter {
            self.config.camera_filter = f.clone();
        }
        if let Some(c) = &msg.gsColor {
            self.config.greenscreen_color = normalize_hex_color(c);
        }
        if let Some(s) = msg.gsSoftness {
            self.config.greenscreen_softness = s.clamp(0, 90);
        }
        self.config.save();
    }

    fn record_config(&self) -> RecordConfig {
        let mode = self.capture_mode();
        let game_hwnd = self
            .config
            .game_hwnd
            .trim()
            .parse::<isize>()
            .unwrap_or(0);
        RecordConfig {
            monitor_index: self.config.monitor_index,
            show_cursor: self.config.show_cursor,
            quality: self.config.quality(),
            fps: self.config.fps,
            codec: self.config.codec(),
            container: self.config.container(),
            mic_enabled: self.config.mic_enabled,
            mode,
            region: RegionRect {
                x: self.config.region_x,
                y: self.config.region_y,
                w: self.config.region_w,
                h: self.config.region_h,
            },
            game_hwnd,
            game_title: self.config.game_title.clone(),
        }
    }

    fn snapshot(&self) -> serde_json::Value {
        let (elapsed, frames, recording, paused) = if let Some(s) = &self.session {
            (s.elapsed_secs(), s.frame_count(), true, s.is_paused())
        } else {
            (0, 0, false, false)
        };

        let mode = self.capture_mode();
        let summary = self.record_config().summary_ckb();
        let region_label = if self.config.region_w >= 32 && self.config.region_h >= 32 {
            format!(
                "{}×{} @ {},{}",
                self.config.region_w,
                self.config.region_h,
                self.config.region_x,
                self.config.region_y
            )
        } else {
            "هەڵنەبژێردراوە".into()
        };

        json!({
            "monitors": self.monitors,
            "selected": self.config.monitor_index,
            "selectedB": self.config.monitor_index_b,
            "dual": mode == CaptureMode::Dual && self.monitors.len() >= 2,
            "dualAvailable": self.monitors.len() >= 2,
            "captureMode": mode.as_str(),
            "regionX": self.config.region_x,
            "regionY": self.config.region_y,
            "regionW": self.config.region_w,
            "regionH": self.config.region_h,
            "regionLabel": region_label,
            "regionReady": self.config.region_w >= 32 && self.config.region_h >= 32,
            "gameHwnd": self.config.game_hwnd,
            "gameTitle": self.config.game_title,
            "windows": self.game_windows.iter().map(|w| json!({
                "hwnd": w.hwnd.to_string(),
                "title": w.title,
                "process": w.process,
            })).collect::<Vec<_>>(),
            "cursor": self.config.show_cursor,
            "quality": self.config.quality,
            "fps": self.config.fps,
            "codec": self.config.codec,
            "container": self.config.container,
            "mic": self.config.mic_enabled,
            "micDevices": self.mic_devices.iter().map(|(id, label)| json!({"id": id, "label": label})).collect::<Vec<_>>(),
            "micDevice": self.config.mic_device_id,
            "noiseReduce": self.config.noise_reduce,
            "noiseStrength": self.config.noise_strength,
            "camera": self.camera_visible,
            "camDevices": self.camera_devices.iter().map(|(id, label)| json!({"id": id, "label": label})).collect::<Vec<_>>(),
            "camDevice": self.config.camera_device_id,
            "shape": self.config.camera_shape,
            "camFilter": self.config.camera_filter,
            "greenscreen": self.config.greenscreen,
            "strength": self.config.greenscreen_strength,
            "gsColor": self.config.greenscreen_color,
            "gsSoftness": self.config.greenscreen_softness,
            "camSize": self.config.camera_size,
            "hotkey_start_stop": self.config.hotkey_start_stop,
            "hotkey_mic": self.config.hotkey_mic,
            "hotkey_camera": self.config.hotkey_camera,
            "hotkey_pause": self.config.hotkey_pause,
            "hotkey_annotate": self.config.hotkey_annotate,
            "annotate": self.config.annotate_enabled,
            "annotateVisible": self.annotate_visible,
            "summary": summary,
            "elapsed": elapsed,
            "frames": frames,
            "recording": recording,
            "paused": paused,
            "saving": self.saving,
            "status": self.status,
            "error": self.error,
            "lastPath": self.last_paths.first().map(|p| p.display().to_string()),
            "lastPaths": self.last_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "toast": self.toast,
            "toastError": self.toast_error,
        })
    }

    fn take_toast(&mut self) {
        self.toast = None;
        self.toast_error = false;
    }

    fn push_toast(&mut self, msg: impl Into<String>, error: bool) {
        self.toast = Some(msg.into());
        self.toast_error = error;
    }
}

fn main() -> Result<()> {
    #[derive(Debug, Clone, Copy)]
    enum AppEvent {
        Ipc,
        Preview,
    }
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let window_icon = tao::window::Icon::from_rgba(
        icon_data::RGBA.to_vec(),
        icon_data::WIDTH,
        icon_data::HEIGHT,
    )
    .ok();

    let main_window = Rc::new(
        WindowBuilder::new()
            .with_title("ڕیل — سکرین ڕیکۆردەر")
            .with_inner_size(LogicalSize::new(1180.0, 760.0))
            .with_min_inner_size(LogicalSize::new(980.0, 680.0))
            .with_resizable(true)
            .with_window_icon(window_icon.clone())
            .build(&event_loop)
            .context("failed to create window")?,
    );

    let cam_window = Rc::new(
        WindowBuilder::new()
            .with_title("ڕیل — کامێرا")
            .with_inner_size(LogicalSize::new(220.0, 220.0))
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top(true)
            .with_resizable(false)
            .with_visible(false)
            .with_window_icon(window_icon.clone())
            .build(&event_loop)
            .context("failed to create camera window")?,
    );

    #[allow(unused_mut)]
    let mut annotate_builder = WindowBuilder::new()
        .with_title("ڕیل — وێنەکێشان")
        .with_inner_size(LogicalSize::new(800.0, 600.0))
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_visible(false)
        .with_window_icon(window_icon.clone());
    #[cfg(windows)]
    {
        annotate_builder = annotate_builder
            .with_skip_taskbar(true)
            .with_undecorated_shadow(false);
    }
    let annotate_window = Rc::new(
        annotate_builder
            .build(&event_loop)
            .context("failed to create annotate window")?,
    );

    #[allow(unused_mut)]
    let mut toolbar_builder = WindowBuilder::new()
        .with_title("ڕیل — ئامرازەکان")
        .with_inner_size(LogicalSize::new(820.0, 72.0))
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_visible(false)
        .with_window_icon(window_icon.clone());
    #[cfg(windows)]
    {
        toolbar_builder = toolbar_builder
            .with_skip_taskbar(true)
            .with_undecorated_shadow(false);
    }
    let annotate_toolbar = Rc::new(
        toolbar_builder
            .build(&event_loop)
            .context("failed to create annotate toolbar")?,
    );

    #[allow(unused_mut)]
    let mut region_builder = WindowBuilder::new()
        .with_title("ڕیل — هەڵبژاردنی ناوچە")
        .with_inner_size(LogicalSize::new(800.0, 600.0))
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_visible(false)
        .with_window_icon(window_icon);
    #[cfg(windows)]
    {
        region_builder = region_builder
            .with_skip_taskbar(true)
            .with_undecorated_shadow(false);
    }
    let region_window = Rc::new(
        region_builder
            .build(&event_loop)
            .context("failed to create region picker window")?,
    );

    let state = Rc::new(RefCell::new(AppState::new()));
    let main_html = build_html(include_str!("../assets/ui.html"));
    let cam_html = include_str!("../assets/camera.html").to_string();
    let annotate_html = include_str!("../assets/annotate.html").to_string();
    let annotate_toolbar_html = include_str!("../assets/annotate-toolbar.html").to_string();
    let region_html = include_str!("../assets/region.html").to_string();

    let proxy = event_loop.create_proxy();
    let preview = preview::PreviewHub::start({
        let proxy = proxy.clone();
        move || {
            let _ = proxy.send_event(AppEvent::Preview);
        }
    });
    preview.set_monitor_index(state.borrow().config.monitor_index);
    {
        let st = state.borrow();
        if st.capture_mode() == CaptureMode::Region
            && st.config.region_w >= 32
            && st.config.region_h >= 32
        {
            preview.set_crop(Some(preview::PreviewCrop {
                x: st.config.region_x,
                y: st.config.region_y,
                w: st.config.region_w,
                h: st.config.region_h,
            }));
        }
    }
    let main_webview = WebViewBuilder::new(main_window.as_ref())
        .with_html(main_html)
        .with_ipc_handler({
            let state = Rc::clone(&state);
            let main_window = Rc::clone(&main_window);
            let proxy = proxy.clone();
            move |request| {
                if let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) {
                    handle_main_ipc(&state, &msg, &main_window);
                    let _ = proxy.send_event(AppEvent::Ipc);
                }
            }
        })
        .build()
        .context("failed to create webview")?;

    let cam_webview = WebViewBuilder::new(cam_window.as_ref())
        .with_html(cam_html)
        .with_transparent(true)
        .with_ipc_handler({
            let state = Rc::clone(&state);
            let proxy = proxy.clone();
            move |request| {
                if let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) {
                    handle_cam_ipc(&state, &msg);
                    let _ = proxy.send_event(AppEvent::Ipc);
                }
            }
        })
        .build()
        .context("failed to create camera webview")?;
    #[cfg(windows)]
    allow_webview_camera(&cam_webview);

    let annotate_webview = WebViewBuilder::new(annotate_window.as_ref())
        .with_html(annotate_html)
        .with_transparent(true)
        .with_ipc_handler({
            let state = Rc::clone(&state);
            let proxy = proxy.clone();
            move |request| {
                if let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) {
                    handle_annotate_ipc(&state, &msg);
                    let _ = proxy.send_event(AppEvent::Ipc);
                }
            }
        })
        .build()
        .context("failed to create annotate webview")?;

    let annotate_toolbar_webview = WebViewBuilder::new(annotate_toolbar.as_ref())
        .with_html(annotate_toolbar_html)
        .with_transparent(true)
        .with_ipc_handler({
            let state = Rc::clone(&state);
            let main_window = Rc::clone(&main_window);
            let proxy = proxy.clone();
            move |request| {
                if let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) {
                    match msg.cmd.as_str() {
                        "toggle_pause" | "stop" => {
                            handle_main_ipc(&state, &msg, main_window.as_ref());
                        }
                        _ => handle_annotate_ipc(&state, &msg),
                    }
                    let _ = proxy.send_event(AppEvent::Ipc);
                }
            }
        })
        .build()
        .context("failed to create annotate toolbar webview")?;

    let region_webview = WebViewBuilder::new(region_window.as_ref())
        .with_html(region_html)
        .with_transparent(true)
        .with_ipc_handler({
            let state = Rc::clone(&state);
            let proxy = proxy.clone();
            move |request| {
                if let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) {
                    handle_region_ipc(&state, &msg);
                    let _ = proxy.send_event(AppEvent::Ipc);
                }
            }
        })
        .build()
        .context("failed to create region picker webview")?;

    // Toolbar must stay visible to you but never appear in the recording.
    exclude_window_from_capture(annotate_toolbar.as_ref());
    exclude_window_from_capture(region_window.as_ref());

    let main_webview = Rc::new(main_webview);
    let cam_webview = Rc::new(cam_webview);
    let annotate_webview = Rc::new(annotate_webview);
    let annotate_toolbar_webview = Rc::new(annotate_toolbar_webview);
    let _region_webview = Rc::new(region_webview);
    let main_id = main_window.id();
    let cam_id = cam_window.id();
    let annotate_id = annotate_window.id();
    let annotate_toolbar_id = annotate_toolbar.id();
    let region_id = region_window.id();

    let mut hotkeys = Hotkeys::new().ok();
    if let Some(hk) = hotkeys.as_mut() {
        if let Err(e) = hk.apply(&state.borrow().config) {
            state
                .borrow_mut()
                .push_toast(format!("شۆرکەت: {e}"), true);
        }
    }

    {
        let size = state.borrow().config.camera_size as f64;
        let _ = cam_window.set_inner_size(LogicalSize::new(size, size));
    }

    push_ui(&main_webview, &state.borrow());
    state.borrow_mut().take_toast();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Some(hk) = hotkeys.as_mut() {
            if let Some(action) = hk.poll_action() {
                match action {
                    HotAction::StartStop => {
                        let recording = state.borrow().is_recording();
                        if recording {
                            let mut st = state.borrow_mut();
                            let _ = finish_stop(&mut st, &main_window);
                            st.push_toast("تۆمار وەستێنرا", false);
                        } else {
                            start_recording(&state, &main_window);
                        }
                    }
                    HotAction::ToggleMic => {
                        let mut st = state.borrow_mut();
                        if !st.is_recording() {
                            st.config.mic_enabled = !st.config.mic_enabled;
                            st.config.save();
                            let on = st.config.mic_enabled;
                            st.push_toast(
                                if on { "مایک هەڵکرا" } else { "مایک کوژایەوە" },
                                false,
                            );
                        } else {
                            st.push_toast("لە کاتی تۆماردا مایک ناگۆڕدرێت", true);
                        }
                    }
                    HotAction::ToggleCamera => {
                        let mut st = state.borrow_mut();
                        st.config.camera_enabled = !st.camera_visible;
                        st.config.save();
                        sync_camera_window(&mut st, &cam_window, &cam_webview);
                    }
                    HotAction::TogglePause => {
                        let mut st = state.borrow_mut();
                        if st.is_recording() && !st.saving {
                            st.toggle_pause();
                        }
                    }
                    HotAction::ToggleAnnotate => {
                        let mut st = state.borrow_mut();
                        if st.is_recording() && !st.saving {
                            if st.annotate_visible
                                && (st.annotate_pass || st.annotate_toolbar_hidden)
                            {
                                st.annotate_pass = false;
                                st.annotate_toolbar_hidden = false;
                                st.annotate_focus_toolbar = true;
                                st.push_toast("ئامرازەکانی وێنەکێشان", false);
                            } else if st.annotate_visible {
                                st.annotate_visible = false;
                                st.push_toast("وێنەکێشان داخرا", false);
                            } else {
                                st.annotate_visible = true;
                                st.annotate_pass = false;
                                st.annotate_toolbar_hidden = false;
                                st.annotate_focus_toolbar = true;
                                st.push_toast("قەلەم و شەیپ ئامادەن", false);
                            }
                        } else {
                            st.push_toast("لە کاتی تۆماردا وێنەکێشان بەردەستە", true);
                        }
                    }
                }
                refresh_ui(&main_webview, &state);
                sync_annotate_windows(
                    &mut state.borrow_mut(),
                    &annotate_window,
                    &annotate_webview,
                    &annotate_toolbar,
                    &annotate_toolbar_webview,
                );
            }
        }

        match event {
            Event::UserEvent(AppEvent::Preview) => {
                if let Some(frame) = preview.take() {
                    push_preview_frame(&main_webview, &frame);
                }
            }
            Event::UserEvent(AppEvent::Ipc) => {
                {
                    let mut st = state.borrow_mut();
                    preview.set_monitor_index(st.config.monitor_index);
                    preview.set_enabled(!st.saving && !st.region_picker_visible);
                    preview.set_recording(st.is_recording());
                    if st.capture_mode() == CaptureMode::Region
                        && st.config.region_w >= 32
                        && st.config.region_h >= 32
                    {
                        preview.set_crop(Some(preview::PreviewCrop {
                            x: st.config.region_x,
                            y: st.config.region_y,
                            w: st.config.region_w,
                            h: st.config.region_h,
                        }));
                    } else {
                        preview.set_crop(None);
                    }
                    if let Some((dx, dy)) = st.pending_drag.take() {
                        if let Ok(pos) = cam_window.outer_position() {
                            cam_window.set_outer_position(PhysicalPosition::new(
                                pos.x + dx as i32,
                                pos.y + dy as i32,
                            ));
                        }
                    }
                    sync_camera_window(&mut st, &cam_window, &cam_webview);
                    if st.suspend_hotkeys {
                        st.suspend_hotkeys = false;
                        if let Some(hk) = hotkeys.as_mut() {
                            hk.suspend();
                        }
                    }
                    if st.rebind_hotkeys {
                        st.rebind_hotkeys = false;
                        if let Some(hk) = hotkeys.as_mut() {
                            match hk.apply(&st.config) {
                                Ok(()) => st.push_toast("شۆرکەتەکان چالاک کران", false),
                                Err(e) => st.push_toast(format!("شۆرکەت: {e}"), true),
                            }
                        }
                    }
                    if st.session.as_ref().is_some_and(|s| s.is_finished()) {
                        let _ = finish_stop(&mut st, &main_window);
                    }
                }
                sync_annotate_windows(
                    &mut state.borrow_mut(),
                    &annotate_window,
                    &annotate_webview,
                    &annotate_toolbar,
                    &annotate_toolbar_webview,
                );
                sync_region_picker(
                    &mut state.borrow_mut(),
                    &region_window,
                    &main_window,
                );
                if state.borrow().camera_visible {
                    cam_window.set_always_on_top(true);
                }
                // Keep toolbar above canvas so it stays clickable.
                if state.borrow().annotate_visible && !state.borrow().annotate_toolbar_hidden {
                    annotate_toolbar.set_always_on_top(true);
                }
                if state.borrow().region_picker_visible {
                    region_window.set_always_on_top(true);
                }
                refresh_ui(&main_webview, &state);
                push_cam_settings(&cam_webview, &state.borrow(), false);
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if window_id == cam_id {
                    let mut st = state.borrow_mut();
                    st.camera_visible = false;
                    st.config.camera_enabled = false;
                    st.config.save();
                    cam_window.set_visible(false);
                    refresh_ui(&main_webview, &state);
                } else if window_id == annotate_id || window_id == annotate_toolbar_id {
                    let mut st = state.borrow_mut();
                    st.annotate_visible = false;
                    annotate_window.set_visible(false);
                    annotate_toolbar.set_visible(false);
                    let _ = annotate_window.set_ignore_cursor_events(false);
                    refresh_ui(&main_webview, &state);
                } else if window_id == region_id {
                    let mut st = state.borrow_mut();
                    st.region_picker_visible = false;
                    region_window.set_visible(false);
                    main_window.set_visible(true);
                    refresh_ui(&main_webview, &state);
                } else if window_id == main_id {
                    let mut st = state.borrow_mut();
                    if st.is_recording() {
                        let _ = finish_stop(&mut st, &main_window);
                    }
                    st.annotate_visible = false;
                    st.region_picker_visible = false;
                    annotate_window.set_visible(false);
                    annotate_toolbar.set_visible(false);
                    region_window.set_visible(false);
                    st.mic = None;
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

fn refresh_ui(webview: &wry::WebView, state: &Rc<RefCell<AppState>>) {
    push_ui(webview, &state.borrow());
    state.borrow_mut().take_toast();
}

fn sync_camera_window(st: &mut AppState, cam_window: &Window, cam_webview: &wry::WebView) {
    let want = st.config.camera_enabled;
    let mut just_opened = false;
    if want && !st.camera_visible {
        let size = st.config.camera_size as f64;
        let _ = cam_window.set_inner_size(LogicalSize::new(size, size));
        cam_window.set_visible(true);
        cam_window.set_always_on_top(true);
        st.camera_visible = true;
        just_opened = true;
        st.push_toast("پۆپاپی کامێرا کرایەوە", false);
    } else if !want && st.camera_visible {
        cam_window.set_visible(false);
        st.camera_visible = false;
        st.push_toast("پۆپاپی کامێرا داخرا", false);
        let _ = cam_webview.evaluate_script(
            "window.__camStop && window.__camStop();",
        );
    } else if st.camera_visible {
        let size = st.config.camera_size as f64;
        let _ = cam_window.set_inner_size(LogicalSize::new(size, size));
    }
    if st.camera_visible {
        push_cam_settings(cam_webview, st, just_opened);
    }
}

fn push_cam_settings(webview: &wry::WebView, st: &AppState, force_start: bool) {
    let payload = json!({
        "greenscreen": st.config.greenscreen,
        "strength": st.config.greenscreen_strength,
        "softness": st.config.greenscreen_softness,
        "color": st.config.greenscreen_color,
        "shape": st.config.camera_shape,
        "filter": st.config.camera_filter,
        "deviceId": st.config.camera_device_id,
        "start": force_start,
    });
    let s = payload.to_string().replace('\\', "\\\\").replace('\'', "\\'");
    let _ = webview.evaluate_script(&format!(
        "window.__camUpdate && window.__camUpdate(JSON.parse('{s}'));"
    ));
}

/// WebView2 denies camera/mic by default unless PermissionRequested is handled.
#[cfg(windows)]
fn allow_webview_camera(webview: &wry::WebView) {
    use webview2_com::{
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_PERMISSION_KIND_CAMERA, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
            COREWEBVIEW2_PERMISSION_STATE_ALLOW,
        },
        PermissionRequestedEventHandler,
    };

    let controller = webview.controller();
    let Ok(core) = (unsafe { controller.CoreWebView2() }) else {
        return;
    };
    let mut token = Default::default();
    let _ = unsafe {
        core.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut kind = Default::default();
                args.PermissionKind(&mut kind)?;
                if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA
                    || kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                {
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                }
                Ok(())
            })),
            &mut token,
        )
    };
}

fn monitor_geom(window: &Window, index: usize) -> (i32, i32, u32, u32) {
    let monitors: Vec<_> = window.available_monitors().collect();
    if let Some(m) = monitors.get(index) {
        let p = m.position();
        let s = m.size();
        return (p.x, p.y, s.width, s.height);
    }
    if let Some(m) = window.primary_monitor() {
        let p = m.position();
        let s = m.size();
        return (p.x, p.y, s.width, s.height);
    }
    (0, 0, 1920, 1080)
}

fn push_preview_frame(webview: &wry::WebView, frame: &preview::PreviewFrame) {
    // Fast path: data URLs are base64-safe — skip JSON stringify/escape overhead.
    let _ = webview.evaluate_script(&format!(
        "window.__previewFrame&&window.__previewFrame({{url:'{}',w:{},h:{}}})",
        frame.data_url, frame.width, frame.height
    ));
}

fn eval_json(webview: &wry::WebView, fn_name: &str, payload: &serde_json::Value) {
    let s = payload.to_string().replace('\\', "\\\\").replace('\'', "\\'");
    let _ = webview.evaluate_script(&format!(
        "window.{fn_name} && window.{fn_name}(JSON.parse('{s}'));"
    ));
}

fn exclude_window_from_capture(window: &Window) {
    #[cfg(windows)]
    {
        let hwnd = window.hwnd();
        unsafe {
            win_capture_exclude::SetWindowDisplayAffinity(
                hwnd,
                win_capture_exclude::WDA_EXCLUDEFROMCAPTURE,
            );
        }
    }
}

fn sync_annotate_windows(
    st: &mut AppState,
    canvas: &Window,
    canvas_webview: &wry::WebView,
    toolbar: &Window,
    toolbar_webview: &wry::WebView,
) {
    if st.annotate_visible && st.is_recording() && !st.saving {
        let (x, y, w, h) = monitor_geom(canvas, st.config.monitor_index);

        // Floating toolbar — visible to you, excluded from capture.
        let tw = 820i32;
        let th = 72i32;
        let gap = 16i32;
        let band = th + gap + 8;
        let tx = x + (w as i32 - tw) / 2;
        let ty = y + h as i32 - th - gap;

        // Keep canvas ABOVE the toolbar band so pen mode cannot steal toolbar clicks.
        let canvas_h = if st.annotate_toolbar_hidden {
            h
        } else {
            h.saturating_sub(band as u32).max(120)
        };
        canvas.set_outer_position(PhysicalPosition::new(x, y));
        let _ = canvas.set_inner_size(PhysicalSize::new(w, canvas_h));
        canvas.set_always_on_top(true);
        canvas.set_visible(true);
        let _ = canvas.set_ignore_cursor_events(st.annotate_pass);

        toolbar.set_outer_position(PhysicalPosition::new(tx, ty));
        let _ = toolbar.set_inner_size(PhysicalSize::new(tw as u32, th as u32));
        exclude_window_from_capture(toolbar);
        toolbar.set_visible(!st.annotate_toolbar_hidden);
        let _ = toolbar.set_ignore_cursor_events(false);
        // Re-assert topmost AFTER canvas so hit-testing prefers the toolbar.
        toolbar.set_always_on_top(true);

        let clear = st.annotate_clear;
        let focus = st.annotate_focus_toolbar;
        let cmd = st.annotate_cmd.take();
        st.annotate_clear = false;
        st.annotate_focus_toolbar = false;

        let paused = st.session.as_ref().is_some_and(|s| s.is_paused());
        if clear {
            eval_json(canvas_webview, "__annotateUpdate", &json!({ "clear": true }));
        }
        if focus {
            eval_json(
                canvas_webview,
                "__annotateUpdate",
                &json!({ "showToolbar": true, "tool": "pen" }),
            );
            eval_json(
                toolbar_webview,
                "__toolbarUpdate",
                &json!({ "showToolbar": true, "tool": "pen", "paused": paused }),
            );
        }
        if let Some(cmd) = cmd {
            eval_json(canvas_webview, "__annotateUpdate", &cmd);
            // Keep toolbar highlight in sync (without forcing pen).
            if cmd.get("tool").is_some() {
                eval_json(toolbar_webview, "__toolbarSync", &cmd);
            }
        }
        eval_json(
            toolbar_webview,
            "__toolbarSync",
            &json!({ "paused": paused }),
        );
    } else {
        if st.annotate_visible && !st.is_recording() {
            st.annotate_visible = false;
            st.annotate_clear = true;
        }
        let clear = st.annotate_clear;
        st.annotate_clear = false;
        st.annotate_focus_toolbar = false;
        st.annotate_pass = false;
        st.annotate_toolbar_hidden = false;
        st.annotate_cmd = None;
        canvas.set_visible(false);
        toolbar.set_visible(false);
        let _ = canvas.set_ignore_cursor_events(false);
        if clear {
            eval_json(canvas_webview, "__annotateUpdate", &json!({ "clear": true }));
        }
    }
}

fn normalize_hex_color(s: &str) -> String {
    let t = s.trim();
    if t.len() == 7 && t.starts_with('#') && t[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        t.to_ascii_lowercase()
    } else {
        "#22c55e".into()
    }
}

/// Keep the recorder visible to you, but exclude it from the recorded video.
fn prepare_recorder_for_capture(main_window: &Window) {
    #[cfg(windows)]
    {
        let hwnd = main_window.hwnd();
        unsafe {
            win_capture_exclude::SetWindowDisplayAffinity(
                hwnd,
                win_capture_exclude::WDA_EXCLUDEFROMCAPTURE,
            );
        }
    }
    // Stay on top so you can still control/pause while recording.
    main_window.set_always_on_top(true);
}

fn restore_recorder_after_capture(main_window: &Window) {
    #[cfg(windows)]
    {
        let hwnd = main_window.hwnd();
        unsafe {
            win_capture_exclude::SetWindowDisplayAffinity(hwnd, win_capture_exclude::WDA_NONE);
        }
    }
    main_window.set_always_on_top(false);
    main_window.set_visible(true);
    let _ = main_window.set_focus();
}

fn sync_region_picker(st: &mut AppState, region_window: &Window, main_window: &Window) {
    if st.region_picker_visible && !st.is_recording() && !st.saving {
        let (x, y, w, h) = monitor_geom(main_window, st.config.monitor_index);
        region_window.set_outer_position(PhysicalPosition::new(x, y));
        let _ = region_window.set_inner_size(PhysicalSize::new(w, h));
        region_window.set_always_on_top(true);
        region_window.set_visible(true);
        main_window.set_visible(false);
        let _ = region_window.set_focus();
    } else if region_window.is_visible() {
        st.region_picker_visible = false;
        region_window.set_visible(false);
        if !st.is_recording() {
            main_window.set_visible(true);
        }
    }
}

fn start_recording(state: &Rc<RefCell<AppState>>, main_window: &Window) {
    let mut st = state.borrow_mut();
    if st.is_recording() || st.saving || st.region_picker_visible {
        return;
    }
    st.error = None;

    let mode = st.capture_mode();
    if mode == CaptureMode::Region
        && (st.config.region_w < 32 || st.config.region_h < 32)
    {
        st.error = Some("سەرەتا ناوچەیەک هەڵبژێرە".into());
        st.push_toast("ناوچە دیاری نەکرا", true);
        return;
    }
    if mode == CaptureMode::Game
        && st.config.game_hwnd.trim().is_empty()
        && st.config.game_title.trim().is_empty()
    {
        st.error = Some("سەرەتا پەنجەرەی گەیم هەڵبژێرە".into());
        st.push_toast("پەنجەرەی گەیم دیاری نەکرا", true);
        return;
    }

    let dual = mode == CaptureMode::Dual && st.monitors.len() >= 2;
    let idx_a = st.config.monitor_index;
    let idx_b = st.config.monitor_index_b;
    if dual && idx_a == idx_b {
        st.error = Some("دوو مۆنیتەری جیاواز هەڵبژێرە".into());
        st.push_toast("مۆنیتەرەکان یەکسانن", true);
        return;
    }

    let mut config_a = st.record_config();
    if dual {
        config_a.mode = CaptureMode::Dual;
    }
    let mic_id = st.config.mic_device_id.clone();
    let noise_cfg = NoiseReduceConfig {
        enabled: st.config.noise_reduce,
        strength: st.config.noise_strength,
    };
    let mic_buffer = if config_a.mic_enabled {
        match MicCapture::start(
            if mic_id.is_empty() {
                None
            } else {
                Some(mic_id.as_str())
            },
            noise_cfg,
        ) {
            Ok(mic) => {
                let label = mic.device_label.clone();
                let buf = mic.buffer.clone();
                buf.clear();
                // Warm up a moment so WASAPI starts delivering samples.
                std::thread::sleep(std::time::Duration::from_millis(120));
                st.mic = Some(mic);
                let noise_note = if noise_cfg.enabled {
                    format!(" · سڕینەوەی noise ({})", noise_cfg.strength)
                } else {
                    String::new()
                };
                st.push_toast(format!("مایک: {label}{noise_note}"), false);
                Some(buf)
            }
            Err(e) => {
                st.mic = None;
                config_a.mic_enabled = false;
                st.push_toast(
                    format!("مایک بەردەست نییە — بێدەنگ تۆمار دەکرێت ({e})"),
                    true,
                );
                None
            }
        }
    } else {
        st.mic = None;
        None
    };

    let count = if dual { 2 } else { 1 };
    let container = st.record_config().container;
    let paths = match recorder::output_paths(count, container) {
        Ok(p) => p,
        Err(e) => {
            st.mic = None;
            st.error = Some(simplify_error(&e.to_string()));
            st.status = "نەتوانرا ڕێگای پاشەکەوت دروست بکرێت".into();
            st.push_toast("ڕێگای پاشەکەوت سەرنەکەوت", true);
            return;
        }
    };

    // Visible to you, excluded from the screen recording.
    drop(st);
    prepare_recorder_for_capture(main_window);
    let mut st = state.borrow_mut();

    let mut captures = Vec::new();

    match recorder::ActiveCapture::start(config_a, paths[0].clone(), mic_buffer) {
        Ok(c) => captures.push(c),
        Err(e) => {
            st.mic = None;
            st.error = Some(simplify_error(&e.to_string()));
            st.status = "نەتوانرا تۆمار دەست پێبکرێت".into();
            st.push_toast("تۆمار سەرنەکەوت", true);
            drop(st);
            restore_recorder_after_capture(main_window);
            return;
        }
    }

    if dual {
        let mut config_b = st.record_config();
        config_b.monitor_index = idx_b;
        config_b.mic_enabled = false; // mic only on primary stream
        match recorder::ActiveCapture::start(config_b, paths[1].clone(), None) {
            Ok(c) => captures.push(c),
            Err(e) => {
                // Roll back first capture
                let _ = captures.pop().map(|c| c.stop());
                st.mic = None;
                st.error = Some(simplify_error(&e.to_string()));
                st.status = "نەتوانرا سکرینی دووەم تۆمار بکرێت".into();
                st.push_toast("تۆماری دوو سکرین سەرنەکەوت", true);
                drop(st);
                restore_recorder_after_capture(main_window);
                return;
            }
        }
    }

    let session = RecordingSession { captures };
    let summary = session.summary_ckb();
    let res: String = session
        .captures
        .iter()
        .map(|c| format!("{}×{}", c.width, c.height))
        .collect::<Vec<_>>()
        .join(" + ");
    let names: String = session
        .captures
        .iter()
        .map(|c| file_name(&c.output))
        .collect::<Vec<_>>()
        .join(" · ");
    st.status = format!("خەریکی تۆمارکردنە · {summary} · {res} · {names}");
    // Don't auto-open pen/toolbar — user enables it via hotkey or the annotate button.
    st.annotate_visible = false;
    st.annotate_pass = false;
    st.annotate_toolbar_hidden = false;
    st.annotate_focus_toolbar = false;
    st.annotate_clear = false;
    st.push_toast(
        "تۆمار دەستی پێکرد — تۆ دەیبینیت، لە ڤیدیۆدا دەرناچێت",
        false,
    );
    st.session = Some(session);
}

fn handle_main_ipc(state: &Rc<RefCell<AppState>>, msg: &IpcMsg, main_window: &Window) {
    match msg.cmd.as_str() {
        "ready" | "tick" => {}
        "settings" => {
            let mut st = state.borrow_mut();
            if !st.is_recording() && !st.saving {
                st.apply_ipc_settings(msg);
            } else {
                st.apply_camera_visuals(msg);
            }
        }
        "start" => {
            {
                let mut st = state.borrow_mut();
                if !st.is_recording() && !st.saving {
                    st.apply_ipc_settings(msg);
                }
            }
            start_recording(state, main_window);
        }
        "stop" => {
            let mut st = state.borrow_mut();
            if st.is_recording() {
                let _ = finish_stop(&mut st, main_window);
                st.push_toast("تۆمار وەستێنرا", false);
            }
        }
        "toggle_pause" => {
            let mut st = state.borrow_mut();
            if st.is_recording() && !st.saving {
                st.toggle_pause();
            }
        }
        "camera_toggle" => {
            let mut st = state.borrow_mut();
            st.apply_camera_visuals(msg);
            st.config.camera_enabled = !st.camera_visible;
            st.config.save();
        }
        "annotate_toggle" => {
            let mut st = state.borrow_mut();
            if let Some(a) = msg.annotate {
                st.config.annotate_enabled = a;
                st.config.save();
            }
            if st.is_recording() && !st.saving {
                st.annotate_visible = !st.annotate_visible;
                if st.annotate_visible {
                    st.annotate_pass = false;
                    st.annotate_toolbar_hidden = false;
                    st.annotate_focus_toolbar = true;
                    st.push_toast("قەلەم و شەیپ ئامادەن", false);
                } else {
                    st.push_toast("وێنەکێشان داخرا", false);
                }
            }
        }
        "save_hotkeys" => {
            let mut st = state.borrow_mut();
            st.apply_ipc_settings(msg);
            st.suspend_hotkeys = false;
            st.rebind_hotkeys = true;
        }
        "hotkeys_suspend" => {
            let mut st = state.borrow_mut();
            st.rebind_hotkeys = false;
            st.suspend_hotkeys = true;
        }
        "hotkeys_resume" => {
            let mut st = state.borrow_mut();
            st.suspend_hotkeys = false;
            st.rebind_hotkeys = true;
        }
        "open_file" => {
            let st = state.borrow();
            if let Some(path) = st.last_paths.first() {
                let _ = open::that(path);
            }
        }
        "open_folder" => {
            let st = state.borrow();
            if let Some(path) = st.last_paths.first() {
                if let Some(parent) = path.parent() {
                    let _ = open::that(parent);
                }
            }
        }
        "pick_region" => {
            let mut st = state.borrow_mut();
            if st.is_recording() || st.saving {
                return;
            }
            st.apply_ipc_settings(msg);
            st.config.capture_mode = CaptureMode::Region.as_str().into();
            st.config.dual_screen = false;
            st.config.save();
            st.region_picker_visible = true;
            st.push_toast("ناوچەیەک بکێشە لەسەر شاشەکە", false);
        }
        "refresh_windows" => {
            let mut st = state.borrow_mut();
            st.refresh_game_windows();
            st.push_toast("لیستی پەنجەرەکان نوێکرایەوە", false);
        }
        _ => {}
    }
}

fn handle_region_ipc(state: &Rc<RefCell<AppState>>, msg: &IpcMsg) {
    match msg.cmd.as_str() {
        "region_ready" => {}
        "region_done" => {
            let mut st = state.borrow_mut();
            if let (Some(x), Some(y), Some(w), Some(h)) =
                (msg.region_x, msg.region_y, msg.region_w, msg.region_h)
            {
                st.config.region_x = x & !1;
                st.config.region_y = y & !1;
                st.config.region_w = w.max(2) & !1;
                st.config.region_h = h.max(2) & !1;
                st.config.capture_mode = CaptureMode::Region.as_str().into();
                st.config.dual_screen = false;
                st.config.save();
                let rw = st.config.region_w;
                let rh = st.config.region_h;
                st.push_toast(format!("ناوچە: {rw}×{rh}"), false);
            }
            st.region_picker_visible = false;
        }
        "region_cancel" => {
            let mut st = state.borrow_mut();
            st.region_picker_visible = false;
            st.push_toast("هەڵبژاردنی ناوچە هەڵوەشایەوە", false);
        }
        _ => {}
    }
}

fn handle_annotate_ipc(state: &Rc<RefCell<AppState>>, msg: &IpcMsg) {
    match msg.cmd.as_str() {
        "annotate_ready" | "annotate_toolbar_ready" => {}
        "annotate_pass" => {
            let mut st = state.borrow_mut();
            st.annotate_pass = msg.pass.unwrap_or(false);
        }
        "annotate_cmd" => {
            let mut st = state.borrow_mut();
            let mut payload = json!({});
            if let Some(tool) = &msg.tool {
                payload["tool"] = json!(tool);
                st.annotate_pass = tool == "pointer";
                if tool != "pointer" {
                    st.annotate_toolbar_hidden = false;
                }
            }
            if let Some(color) = &msg.color {
                payload["color"] = json!(color);
            }
            if let Some(size) = msg.size {
                payload["size"] = json!(size);
            }
            if let Some(action) = &msg.action {
                payload["action"] = json!(action);
                if action == "hide_toolbar" {
                    st.annotate_toolbar_hidden = true;
                    st.annotate_pass = true;
                }
            }
            st.annotate_cmd = Some(payload);
        }
        _ => {}
    }
}

fn handle_cam_ipc(state: &Rc<RefCell<AppState>>, msg: &IpcMsg) {
    match msg.cmd.as_str() {
        "cam_ready" => {}
        "cam_devices" => {
            let mut st = state.borrow_mut();
            if let Some(list) = &msg.devices {
                st.camera_devices = list
                    .iter()
                    .map(|d| {
                        let label = if d.label.trim().is_empty() {
                            format!("کامێرا {}", &d.id[..d.id.len().min(8)])
                        } else {
                            d.label.clone()
                        };
                        (d.id.clone(), label)
                    })
                    .collect();
            }
            if let Some(cur) = &msg.current {
                if !cur.is_empty() && st.config.camera_device_id.is_empty() {
                    st.config.camera_device_id = cur.clone();
                    st.config.save();
                }
            }
        }
        "cam_drag" => {
            if let (Some(dx), Some(dy)) = (msg.dx, msg.dy) {
                state.borrow_mut().pending_drag = Some((dx, dy));
            }
        }
        _ => {}
    }
}

fn finish_stop(st: &mut AppState, main_window: &Window) -> Result<()> {
    if let Some(session) = st.session.take() {
        st.saving = true;
        st.status = "خەریکی پاشەکەوتکردنی ڤیدیۆ…".into();
        let dual = session.captures.len() > 1;
        match session.stop() {
            Ok(paths) => {
                let size: f64 = paths
                    .iter()
                    .filter_map(|p| std::fs::metadata(p).ok())
                    .map(|m| m.len() as f64 / 1_048_576.0)
                    .sum();
                let n = paths.len();
                st.status = if n > 1 {
                    format!("پاشەکەوت کرا · {n} فایل · {size:.1} مێگابایت")
                } else {
                    format!("پاشەکەوت کرا · {size:.1} مێگابایت")
                };
                st.last_paths = paths;
                st.error = None;
                st.push_toast(
                    if dual {
                        "هەردوو ڤیدیۆ پاشەکەوت کران"
                    } else {
                        "ڤیدیۆ پاشەکەوت کرا"
                    },
                    false,
                );
            }
            Err(e) => {
                st.error = Some(simplify_error(&e.to_string()));
                st.status = "وەستاندن سەرنەکەوت".into();
                st.push_toast("وەستاندن سەرنەکەوت", true);
            }
        }
        st.saving = false;
    }
    st.mic = None;
    st.annotate_visible = false;
    st.annotate_pass = false;
    st.annotate_toolbar_hidden = false;
    st.annotate_clear = true;
    st.annotate_cmd = None;
    restore_recorder_after_capture(main_window);
    Ok(())
}

fn build_html(template: &str) -> String {
    let font_b64 = base64_encode(include_bytes!("../assets/fonts/Rabar.ttf"));
    template.replace(
        "url(\"app://fonts/Rabar.ttf\") format(\"truetype\")",
        &format!("url(\"data:font/ttf;base64,{font_b64}\") format(\"truetype\")"),
    )
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (a << 16) | (b << 8) | c;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn push_ui(webview: &wry::WebView, state: &AppState) {
    let payload = state.snapshot().to_string();
    let escaped = payload
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "");
    let script = format!("window.__reelUpdate && window.__reelUpdate(JSON.parse('{escaped}'));");
    let _ = webview.evaluate_script(&script);
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("recording.mp4")
        .to_string()
}

fn simplify_error(err: &str) -> String {
    if err.contains("monitor") {
        "مۆنیتەر نەدۆزرایەوە".into()
    } else if err.contains("encoder") || err.contains("encode") {
        "کۆدکەری ڤیدیۆ سەرنەکەوت — کۆدێک یان کوالیتی بگۆڕە".into()
    } else if err.contains("create") {
        "نەتوانرا فایل دروست بکرێت".into()
    } else {
        err.to_string()
    }
}
