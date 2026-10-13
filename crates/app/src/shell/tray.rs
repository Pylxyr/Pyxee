//! The system tray icon: a menu, the tooltip, and a left click that opens the flyout.

use super::{flyout, mini, show_main, ShellState};
use pyxee_core::Command;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const TRAY_ID: &str = "main";

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let item = |id: &str, text: &str| MenuItem::with_id(app, id, text, true, None::<&str>);
    let menu = Menu::with_items(
        app,
        &[
            &item("show", "Open Pyxee")?,
            &PredefinedMenuItem::separator(app)?,
            &item("playpause", "Play / Pause")?,
            &item("previous", "Previous")?,
            &item("next", "Next")?,
            &PredefinedMenuItem::separator(app)?,
            &item("mini", "Mini player")?,
            &PredefinedMenuItem::separator(app)?,
            &item("quit", "Quit")?,
        ],
    )?;
    let icon = Image::from_bytes(include_bytes!("../../icons/tray-32.png"))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Pyxee")
        .menu(&menu)
        // The left click opens the flyout; the menu is on the right click.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "playpause" => super::command(app, Command::Toggle),
            "previous" => super::command(app, Command::Previous),
            "next" => super::command(app, Command::Next),
            "mini" => mini::toggle(app),
            "quit" => super::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } => flyout::toggle(tray.app_handle(), rect),
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                flyout::hide(tray.app_handle());
                show_main(tray.app_handle());
            }
            _ => {}
        })
        .build(app)?;
    let _ = app.state::<ShellState>(); // the tray is only useful with the shell state in place
    Ok(())
}

pub fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        // The Windows tooltip is limited to 127 characters.
        let t: String = text.chars().take(120).collect();
        if let Err(e) = tray.set_tooltip(Some(t)) {
            log::debug!("tray tooltip not set: {e}");
        }
    }
}
