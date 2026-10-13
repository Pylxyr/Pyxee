//! The tray flyout: a small borderless window that opens next to the tray icon and closes when it loses focus.
//! Where it goes is `pyxee_shell::anchor`; this file only talks to Tauri.

use super::ShellState;
use pyxee_shell::anchor::{flyout_position, monitor_at, reopen_after_blur, Monitor};
use pyxee_shell::Rect;
use std::time::Instant;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LABEL: &str = "flyout";
/// Logical size; the real size is this times the scale of the monitor the tray is on.
const SIZE: (f64, f64) = (360.0, 148.0);
const MARGIN: f64 = 12.0;

pub fn toggle(app: &AppHandle, icon: tauri::Rect) {
    if app
        .get_webview_window(LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
    {
        hide(app);
        return;
    }
    let shell = app.state::<ShellState>();
    let just_closed = shell
        .flyout_hidden_at
        .lock()
        .ok()
        .and_then(|t| *t)
        .is_some_and(|t| !reopen_after_blur(t.elapsed().as_millis() as u64));
    if just_closed {
        return; // the same click that took focus away from it
    }
    if let Err(e) = show(app, icon) {
        log::warn!("could not open the tray flyout: {e}");
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            if let Ok(mut t) = app.state::<ShellState>().flyout_hidden_at.lock() {
                *t = Some(Instant::now());
            }
        }
    }
}

fn build(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=flyout".into()))
        .title("Pyxee")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .shadow(true)
        .build()?;
    let _ = crate::window::apply_backdrop(&w);
    Ok(w)
}

/// The monitors as `pyxee_shell` sees them, with each one's scale factor.
pub(super) fn monitors(app: &AppHandle) -> Vec<(Monitor, f64)> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            let wa = m.work_area();
            (
                Monitor {
                    area: Rect::new(p.x, p.y, s.width as i32, s.height as i32),
                    work: Rect::new(
                        wa.position.x,
                        wa.position.y,
                        wa.size.width as i32,
                        wa.size.height as i32,
                    ),
                },
                m.scale_factor(),
            )
        })
        .collect()
}

fn show(app: &AppHandle, icon: tauri::Rect) -> tauri::Result<()> {
    let w = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => build(app)?,
    };
    let (pos, size) = (
        icon.position.to_physical::<f64>(1.0),
        icon.size.to_physical::<f64>(1.0),
    );
    let icon_rect = Rect::new(
        pos.x as i32,
        pos.y as i32,
        size.width as i32,
        size.height as i32,
    );
    let all = monitors(app);
    let plain: Vec<Monitor> = all.iter().map(|(m, _)| *m).collect();
    let scale = monitor_at(&plain, icon_rect.center())
        .and_then(|m| all.iter().find(|(x, _)| x == m))
        .map_or(1.0, |(_, s)| *s);
    let px = (
        (SIZE.0 * scale).round() as i32,
        (SIZE.1 * scale).round() as i32,
    );
    let (x, y) = flyout_position(icon_rect, &plain, px, (MARGIN * scale).round() as i32);
    w.set_size(PhysicalSize::new(px.0 as u32, px.1 as u32))?;
    w.set_position(PhysicalPosition::new(x, y))?;
    w.show()?;
    w.set_focus()?;
    Ok(())
}
