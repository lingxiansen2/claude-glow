// Portable app config, stored next to the EXE so the tool travels between PCs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// Absolute path to the Claude Code settings.json this machine uses.
    pub settings_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            settings_path: default_settings_path()
                .to_string_lossy()
                .to_string(),
        }
    }
}

/// The directory containing the running EXE (portable config lives here).
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn config_path() -> PathBuf {
    exe_dir().join("claude-glow.json")
}

/// Best-guess location of Claude Code's global settings on this machine.
pub fn default_settings_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        return home.join(".claude").join("settings.json");
    }
    PathBuf::from(".claude/settings.json")
}

pub fn load() -> Config {
    match std::fs::read_to_string(config_path()) {
        // Tolerate a UTF-8 BOM (editors / PowerShell often add one).
        Ok(s) => serde_json::from_str(s.trim_start_matches('\u{feff}')).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(cfg).unwrap_or_else(|_| "{}".into());
    std::fs::write(config_path(), s)
}

/// Absolute path to the running EXE, used when writing hook commands.
pub fn exe_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "claude-glow.exe".into())
}
