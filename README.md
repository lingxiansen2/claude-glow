# claude-glow

A **portable, single-EXE** screen-edge ambient glow that reflects Claude Code's
live status — a tiny native Rust rewrite of the Electron prototype (~0.4 MB vs
~150 MB).

- 🔴 **thinking** — red breathing pulse (Claude is reasoning)
- 🟠 **tooling** — steady orange glow (running a tool / shell / web)
- 🟡 **waiting** — yellow fast strobe (needs your confirmation)
- 🟢 **done** — solid green, fades out (turn finished)

A transparent, click-through, always-on-top Win32 layered window draws the glow.
A status file is updated by Claude Code hooks that call this same EXE.

```
Claude Code hooks ──▶ claude-glow.exe --set <state>
                          └─▶ %TEMP%\claude-glow\status.json ──poll──▶ overlay
```

## One EXE, several modes

| Invocation                       | What it does                                   |
| -------------------------------- | ---------------------------------------------- |
| `claude-glow.exe`                | Config window + tray icon + overlay            |
| `claude-glow.exe --set <state>`  | Write the status file and exit (used by hooks) |
| `claude-glow.exe --install`      | Add hooks to the configured settings.json      |
| `claude-glow.exe --uninstall`    | Remove our hooks (leaves your other hooks)     |

`<state>` ∈ `thinking | tooling | waiting | done | idle`.

## Use it

1. Run `claude-glow.exe`. The config window shows the auto-detected
   `~/.claude/settings.json`. Adjust the path if needed (**Browse…**), then
   click **安装 Hooks / Install Hooks**.
2. **Restart your Claude Code session** so it reloads the hooks.
3. Closing the window minimizes to the tray; right-click the tray icon to
   reopen, toggle the overlay, or quit.

Config is saved to `claude-glow.json` **next to the EXE**, so the whole thing is
portable — copy `claude-glow.exe` to another PC, run it, set that machine's
settings.json path, and install.

## Portability

The release EXE is self-contained: it links the C runtime and libunwind
statically (`crt-static`), so it depends only on standard Windows 10/11 system
DLLs + UCRT — no MinGW/LLVM runtime DLLs needed on the target machine.

## Build

Requires the **windows-gnullvm** toolchain (matches the installed LLVM-MinGW
UCRT). The repo pins this via `rustup override` + `.cargo/config.toml`.

```powershell
cd D:\workspace\Github\claude-glow
rustup override set stable-x86_64-pc-windows-gnullvm   # once
cargo build --release
# -> target\release\claude-glow.exe  (~0.4 MB)
```

> Why gnullvm: the MSVC target needs `link.exe` (not installed); the plain
> `windows-gnu` target expects classic MinGW (MSVCRT + libgcc), which clashes
> with the installed LLVM-MinGW (UCRT + libunwind). `gnullvm` is the matching
> target.

## Status → hook mapping

| Hook event       | State    |
| ---------------- | -------- |
| UserPromptSubmit | thinking |
| PreToolUse       | tooling  |
| PostToolUse      | thinking |
| Notification     | waiting  |
| Stop             | done     |

## Source layout

- `src/main.rs` — arg dispatch (`--set` / `--install` / GUI)
- `src/overlay.rs` — Win32 layered window + per-pixel-alpha glow animation
- `src/ui.rs` — config window + tray (native-windows-gui)
- `src/hooks.rs` — install/remove hooks in settings.json (preserves others)
- `src/config.rs` — portable config (lives next to the EXE)
- `src/status.rs` — status file read/write
- `assets/icon.ico` — embedded tray icon
