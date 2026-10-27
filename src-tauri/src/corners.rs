//! Window corners at the reader's corner style (Settings > Appearance), on
//! every system, square while the window fills the screen (maximized, full
//! screen, tiled), where a rounded corner would show the desktop behind it.
//! The same approach as Kryoto Desktop's `system.rs`.
//!
//! * Windows 11 rounds a frameless window when asked (DWM).
//! * Windows 10 has no such call: a rounded region, redone on every resize.
//! * Linux: a shape mask on an opaque window. A see-through window drew dark
//!   pixels as clear on WebKitGTK 2.54 and the launcher vanished on new
//!   distros (kryoto-bugs KB-0006), so the corners are cut off for real.

use tauri::Runtime;

/// The corner radius in logical pixels, set from the page by `window_corners`.
static RADIUS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(16);

fn corner() -> i32 {
    RADIUS.load(std::sync::atomic::Ordering::Relaxed)
}

/// The page's corner style changed (`look.ts`): the window follows it.
#[tauri::command]
pub fn window_corners(window: tauri::WebviewWindow, radius: String) {
    let px = match radius.as_str() {
        "sharp" => 0,
        "soft" => 2,
        "rounded" => 5,
        "round" => 10,
        _ => 16,
    };
    RADIUS.store(px, std::sync::atomic::Ordering::Relaxed);
    apply(&window);
}

#[cfg(windows)]
static WIN10_REGION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set the corners up once, at start.
pub fn round<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    #[cfg(target_os = "linux")]
    {
        use gtk::glib::Propagation;
        use gtk::prelude::*;
        if let Ok(win) = window.gtk_window() {
            win.connect_size_allocate(|win, _| shape(win));
            win.connect_window_state_event(|win, _| {
                shape(win);
                Propagation::Proceed
            });
        }
    }
    apply(window);
}

/// Draw the corners at the current style.
fn apply<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
        if let Ok(hwnd) = window.hwnd() {
            // DWMWCP_DONOTROUND = 1, ROUND = 2, ROUNDSMALL = 3: Windows 11's three.
            let pref: i32 = match corner() {
                0 => 1,
                1..=5 => 3,
                _ => 2,
            };
            // SAFETY: a live window handle and a pointer to a 4-byte value.
            let result = unsafe {
                DwmSetWindowAttribute(hwnd.0 as _, DWMWA_WINDOW_CORNER_PREFERENCE as u32, (&pref as *const i32).cast(), std::mem::size_of::<i32>() as u32)
            };
            if result != 0 {
                WIN10_REGION.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        refresh(window);
    }
    // GTK only on its own thread; the Tauri window travels, the GTK one is
    // looked up there.
    #[cfg(target_os = "linux")]
    {
        let handle = window.clone();
        let _ = window.run_on_main_thread(move || {
            if let Ok(win) = handle.gtk_window() {
                shape(&win);
            }
        });
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = window;
}

/// After a resize: redo the Windows 10 region (a no-op elsewhere).
pub fn refresh<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
        if !WIN10_REGION.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let Ok(hwnd) = window.hwnd() else { return };
        let square = corner() == 0 || window.is_maximized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false);
        let Ok(size) = window.outer_size() else { return };
        let d = (f64::from(corner() * 2) * window.scale_factor().unwrap_or(1.0)).round() as i32;
        // SAFETY: a live window handle; SetWindowRgn takes ownership of the region.
        unsafe {
            if square {
                SetWindowRgn(hwnd.0 as _, std::ptr::null_mut(), 1);
            } else {
                SetWindowRgn(hwnd.0 as _, CreateRoundRectRgn(0, 0, size.width as i32 + 1, size.height as i32 + 1, d, d), 1);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(target_os = "linux")]
fn shape(win: &gtk::ApplicationWindow) {
    use gtk::cairo::{RectangleInt, Region};
    use gtk::gdk::WindowState;
    use gtk::prelude::*;
    const SQUARE: WindowState = WindowState::MAXIMIZED.union(WindowState::FULLSCREEN).union(WindowState::TILED);
    let r = corner();
    let square = r == 0 || win.window().is_none_or(|g| g.state().intersects(SQUARE));
    let (w, h) = (win.allocated_width(), win.allocated_height());
    if square || w <= 2 * r || h <= 2 * r {
        win.shape_combine_region(None);
        return;
    }
    let region = Region::create();
    for y in 0..r {
        let dy = f64::from(r) - f64::from(y) - 0.5;
        let inset = (f64::from(r) - (f64::from(r * r) - dy * dy).max(0.0).sqrt()).round() as i32;
        let _ = region.union_rectangle(&RectangleInt::new(inset, y, w - 2 * inset, 1));
        let _ = region.union_rectangle(&RectangleInt::new(inset, h - 1 - y, w - 2 * inset, 1));
    }
    let _ = region.union_rectangle(&RectangleInt::new(0, r, w, h - 2 * r));
    win.shape_combine_region(Some(&region));
}
