// Win32 layered, click-through, topmost overlay that paints a glowing border
// reflecting the current status. Runs its own message loop on a dedicated
// thread; reads the status file directly so no cross-thread state is needed.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, GetDC, ReleaseDC, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Toggle drawn from the tray/config UI. When false the overlay hides.
pub static OVERLAY_ENABLED: AtomicBool = AtomicBool::new(true);

const TIMER_ID: usize = 1;
const BORDER_THICKNESS: i32 = 70;
const PEAK_ALPHA: f32 = 205.0;
const DONE_HOLD_MS: u64 = 4000;
const DONE_FADE_MS: u64 = 800;

struct Overlay {
    hwnd: HWND,
    dc_mem: HDC,
    bits: *mut u8,
    width: i32,
    height: i32,
    start: Instant,
    last_read: Instant,
    state: String,
    ts: u64,
    visible: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// (r, g, b) for a state.
fn state_color(state: &str) -> (u8, u8, u8) {
    match state {
        "thinking" => (255, 40, 40),
        "tooling" => (255, 100, 0),
        "waiting" => (255, 245, 130),
        "done" => (40, 230, 90),
        _ => (0, 0, 0),
    }
}

/// Global opacity 0..1 for the current state/time. None => hide the overlay.
fn state_opacity(state: &str, ts: u64, t: f64) -> Option<f32> {
    use std::f64::consts::PI;
    match state {
        "thinking" => {
            let s = 0.5 + 0.5 * (2.0 * PI * t / 1.1).sin();
            Some((0.35 + 0.65 * s) as f32)
        }
        "tooling" => {
            let s = 0.5 + 0.5 * (2.0 * PI * t / 2.4).sin();
            Some((0.82 + 0.18 * s) as f32)
        }
        "waiting" => {
            let phase = t.rem_euclid(0.45);
            Some(if phase < 0.225 { 1.0 } else { 0.1 })
        }
        "done" => {
            let elapsed = now_ms().saturating_sub(ts);
            if elapsed < DONE_HOLD_MS {
                Some(1.0)
            } else if elapsed < DONE_HOLD_MS + DONE_FADE_MS {
                let f = (elapsed - DONE_HOLD_MS) as f32 / DONE_FADE_MS as f32;
                Some(1.0 - f)
            } else {
                None
            }
        }
        _ => None, // idle / unknown
    }
}

/// Paint the glowing border into the premultiplied BGRA top-down buffer.
fn fill_border(bits: *mut u8, w: i32, h: i32, color: (u8, u8, u8), opacity: f32) {
    let (r, g, b) = color;
    let t = BORDER_THICKNESS;
    unsafe {
        std::ptr::write_bytes(bits, 0, (w * h * 4) as usize);
        for y in 0..h {
            let band_y = y.min(h - 1 - y);
            // Rows inside the top/bottom band fill fully; otherwise only the
            // left/right columns carry the glow.
            let ranges: [(i32, i32); 2] = if band_y < t {
                [(0, w), (0, 0)]
            } else {
                [(0, t), (w - t, w)]
            };
            for (x0, x1) in ranges {
                for x in x0..x1 {
                    let dx = x.min(w - 1 - x);
                    let d = band_y.min(dx);
                    if d >= t {
                        continue;
                    }
                    let tnorm = 1.0 - (d as f32) / (t as f32);
                    let mut a = PEAK_ALPHA * tnorm * tnorm * opacity;
                    if d < 3 {
                        // crisp defined edge line
                        a = a.max(180.0 * opacity);
                    }
                    if a < 1.0 {
                        continue;
                    }
                    let af = a / 255.0;
                    let idx = ((y * w + x) * 4) as usize;
                    *bits.add(idx) = (b as f32 * af) as u8;
                    *bits.add(idx + 1) = (g as f32 * af) as u8;
                    *bits.add(idx + 2) = (r as f32 * af) as u8;
                    *bits.add(idx + 3) = a as u8;
                }
            }
        }
    }
}

fn render(ov: &mut Overlay) {
    // Refresh status from disk at ~150 ms cadence.
    if ov.last_read.elapsed().as_millis() >= 150 {
        let (s, ts) = crate::status::read_state();
        ov.state = s;
        ov.ts = ts;
        ov.last_read = Instant::now();
    }

    let enabled = OVERLAY_ENABLED.load(Ordering::Relaxed);
    let t = ov.start.elapsed().as_secs_f64();
    let opacity = if enabled {
        state_opacity(&ov.state, ov.ts, t)
    } else {
        None
    };

    match opacity {
        None => {
            if ov.visible {
                unsafe { let _ = ShowWindow(ov.hwnd, SW_HIDE); }
                ov.visible = false;
            }
        }
        Some(op) => {
            let color = state_color(&ov.state);
            fill_border(ov.bits, ov.width, ov.height, color, op);

            let size = SIZE { cx: ov.width, cy: ov.height };
            let src = POINT { x: 0, y: 0 };
            let dst = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            unsafe {
                let screen = GetDC(None);
                let _ = UpdateLayeredWindow(
                    ov.hwnd,
                    screen,
                    Some(&dst),
                    Some(&size),
                    ov.dc_mem,
                    Some(&src),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );
                ReleaseDC(None, screen);
                if !ov.visible {
                    let _ = ShowWindow(ov.hwnd, SW_SHOWNOACTIVATE);
                    ov.visible = true;
                }
            }
        }
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Overlay;
            if !ptr.is_null() {
                render(&mut *ptr);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Create the overlay window and run its message loop. Blocks the thread.
pub fn run_overlay() {
    unsafe {
        let hmodule = GetModuleHandleW(None).unwrap();
        let hinst = HINSTANCE(hmodule.0);
        let class_name = to_wide("ClaudeGlowOverlay");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);

        let ex_style = WS_EX_LAYERED
            | WS_EX_TRANSPARENT
            | WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE;

        let hwnd = CreateWindowExW(
            ex_style,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide("Claude Glow").as_ptr()),
            WS_POPUP,
            0,
            0,
            width,
            height,
            None,
            None,
            hinst,
            None,
        )
        .expect("overlay window");

        // Allocate the 32-bit top-down DIB section we paint into.
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let screen = GetDC(None);
        let dc_mem = CreateCompatibleDC(screen);
        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        let hbmp: HBITMAP =
            CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)
                .expect("dib section");
        SelectObject(dc_mem, HGDIOBJ(hbmp.0));
        ReleaseDC(None, screen);

        let overlay = Box::new(Overlay {
            hwnd,
            dc_mem,
            bits: bits_ptr as *mut u8,
            width,
            height,
            start: Instant::now(),
            last_read: Instant::now() - std::time::Duration::from_secs(1),
            state: "idle".into(),
            ts: 0,
            visible: false,
        });
        let raw = Box::into_raw(overlay);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw as isize);

        // First paint + animation timer (~33 fps).
        render(&mut *raw);
        SetTimer(hwnd, TIMER_ID, 30, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup (reached only if the loop exits).
        let _ = DeleteDC(dc_mem);
        drop(Box::from_raw(raw));
    }
}
