use anyhow::{bail, Context, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::config::AppConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotAction {
    StartStop,
    ToggleMic,
    ToggleCamera,
    TogglePause,
    ToggleAnnotate,
}

pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    start_stop: Option<HotKey>,
    mic: Option<HotKey>,
    camera: Option<HotKey>,
    pause: Option<HotKey>,
    annotate: Option<HotKey>,
}

impl Hotkeys {
    pub fn new() -> Result<Self> {
        Ok(Self {
            manager: GlobalHotKeyManager::new().context("failed to create hotkey manager")?,
            start_stop: None,
            mic: None,
            camera: None,
            pause: None,
            annotate: None,
        })
    }

    pub fn apply(&mut self, cfg: &AppConfig) -> Result<()> {
        self.unregister_all();

        let mut errors = Vec::new();

        self.start_stop = register_optional(
            &self.manager,
            &cfg.hotkey_start_stop,
            "دەستپێ/وەستان",
            &mut errors,
        );
        self.mic = register_optional(&self.manager, &cfg.hotkey_mic, "مایک", &mut errors);
        self.camera = register_optional(&self.manager, &cfg.hotkey_camera, "کامێرا", &mut errors);
        self.pause = register_optional(&self.manager, &cfg.hotkey_pause, "پۆز", &mut errors);
        self.annotate =
            register_optional(&self.manager, &cfg.hotkey_annotate, "قەلەم", &mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            bail!(errors.join(" · "))
        }
    }

    pub fn suspend(&mut self) {
        self.unregister_all();
    }

    fn unregister_all(&mut self) {
        if let Some(k) = self.start_stop.take() {
            let _ = self.manager.unregister(k);
        }
        if let Some(k) = self.mic.take() {
            let _ = self.manager.unregister(k);
        }
        if let Some(k) = self.camera.take() {
            let _ = self.manager.unregister(k);
        }
        if let Some(k) = self.pause.take() {
            let _ = self.manager.unregister(k);
        }
        if let Some(k) = self.annotate.take() {
            let _ = self.manager.unregister(k);
        }
    }

    pub fn poll_action(&self) -> Option<HotAction> {
        let rx = GlobalHotKeyEvent::receiver();
        while let Ok(ev) = rx.try_recv() {
            if ev.state != HotKeyState::Pressed {
                continue;
            }
            if self.start_stop.is_some_and(|k| k.id() == ev.id) {
                return Some(HotAction::StartStop);
            }
            if self.mic.is_some_and(|k| k.id() == ev.id) {
                return Some(HotAction::ToggleMic);
            }
            if self.camera.is_some_and(|k| k.id() == ev.id) {
                return Some(HotAction::ToggleCamera);
            }
            if self.pause.is_some_and(|k| k.id() == ev.id) {
                return Some(HotAction::TogglePause);
            }
            if self.annotate.is_some_and(|k| k.id() == ev.id) {
                return Some(HotAction::ToggleAnnotate);
            }
        }
        None
    }
}

fn register_optional(
    manager: &GlobalHotKeyManager,
    raw: &str,
    label: &str,
    errors: &mut Vec<String>,
) -> Option<HotKey> {
    let s = raw.trim();
    if s.is_empty() || s == "—" || s == "-" || s == "none" {
        return None;
    }
    match parse_hotkey(s) {
        Ok(hk) => match manager.register(hk) {
            Ok(()) => Some(hk),
            Err(e) => {
                errors.push(format!("{label}: {e}"));
                None
            }
        },
        Err(e) => {
            errors.push(format!("{label}: {e}"));
            None
        }
    }
}

fn parse_hotkey(s: &str) -> Result<HotKey> {
    let mut mods = Modifiers::empty();
    let mut code: Option<Code> = None;
    for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "shift" => mods |= Modifiers::SHIFT,
            "alt" => mods |= Modifiers::ALT,
            "super" | "meta" | "win" => mods |= Modifiers::SUPER,
            other => {
                code = Some(code_from_str(other)?);
            }
        }
    }
    let Some(code) = code else {
        bail!("کلیل دیاری نەکرا: {s}");
    };
    if mods.is_empty() {
        bail!("پێویستە Ctrl/Shift/Alt هەبێت: {s}");
    }
    Ok(HotKey::new(Some(mods), code))
}

fn code_from_str(s: &str) -> Result<Code> {
    let u = s.to_ascii_uppercase();
    let code = match u.as_str() {
        "A" => Code::KeyA,
        "B" => Code::KeyB,
        "C" => Code::KeyC,
        "D" => Code::KeyD,
        "E" => Code::KeyE,
        "F" => Code::KeyF,
        "G" => Code::KeyG,
        "H" => Code::KeyH,
        "I" => Code::KeyI,
        "J" => Code::KeyJ,
        "K" => Code::KeyK,
        "L" => Code::KeyL,
        "M" => Code::KeyM,
        "N" => Code::KeyN,
        "O" => Code::KeyO,
        "P" => Code::KeyP,
        "Q" => Code::KeyQ,
        "R" => Code::KeyR,
        "S" => Code::KeyS,
        "T" => Code::KeyT,
        "U" => Code::KeyU,
        "V" => Code::KeyV,
        "W" => Code::KeyW,
        "X" => Code::KeyX,
        "Y" => Code::KeyY,
        "Z" => Code::KeyZ,
        "0" | "DIGIT0" => Code::Digit0,
        "1" | "DIGIT1" => Code::Digit1,
        "2" | "DIGIT2" => Code::Digit2,
        "3" | "DIGIT3" => Code::Digit3,
        "4" | "DIGIT4" => Code::Digit4,
        "5" | "DIGIT5" => Code::Digit5,
        "6" | "DIGIT6" => Code::Digit6,
        "7" | "DIGIT7" => Code::Digit7,
        "8" | "DIGIT8" => Code::Digit8,
        "9" | "DIGIT9" => Code::Digit9,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        "SPACE" | " " | "SPACEBAR" => Code::Space,
        "TAB" => Code::Tab,
        "ENTER" | "RETURN" => Code::Enter,
        "BACKSPACE" => Code::Backspace,
        "DELETE" | "DEL" => Code::Delete,
        "INSERT" | "INS" => Code::Insert,
        "HOME" => Code::Home,
        "END" => Code::End,
        "PAGEUP" | "PAGE UP" | "PGUP" => Code::PageUp,
        "PAGEDOWN" | "PAGE DOWN" | "PGDN" => Code::PageDown,
        "ARROWUP" | "UP" => Code::ArrowUp,
        "ARROWDOWN" | "DOWN" => Code::ArrowDown,
        "ARROWLEFT" | "LEFT" => Code::ArrowLeft,
        "ARROWRIGHT" | "RIGHT" => Code::ArrowRight,
        other => bail!("کلیلی پشتگیری نەکراو: {other}"),
    };
    Ok(code)
}
