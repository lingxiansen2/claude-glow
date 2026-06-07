// Status file shared between the hook writer (`--set`) and the overlay reader.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Directory holding the status file (per-machine temp).
pub fn status_dir() -> PathBuf {
    std::env::temp_dir().join("claude-glow")
}

pub fn status_file() -> PathBuf {
    status_dir().join("status.json")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Valid states the overlay understands.
pub fn is_valid_state(s: &str) -> bool {
    matches!(s, "thinking" | "tooling" | "waiting" | "done" | "idle")
}

/// Write `{ "state": "...", "ts": <ms> }`. Called by the `--set` hook path.
pub fn write_state(state: &str) -> std::io::Result<()> {
    let dir = status_dir();
    fs::create_dir_all(&dir)?;
    let payload = format!("{{\"state\":\"{}\",\"ts\":{}}}", state, now_ms());
    fs::write(status_file(), payload)
}

/// Current state + its timestamp (ms). Missing/invalid file -> idle.
pub fn read_state() -> (String, u64) {
    let raw = match fs::read_to_string(status_file()) {
        Ok(r) => r,
        Err(_) => return ("idle".into(), 0),
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return ("idle".into(), 0);
    }
    // Tiny hand-parse to avoid pulling serde into the hot overlay loop.
    let state = extract_str(raw, "state").unwrap_or_else(|| "idle".into());
    let ts = extract_num(raw, "ts").unwrap_or(0);
    if is_valid_state(&state) {
        (state, ts)
    } else {
        ("idle".into(), ts)
    }
}

fn extract_str(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\"", key);
    let i = json.find(&pat)? + pat.len();
    let rest = &json[i..];
    let c = rest.find(':')? + 1;
    let rest = &rest[c..];
    let q1 = rest.find('"')? + 1;
    let rest2 = &rest[q1..];
    let q2 = rest2.find('"')?;
    Some(rest2[..q2].to_string())
}

fn extract_num(json: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{}\"", key);
    let i = json.find(&pat)? + pat.len();
    let rest = &json[i..];
    let c = rest.find(':')? + 1;
    let rest = rest[c..].trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}
