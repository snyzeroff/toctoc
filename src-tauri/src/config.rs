use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_RELAY: &str = "https://ntfy.sh";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// carillon | goutte | toctoc | cloche | bulle | custom | none
    pub sound: String,
    pub custom_sound: Option<String>,
    /// 0.0 – 1.0
    pub volume: f32,
    /// top-left | top-center | top-right | center | bottom-left | bottom-center | bottom-right
    pub position: String,
    /// small | medium | large
    pub size: String,
    /// secondes
    pub duration: f32,
    pub autostart: bool,
    pub dnd: bool,
    /// raccourci global, vide = désactivé
    pub shortcut: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound: "goutte".into(),
            custom_sound: None,
            volume: 0.5,
            position: "top-right".into(),
            size: "medium".into(),
            duration: 4.0,
            autostart: false,
            dnd: false,
            shortcut: "CmdOrCtrl+Alt+P".into(),
        }
    }
}

pub const SHORTCUTS: [&str; 4] = ["CmdOrCtrl+Alt+P", "CmdOrCtrl+Alt+T", "CmdOrCtrl+Alt+Space", ""];

impl Settings {
    pub fn sanitize(&mut self) {
        self.volume = if self.volume.is_finite() { self.volume.clamp(0.0, 1.0) } else { 0.7 };
        self.duration = if self.duration.is_finite() { self.duration.clamp(1.0, 60.0) } else { 5.0 };
        const POS: [&str; 7] = [
            "top-left", "top-center", "top-right", "center", "bottom-left", "bottom-center", "bottom-right",
        ];
        if !POS.contains(&self.position.as_str()) {
            self.position = "top-right".into();
        }
        if !["small", "medium", "large"].contains(&self.size.as_str()) {
            self.size = "medium".into();
        }
        if !["carillon", "goutte", "toctoc", "cloche", "bulle", "custom", "none"].contains(&self.sound.as_str()) {
            self.sound = "carillon".into();
        }
        if self.sound == "custom" && self.custom_sound.is_none() {
            self.sound = "carillon".into();
        }
        if !SHORTCUTS.contains(&self.shortcut.as_str()) {
            self.shortcut = SHORTCUTS[0].into();
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pairing {
    /// canal privé sur le relais (issu de l'appairage, impossible à deviner)
    pub topic: String,
    /// clé de chiffrement de bout en bout (32 octets, base64)
    pub key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub device_id: String,
    pub relay: String,
    pub pairing: Option<Pairing>,
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_id: crate::crypto::random_hex(8),
            relay: DEFAULT_RELAY.into(),
            pairing: None,
            settings: Settings::default(),
        }
    }
}

pub fn config_file(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load(dir: &Path) -> Config {
    let mut cfg: Config = std::fs::read(config_file(dir))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if cfg.device_id.is_empty() {
        cfg.device_id = crate::crypto::random_hex(8);
    }
    if !cfg.relay.starts_with("https://") && !cfg.relay.starts_with("http://") {
        cfg.relay = DEFAULT_RELAY.into();
    }
    cfg.relay = cfg.relay.trim_end_matches('/').to_string();
    cfg.settings.sanitize();
    cfg
}

pub fn save(dir: &Path, cfg: &Config) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(json) = serde_json::to_vec_pretty(cfg) {
        let tmp = dir.join("config.json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, config_file(dir));
        }
    }
}

/// Petit journal local (aucun secret, aucun texte de message).
pub fn log(dir: &Path, line: &str) {
    use std::io::Write;
    let path = dir.join("toctoc.log");
    if std::fs::metadata(&path).map(|m| m.len() > 300_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{secs} {line}");
    }
}
