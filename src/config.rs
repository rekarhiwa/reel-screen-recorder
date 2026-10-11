use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::recorder::{Codec, Quality};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    #[serde(default)]
    pub settings_version: u32,
    pub monitor_index: usize,
    #[serde(default)]
    pub monitor_index_b: usize,
    #[serde(default)]
    pub dual_screen: bool,
    pub show_cursor: bool,
    pub quality: String,
    pub fps: u32,
    pub codec: String,
    pub mic_enabled: bool,
    pub camera_enabled: bool,
    #[serde(default)]
    pub camera_device_id: String,
    pub camera_shape: String,
    #[serde(default = "default_cam_filter")]
    pub camera_filter: String,
    pub greenscreen: bool,
    pub greenscreen_strength: u8,
    #[serde(default = "default_gs_color")]
    pub greenscreen_color: String,
    #[serde(default = "default_gs_softness")]
    pub greenscreen_softness: u8,
    pub camera_size: u32,
    pub hotkey_start_stop: String,
    pub hotkey_mic: String,
    pub hotkey_camera: String,
    #[serde(default = "default_pause_hotkey")]
    pub hotkey_pause: String,
    #[serde(default = "default_true")]
    pub annotate_enabled: bool,
    #[serde(default = "default_annotate_hotkey")]
    pub hotkey_annotate: String,
}

fn default_pause_hotkey() -> String {
    "Ctrl+Shift+P".into()
}

fn default_gs_color() -> String {
    "#22c55e".into()
}

fn default_gs_softness() -> u8 {
    35
}

fn default_cam_filter() -> String {
    "none".into()
}

fn default_true() -> bool {
    true
}

fn default_annotate_hotkey() -> String {
    "Ctrl+Shift+D".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            settings_version: 3,
            monitor_index: 0,
            monitor_index_b: 1,
            dual_screen: false,
            show_cursor: true,
            quality: "high".into(),
            fps: 30,
            codec: "hevc".into(),
            mic_enabled: true,
            camera_enabled: false,
            camera_device_id: String::new(),
            camera_shape: "circle".into(),
            camera_filter: default_cam_filter(),
            greenscreen: false,
            greenscreen_strength: 40,
            greenscreen_color: default_gs_color(),
            greenscreen_softness: default_gs_softness(),
            camera_size: 220,
            hotkey_start_stop: "Ctrl+Shift+R".into(),
            hotkey_mic: "Ctrl+Shift+M".into(),
            hotkey_camera: "Ctrl+Shift+C".into(),
            hotkey_pause: "Ctrl+Shift+P".into(),
            annotate_enabled: true,
            hotkey_annotate: default_annotate_hotkey(),
        }
    }
}

impl AppConfig {
    fn path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("reel-settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let mut cfg: Self = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if cfg.settings_version < 2 {
            cfg.quality = "high".into();
            cfg.fps = 30;
            cfg.codec = "hevc".into();
            cfg.settings_version = 2;
        }
        if cfg.settings_version < 3 {
            cfg.settings_version = 3;
            cfg.save();
        }
        cfg
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, s);
        }
    }

    pub fn quality(&self) -> Quality {
        Quality::parse(&self.quality)
    }

    pub fn codec(&self) -> Codec {
        Codec::parse(&self.codec)
    }
}
