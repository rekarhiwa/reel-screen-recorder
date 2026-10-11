#![windows_subsystem = "windows"]

mod audio;
mod config;
mod hotkeys;
mod icon_data;
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
mod win_capture_exclude {
    /// Visible on your screen, but omitted from screen capture / recordings.
    pub const WDA_NONE: u32 = 0;
    pub const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;

    #[link(name = "user32")]
    extern "system" {
        pub fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
    }
}

use audio::MicCapture;
use config::AppConfig;
use hotkeys::{HotAction, Hotkeys};
use recorder::{RecordConfig, RecordingSession};

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
    cursor: bool,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    fps: Option<u32>,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    mic: Option<bool>,
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
    config: AppConfig,
    camera_visible: bool,
    camera_devices: Vec<(String, String)>,
    annotate_visible: bool,
    annotate_pass: bool,
    annotate_focus_toolbar: bool,
    annotate_clear: bool,
    rebind_hotkeys: bool,
    pending_drag: Option<(f64, f64)>,
}

impl AppState {
    fn new() -> Self {
        let infos = recorder::list_monitors().unwrap_or_default();
        let selected = infos.iter().position(|m| m.is_primary).unwrap_or(0);
        let monitors: Vec<String> = infos.into_iter().map(|m| m.label).collect();
        let mut config = AppConfig::load();
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
        }
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
            config,
            camera_visible: false,
            camera_devices: Vec::new(),
            annotate_visible: false,
            annotate_pass: false,
            annotate_focus_toolbar: false,
            annotate_clear: false,
            rebind_hotkeys: false,
            pending_drag: None,
        }
    }

    fn is_recording(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.is_recording())
    }

    fn apply_ipc_settings(&mut self, msg: &IpcMsg) {
        self.config.monitor_index = msg.monitor;
        if let Some(b) = msg.monitor_b {
            self.config.monitor_index_b = b;
        }
        if let Some(d) = msg.dual {
            self.config.dual_screen = d && self.monitors.len() >= 2;
        }
        if self.config.dual_screen && self.config.monitor_index_b == self.config.monitor_index {
            self.config.monitor_index_b = (0..self.monitors.len())
                .find(|&i| i != self.config.monitor_index)
                .unwrap_or(self.config.monitor_index);
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
        if let Some(m) = msg.mic {
            self.config.mic_enabled = m;
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
        if self.config.fps != 24 && self.config.fps != 30 && self.config.fps != 60 {
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
            self.status = "تۆمار وەستێنرا (پۆز) — دەتوانیت بەردەوام بیت".into();
            self.push_toast("پۆز", false);
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
        RecordConfig {
            monitor_index: self.config.monitor_index,
            show_cursor: self.config.show_cursor,
            quality: self.config.quality(),
            fps: self.config.fps,
            codec: self.config.codec(),
            mic_enabled: self.config.mic_enabled,
        }
    }

    fn snapshot(&self) -> serde_json::Value {
        let (elapsed, frames, recording, paused) = if let Some(s) = &self.session {
            (s.elapsed_secs(), s.frame_count(), true, s.is_paused())
        } else {
            (0, 0, false, false)
        };

        let dual_label = if self.config.dual_screen && self.monitors.len() >= 2 {
            "دوو سکرین"
        } else {
            "یەک سکرین"
        };
        let summary = format!("{} · {}", self.record_config().summary_ckb(), dual_label);

        json!({
            "monitors": self.monitors,
            "selected": self.config.monitor_index,
            "selectedB": self.config.monitor_index_b,
            "dual": self.config.dual_screen && self.monitors.len() >= 2,
            "dualAvailable": self.monitors.len() >= 2,
            "cursor": self.config.show_cursor,
            "quality": self.config.quality,
            "fps": self.config.fps,
            "codec": self.config.codec,
            "mic": self.config.mic_enabled,
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
    let event_loop = EventLoopBuilder::<()>::with_user_event().build();
    let window_icon = tao::window::Icon::from_rgba(
        icon_data::RGBA.to_vec(),
        icon_data::WIDTH,
        icon_data::HEIGHT,
    )
    .ok();

    let main_window = Rc::new(
        WindowBuilder::new()
            .with_title("ڕیل — سکرین ڕیکۆردەر")
            .with_inner_size(LogicalSize::new(520.0, 520.0))
            .with_min_inner_size(LogicalSize::new(520.0, 520.0))
            .with_resizable(false)
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
        .with_window_icon(window_icon);
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

    let state = Rc::new(RefCell::new(AppState::new()));
    let main_html = build_html(include_str!("../assets/ui.html"));
    let cam_html = include_str!("../assets/camera.html").to_string();
    let annotate_html = include_str!("../assets/annotate.html").to_string();

    let proxy = event_loop.create_proxy();
    let main_webview = WebViewBuilder::new(main_window.as_ref())
        .with_html(main_html)
        .with_ipc_handler({
            let state = Rc::clone(&state);
            let main_window = Rc::clone(&main_window);
            let proxy = proxy.clone();
            move |request| {
                if let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) {
                    handle_main_ipc(&state, &msg, &main_window);
                    let _ = proxy.send_event(());
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
                    let _ = proxy.send_event(());
                }
            }
        })
        .build()
        .context("failed to create camera webview")?;

    let annotate_webview = WebViewBuilder::new(annotate_window.as_ref())
        .with_html(annotate_html)
        .with_transparent(true)
        .with_ipc_handler({
            let state = Rc::clone(&state);
            let proxy = proxy.clone();
            move |request| {
                if let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) {
                    handle_annotate_ipc(&state, &msg);
                    let _ = proxy.send_event(());
                }
            }
        })
        .build()
        .context("failed to create annotate webview")?;

    let main_webview = Rc::new(main_webview);
    let cam_webview = Rc::new(cam_webview);
    let annotate_webview = Rc::new(annotate_webview);
    let main_id = main_window.id();
    let cam_id = cam_window.id();
    let annotate_id = annotate_window.id();

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
                            if st.annotate_visible && st.annotate_pass {
                                st.annotate_pass = false;
                                st.annotate_focus_toolbar = true;
                                st.push_toast("ئامرازەکانی وێنەکێشان", false);
                            } else if st.annotate_visible {
                                st.annotate_visible = false;
                                st.push_toast("وێنەکێشان داخرا", false);
                            } else {
                                st.annotate_visible = true;
                                st.annotate_pass = false;
                                st.annotate_focus_toolbar = true;
                                st.push_toast("قەلەم و شەیپ ئامادەن", false);
                            }
                        } else {
                            st.push_toast("لە کاتی تۆماردا وێنەکێشان بەردەستە", true);
                        }
                    }
                }
                refresh_ui(&main_webview, &state);
                sync_annotate_window(
                    &mut state.borrow_mut(),
                    &annotate_window,
                    &annotate_webview,
                );
            }
        }

        match event {
            Event::UserEvent(()) => {
                {
                    let mut st = state.borrow_mut();
                    if let Some((dx, dy)) = st.pending_drag.take() {
                        if let Ok(pos) = cam_window.outer_position() {
                            cam_window.set_outer_position(PhysicalPosition::new(
                                pos.x + dx as i32,
                                pos.y + dy as i32,
                            ));
                        }
                    }
                    sync_camera_window(&mut st, &cam_window, &cam_webview);
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
                sync_annotate_window(
                    &mut state.borrow_mut(),
                    &annotate_window,
                    &annotate_webview,
                );
                if state.borrow().camera_visible {
                    cam_window.set_always_on_top(true);
                }
                refresh_ui(&main_webview, &state);
                push_cam_settings(&cam_webview, &state.borrow());
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
                } else if window_id == annotate_id {
                    let mut st = state.borrow_mut();
                    st.annotate_visible = false;
                    annotate_window.set_visible(false);
                    let _ = annotate_window.set_ignore_cursor_events(false);
                    refresh_ui(&main_webview, &state);
                } else if window_id == main_id {
                    let mut st = state.borrow_mut();
                    if st.is_recording() {
                        let _ = finish_stop(&mut st, &main_window);
                    }
                    st.annotate_visible = false;
                    annotate_window.set_visible(false);
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
    if want && !st.camera_visible {
        let size = st.config.camera_size as f64;
        let _ = cam_window.set_inner_size(LogicalSize::new(size, size));
        cam_window.set_visible(true);
        cam_window.set_always_on_top(true);
        st.camera_visible = true;
        st.push_toast("پۆپاپی کامێرا کرایەوە", false);
    } else if !want && st.camera_visible {
        cam_window.set_visible(false);
        st.camera_visible = false;
        st.push_toast("پۆپاپی کامێرا داخرا", false);
    } else if st.camera_visible {
        let size = st.config.camera_size as f64;
        let _ = cam_window.set_inner_size(LogicalSize::new(size, size));
    }
    if st.camera_visible {
        push_cam_settings(cam_webview, st);
    }
}

fn push_cam_settings(webview: &wry::WebView, st: &AppState) {
    let payload = json!({
        "greenscreen": st.config.greenscreen,
        "strength": st.config.greenscreen_strength,
        "softness": st.config.greenscreen_softness,
        "color": st.config.greenscreen_color,
        "shape": st.config.camera_shape,
        "filter": st.config.camera_filter,
        "deviceId": st.config.camera_device_id,
    });
    let s = payload.to_string().replace('\\', "\\\\").replace('\'', "\\'");
    let _ = webview.evaluate_script(&format!(
        "window.__camUpdate && window.__camUpdate(JSON.parse('{s}'));"
    ));
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

fn push_annotate(webview: &wry::WebView, clear: bool, show_toolbar: bool) {
    let payload = json!({
        "clear": clear,
        "showToolbar": show_toolbar,
    });
    let s = payload.to_string().replace('\\', "\\\\").replace('\'', "\\'");
    let _ = webview.evaluate_script(&format!(
        "window.__annotateUpdate && window.__annotateUpdate(JSON.parse('{s}'));"
    ));
}

fn sync_annotate_window(st: &mut AppState, window: &Window, webview: &wry::WebView) {
    if st.annotate_visible && st.is_recording() && !st.saving {
        let (x, y, w, h) = monitor_geom(window, st.config.monitor_index);
        window.set_outer_position(PhysicalPosition::new(x, y));
        let _ = window.set_inner_size(PhysicalSize::new(w, h));
        window.set_always_on_top(true);
        window.set_visible(true);
        let _ = window.set_ignore_cursor_events(st.annotate_pass);
        let clear = st.annotate_clear;
        let focus = st.annotate_focus_toolbar;
        st.annotate_clear = false;
        st.annotate_focus_toolbar = false;
        if clear || focus {
            push_annotate(webview, clear, focus);
        }
    } else {
        if st.annotate_visible && !st.is_recording() {
            st.annotate_visible = false;
            st.annotate_clear = true;
        }
        let clear = st.annotate_clear;
        st.annotate_clear = false;
        st.annotate_focus_toolbar = false;
        st.annotate_pass = false;
        window.set_visible(false);
        let _ = window.set_ignore_cursor_events(false);
        if clear {
            push_annotate(webview, true, false);
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

fn start_recording(state: &Rc<RefCell<AppState>>, main_window: &Window) {
    let mut st = state.borrow_mut();
    if st.is_recording() || st.saving {
        return;
    }
    st.error = None;

    let dual = st.config.dual_screen && st.monitors.len() >= 2;
    let idx_a = st.config.monitor_index;
    let idx_b = st.config.monitor_index_b;
    if dual && idx_a == idx_b {
        st.error = Some("دوو مۆنیتەری جیاواز هەڵبژێرە".into());
        st.push_toast("مۆنیتەرەکان یەکسانن", true);
        return;
    }

    let mut config_a = st.record_config();
    let mic_buffer = if config_a.mic_enabled {
        match MicCapture::start() {
            Ok(mic) => {
                let buf = mic.buffer.clone();
                buf.clear();
                st.mic = Some(mic);
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
    let paths = match recorder::output_paths(count) {
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
    if st.config.annotate_enabled {
        st.annotate_visible = true;
        st.annotate_pass = false;
        st.annotate_focus_toolbar = true;
        st.annotate_clear = true;
        st.push_toast(
            "تۆمار دەستی پێکرد — قەلەم/شەیپ ئامادەن (Ctrl+Shift+D)",
            false,
        );
    } else {
        st.push_toast(
            "تۆمار دەستی پێکرد — تۆ دەیبینیت، لە ڤیدیۆدا دەرناچێت",
            false,
        );
    }
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
        _ => {}
    }
}

fn handle_annotate_ipc(state: &Rc<RefCell<AppState>>, msg: &IpcMsg) {
    match msg.cmd.as_str() {
        "annotate_ready" => {}
        "annotate_pass" => {
            let mut st = state.borrow_mut();
            st.annotate_pass = msg.pass.unwrap_or(false);
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
    st.annotate_clear = true;
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
