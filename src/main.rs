#![windows_subsystem = "windows"]

mod recorder;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

use recorder::ActiveCapture;

#[derive(Debug, Deserialize)]
struct IpcMsg {
    cmd: String,
    #[serde(default)]
    monitor: usize,
    #[serde(default)]
    cursor: bool,
}

struct AppState {
    capture: Option<ActiveCapture>,
    saving: bool,
    last_path: Option<PathBuf>,
    status: String,
    error: Option<String>,
    monitors: Vec<String>,
    selected: usize,
}

impl AppState {
    fn new() -> Self {
        let infos = recorder::list_monitors().unwrap_or_default();
        let selected = infos.iter().position(|m| m.is_primary).unwrap_or(0);
        let monitors = infos.into_iter().map(|m| m.label).collect();
        Self {
            capture: None,
            saving: false,
            last_path: None,
            status: "ئامادەیە بۆ تۆمارکردنی شاشەکەت".into(),
            error: None,
            monitors,
            selected,
        }
    }

    fn snapshot(&self) -> serde_json::Value {
        let (elapsed, frames, recording) = if let Some(c) = &self.capture {
            (
                c.stats.elapsed_secs(),
                c.stats.frame_count.load(Ordering::SeqCst),
                true,
            )
        } else {
            (0, 0, false)
        };

        json!({
            "monitors": self.monitors,
            "selected": self.selected,
            "elapsed": elapsed,
            "frames": frames,
            "recording": recording,
            "saving": self.saving,
            "status": self.status,
            "error": self.error,
            "lastPath": self.last_path.as_ref().map(|p| p.display().to_string()),
        })
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoopBuilder::<()>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("ڕیل — سکرین ڕیکۆردەر")
        .with_inner_size(tao::dpi::LogicalSize::new(430.0, 700.0))
        .with_resizable(true)
        .build(&event_loop)
        .context("failed to create window")?;

    let state = Rc::new(RefCell::new(AppState::new()));
    let html = build_html();

    let state_for_ipc = Rc::clone(&state);
    let proxy = event_loop.create_proxy();

    let webview = WebViewBuilder::new(&window)
        .with_html(html)
        .with_ipc_handler(move |request| {
            let Ok(msg) = serde_json::from_str::<IpcMsg>(request.body()) else {
                return;
            };
            handle_ipc(&state_for_ipc, &msg);
            let _ = proxy.send_event(());
        })
        .build()
        .context("failed to create webview")?;

    let webview = Rc::new(webview);
    push_ui(&webview, &state.borrow());

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(()) => {
                {
                    let mut st = state.borrow_mut();
                    if let Some(cap) = &st.capture {
                        if cap.is_finished() {
                            let _ = finish_stop(&mut st);
                        }
                    }
                }
                push_ui(&webview, &state.borrow());
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                let mut st = state.borrow_mut();
                if st.capture.is_some() {
                    let _ = finish_stop(&mut st);
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn build_html() -> String {
    let font_b64 = base64_encode(include_bytes!("../assets/fonts/Rabar.ttf"));
    include_str!("../assets/ui.html").replace(
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

fn handle_ipc(state: &Rc<RefCell<AppState>>, msg: &IpcMsg) {
    match msg.cmd.as_str() {
        "ready" | "tick" => {}
        "start" => {
            let mut st = state.borrow_mut();
            if st.capture.is_some() || st.saving {
                return;
            }
            st.error = None;
            st.selected = msg.monitor;
            match recorder::default_output_path() {
                Ok(output) => match ActiveCapture::start(msg.monitor, msg.cursor, output) {
                    Ok(capture) => {
                        let name = file_name(&capture.output);
                        st.status = format!("خەریکی تۆمارکردنە بۆ {name}");
                        st.capture = Some(capture);
                    }
                    Err(e) => {
                        st.error = Some(simplify_error(&e.to_string()));
                        st.status = "نەتوانرا تۆمار دەست پێبکرێت".into();
                    }
                },
                Err(e) => {
                    st.error = Some(simplify_error(&e.to_string()));
                    st.status = "نەتوانرا ڕێگای پاشەکەوت دروست بکرێت".into();
                }
            }
        }
        "stop" => {
            let mut st = state.borrow_mut();
            if st.capture.is_some() {
                let _ = finish_stop(&mut st);
            }
        }
        "open_file" => {
            let st = state.borrow();
            if let Some(path) = &st.last_path {
                let _ = open::that(path);
            }
        }
        "open_folder" => {
            let st = state.borrow();
            if let Some(path) = &st.last_path {
                if let Some(parent) = path.parent() {
                    let _ = open::that(parent);
                }
            }
        }
        _ => {}
    }
}

fn finish_stop(st: &mut AppState) -> Result<()> {
    if let Some(capture) = st.capture.take() {
        st.saving = true;
        st.status = "خەریکی پاشەکەوتکردنی ڤیدیۆ…".into();
        match capture.stop() {
            Ok(path) => {
                let size = std::fs::metadata(&path)
                    .map(|m| m.len() as f64 / 1_048_576.0)
                    .unwrap_or(0.0);
                st.status = format!("پاشەکەوت کرا · {size:.1} مێگابایت");
                st.last_path = Some(path);
                st.error = None;
            }
            Err(e) => {
                st.error = Some(simplify_error(&e.to_string()));
                st.status = "وەستاندن سەرنەکەوت".into();
            }
        }
        st.saving = false;
    }
    Ok(())
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
        "کۆدکەری ڤیدیۆ سەرنەکەوت".into()
    } else if err.contains("create") {
        "نەتوانرا فایل دروست بکرێت".into()
    } else {
        err.to_string()
    }
}
