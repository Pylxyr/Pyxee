use crate::dto::BackdropDto;

/// Applies the Windows 11 Mica backdrop. Fails on Windows 10 (and anywhere Mica is unavailable),
/// in which case the UI keeps its solid background.
#[cfg(windows)]
pub fn apply_backdrop(window: &tauri::WebviewWindow) -> BackdropDto {
    // `None` follows the system dark/light setting.
    match window_vibrancy::apply_mica(window, None) {
        Ok(()) => BackdropDto::Mica,
        Err(_) => BackdropDto::None,
    }
}

#[cfg(not(windows))]
pub fn apply_backdrop(_window: &tauri::WebviewWindow) -> BackdropDto {
    BackdropDto::None
}
