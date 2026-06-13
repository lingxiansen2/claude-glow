// claude-glow: portable screen-edge status glow for Claude Code.
//
//   claude-glow.exe                  -> config window + tray + overlay
//   claude-glow.exe --set <state>    -> write status file (used by hooks)
//   claude-glow.exe --install        -> install hooks into settings.json
//   claude-glow.exe --uninstall      -> remove our hooks from settings.json
#![windows_subsystem = "windows"]

mod config;
mod hooks;
mod overlay;
mod status;
mod ui;

/// A `Notification` hook fires for BOTH real permission prompts and benign
/// "waiting for your input" / idle notices. Read the event JSON on stdin and
/// keep "waiting" (yellow strobe) only for permission/approval/confirm
/// messages; otherwise return "idle" so the overlay goes dark instead of being
/// stuck strobing yellow after every turn ends.
fn resolve_waiting() -> String {
    use std::io::{IsTerminal, Read};
    // Manual `--set waiting` from a console has no piped event → assume a real
    // prompt and don't block on console input.
    if std::io::stdin().is_terminal() {
        return "waiting".into();
    }
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() || buf.trim().is_empty() {
        return "waiting".into();
    }
    let low = buf.to_lowercase();
    let is_permission = low.contains("permission")
        || low.contains("approval")
        || low.contains("approve")
        || low.contains("confirm");
    if is_permission {
        "waiting".into()
    } else {
        // Idle / "waiting for your input" / anything else → go dark.
        "idle".into()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Fast path used by Claude Code hooks: write status and exit immediately.
    if let Some(pos) = args.iter().position(|a| a == "--set") {
        if let Some(state) = args.get(pos + 1) {
            // Notification → "waiting" is ambiguous; classify by the event message.
            let resolved = if state == "waiting" {
                resolve_waiting()
            } else {
                state.clone()
            };
            if status::is_valid_state(&resolved) {
                let _ = status::write_state(&resolved);
            }
        }
        return;
    }

    if args.iter().any(|a| a == "--install") {
        let cfg = config::load();
        let _ = hooks::install(&cfg.settings_path, &config::exe_path());
        return;
    }

    if args.iter().any(|a| a == "--uninstall") {
        let cfg = config::load();
        let _ = hooks::uninstall(&cfg.settings_path);
        return;
    }

    // GUI mode: run the overlay on its own thread, config/tray on the main thread.
    std::thread::spawn(|| overlay::run_overlay());
    ui::run_app();
}
