// Install / uninstall the glow status hooks into a Claude Code settings.json,
// preserving any hooks the user already has (e.g. unrelated bridges).

use serde_json::{json, Map, Value};
use std::path::Path;

/// (event name, state to write, whether the event group needs a "*" matcher).
const EVENTS: &[(&str, &str, bool)] = &[
    ("UserPromptSubmit", "thinking", false),
    ("PreToolUse", "tooling", true),
    ("PostToolUse", "thinking", true),
    ("Notification", "waiting", true),
    ("Stop", "done", false),
];

/// Marker that identifies a hook command as belonging to this tool.
fn is_our_command(cmd: &str) -> bool {
    cmd.contains("claude-glow") && cmd.contains("--set")
}

fn our_command(exe: &str, state: &str) -> String {
    format!("\"{}\" --set {}", exe, state)
}

fn read_root(path: &Path) -> Value {
    match std::fs::read_to_string(path) {
        Ok(s) if !s.trim().is_empty() => {
            // Tolerate a UTF-8 BOM at the start of settings.json.
            serde_json::from_str(s.trim_start_matches('\u{feff}'))
                .unwrap_or_else(|_| json!({}))
        }
        _ => json!({}),
    }
}

/// True if a group object is one of ours (any command carries our marker).
fn group_is_ours(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|arr| {
            arr.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(is_our_command)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Drop our groups from an event array; returns the kept (foreign) groups.
fn strip_ours(event_arr: &[Value]) -> Vec<Value> {
    event_arr
        .iter()
        .filter(|g| !group_is_ours(g))
        .cloned()
        .collect()
}

pub fn install(settings_path: &str, exe: &str) -> std::io::Result<()> {
    let path = Path::new(settings_path);
    let mut root = read_root(path);
    if !root.is_object() {
        root = json!({});
    }

    let obj = root.as_object_mut().unwrap();
    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks = hooks.as_object_mut().expect("hooks must be an object");

    for (event, state, has_matcher) in EVENTS {
        let existing = hooks
            .get(*event)
            .and_then(|v| v.as_array())
            .map(|a| strip_ours(a))
            .unwrap_or_default();

        let mut group = Map::new();
        if *has_matcher {
            group.insert("matcher".into(), json!("*"));
        }
        group.insert(
            "hooks".into(),
            json!([{ "type": "command", "command": our_command(exe, state) }]),
        );

        let mut arr = existing;
        arr.push(Value::Object(group));
        hooks.insert((*event).to_string(), Value::Array(arr));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let out = serde_json::to_string_pretty(&root).unwrap();
    std::fs::write(path, out)
}

pub fn uninstall(settings_path: &str) -> std::io::Result<()> {
    let path = Path::new(settings_path);
    if !path.exists() {
        return Ok(());
    }
    let mut root = read_root(path);
    if let Some(hooks) = root
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
    {
        let events: Vec<String> = hooks.keys().cloned().collect();
        for event in events {
            if let Some(arr) = hooks.get(&event).and_then(|v| v.as_array()) {
                let kept = strip_ours(arr);
                if kept.is_empty() {
                    hooks.remove(&event);
                } else {
                    hooks.insert(event, Value::Array(kept));
                }
            }
        }
    }
    let out = serde_json::to_string_pretty(&root).unwrap();
    std::fs::write(path, out)
}

/// Are our hooks currently present in this settings file?
pub fn is_installed(settings_path: &str) -> bool {
    let root = read_root(Path::new(settings_path));
    root.get("hooks")
        .and_then(|h| h.as_object())
        .map(|hooks| {
            hooks.values().any(|ev| {
                ev.as_array()
                    .map(|arr| arr.iter().any(group_is_ours))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
