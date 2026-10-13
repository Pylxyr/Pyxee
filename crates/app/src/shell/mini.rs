//! The always-on-top mini player. It remembers where it was left; `pyxee_shell::geometry` keeps it reachable when
//! monitors change.

use super::ShellState;
use pyxee_shell::anchor::clamp_into;
use pyxee_shell::geometry::fit_on_screen;
use pyxee_shell::Rect;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LABEL: &str = "mini";
const SIZE: (f64, f64) = (340.0, 112.0);
const SAVE_EVERY: Duration = Duration::from_millis(800);

pub fn toggle(app: &AppHandle) {
    match app.get_webview_window(LABEL) {
        Some(w) if w.is_visible().unwrap_or(false) => hide(app),
        Some(w) => show(app, &w),
        None => match build(app) {
            Ok(w) => show(app, &w),
            Err(e) => log::warn!("could not open the mini player: {e}"),
        },
    }
}

fn build(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=mini".into()))
        .title("Pyxee mini player")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .maximizable(false)
        .always_on_top(true)
        .visible(false)
        .shadow(true)
        .build()?;
    let _ = crate::window::apply_backdrop(&w);
    Ok(w)
}

fn show(app: &AppHandle, w: &WebviewWindow) {
    place(app, w);
    let _ = w.show();
    let _ = w.set_focus();
}

/// Puts the window where it was left (if that is still on a screen), else in the corner of the primary screen.
fn place(app: &AppHandle, w: &WebviewWindow) {
    let monitors = super::flyout::monitors(app);
    let Some((first, _)) = monitors.first() else {
        return;
    };
    let saved = app
        .state::<ShellState>()
        .settings
        .lock()
        .ok()
        .and_then(|s| s.mini_bounds);
    let work: Vec<Rect> = monitors.iter().map(|(m, _)| m.work).collect();
    let scale_at = |x: i32, y: i32| {
        monitors
            .iter()
            .find(|(m, _)| m.area.contains((x, y)))
            .map_or(monitors[0].1, |(_, s)| *s)
    };
    let (rect, scale) = match saved {
        Some(b) => {
            let scale = scale_at(b.x, b.y);
            let size = (
                (SIZE.0 * scale).round() as i32,
                (SIZE.1 * scale).round() as i32,
            );
            (
                fit_on_screen(Rect::new(b.x, b.y, size.0, size.1), &work),
                scale,
            )
        }
        None => {
            let scale = monitors[0].1;
            let size = (
                (SIZE.0 * scale).round() as i32,
                (SIZE.1 * scale).round() as i32,
            );
            let margin = (24.0 * scale).round() as i32;
            let (x, y) = clamp_into(
                first.work,
                (first.work.right(), first.work.bottom()),
                size,
                margin,
            );
            (Rect::new(x, y, size.0, size.1), scale)
        }
    };
    let _ = scale;
    let _ = w.set_size(PhysicalSize::new(rect.w as u32, rect.h as u32));
    let _ = w.set_position(PhysicalPosition::new(rect.x, rect.y));
}

/// The window moved: remember where, writing to disk at most every 800 ms.
pub fn on_moved(app: &AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else {
        return;
    };
    if !w.is_visible().unwrap_or(false) {
        return; // moves while it is being placed are not the user's
    }
    let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) else {
        return;
    };
    let shell = app.state::<ShellState>();
    if let Ok(mut st) = shell.settings.lock() {
        st.mini_bounds = Some(Rect::new(p.x, p.y, s.width as i32, s.height as i32));
    }
    shell.mini_dirty.store(true, Ordering::Relaxed);
    let due = shell
        .mini_saved
        .lock()
        .ok()
        .is_none_or(|t| t.is_none_or(|t| t.elapsed() >= SAVE_EVERY));
    if due {
        flush(app);
    }
}

/// Writes the position if it changed since the last write.
pub fn flush(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    if shell.mini_dirty.swap(false, Ordering::Relaxed) {
        shell.save_settings();
        if let Ok(mut t) = shell.mini_saved.lock() {
            *t = Some(Instant::now());
        }
    }
}

pub fn hide(app: &AppHandle) {
    flush(app);
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}
