//! Previous / play-pause / next as three small icons in the notification area, next to the main tray icon.
//! What they show is decided by `pyxee_shell::tray_controls`; this file only puts it on screen.
//!
//! Windows decides where tray icons sit and whether they start in the overflow flyout; an app cannot pin or order them.
//! The user drags them out of the overflow once (Settings > Personalization > Taskbar > Other system tray icons on
//! Windows 11) and Windows remembers. See docs/shell.md.

use super::{command, current_phase};
use pyxee_core::Command;
use pyxee_shell::progress::Phase;
use pyxee_shell::tray_controls::{Control, Glyph, CONTROLS};
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

fn icon(glyph: Glyph) -> tauri::Result<Image<'static>> {
    let bytes: &[u8] = match glyph {
        Glyph::Previous => include_bytes!("../../icons/tray-buttons/prev.png"),
        Glyph::Play => include_bytes!("../../icons/tray-buttons/play.png"),
        Glyph::Pause => include_bytes!("../../icons/tray-buttons/pause.png"),
        Glyph::Next => include_bytes!("../../icons/tray-buttons/next.png"),
    };
    Image::from_bytes(bytes)
}

fn press(app: &AppHandle, control: Control) {
    command(
        app,
        match control {
            Control::Previous => Command::Previous,
            Control::PlayPause => Command::Toggle,
            Control::Next => Command::Next,
        },
    );
}

fn build(app: &AppHandle, control: Control, phase: Phase) -> tauri::Result<()> {
    TrayIconBuilder::with_id(control.id())
        .icon(icon(control.glyph(phase))?)
        .tooltip(control.tooltip(phase))
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                press(tray.app_handle(), control);
            }
        })
        .build(app)?;
    Ok(())
}

/// Makes the notification area match the setting: the three icons exist when `enabled`, and none do otherwise.
pub fn sync(app: &AppHandle, enabled: bool) {
    if !enabled {
        for control in CONTROLS {
            // Dropping the returned icon removes it from the notification area.
            drop(app.remove_tray_by_id(control.id()));
        }
        return;
    }
    let (phase, _) = current_phase(app);
    for control in CONTROLS {
        if app.tray_by_id(control.id()).is_none() {
            if let Err(e) = build(app, control, phase) {
                log::warn!("could not create the {} tray button: {e}", control.id());
            }
        }
    }
}

/// Play becomes Pause (and back) when the transport changes. Cheap when the buttons are off: there is no icon to find.
pub fn update(app: &AppHandle, phase: Phase) {
    let control = Control::PlayPause;
    let Some(tray) = app.tray_by_id(control.id()) else {
        return;
    };
    match icon(control.glyph(phase)) {
        Ok(image) => {
            if let Err(e) = tray.set_icon(Some(image)) {
                log::debug!("tray button icon not set: {e}");
            }
        }
        Err(e) => log::debug!("tray button icon not decoded: {e}"),
    }
    if let Err(e) = tray.set_tooltip(Some(control.tooltip(phase))) {
        log::debug!("tray button tooltip not set: {e}");
    }
}
