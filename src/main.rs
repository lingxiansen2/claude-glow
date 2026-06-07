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

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Fast path used by Claude Code hooks: write status and exit immediately.
    if let Some(pos) = args.iter().position(|a| a == "--set") {
        if let Some(state) = args.get(pos + 1) {
            if status::is_valid_state(state) {
                let _ = status::write_state(state);
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
