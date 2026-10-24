use crate::dto::BackdropDto;

/// `MARGINS` of the Win32 API; -1 on every side makes the whole window "glass" for the DWM.
#[cfg(windows)]
#[repr(C)]
struct Margins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

#[cfg(windows)]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmExtendFrameIntoClientArea(hwnd: *mut std::ffi::c_void, margins: *const Margins) -> i32;
}

/// Lets the system backdrop show through the whole window. A window with a native title bar gets that from
/// Windows; one without (the borderless main window) shows it only where the frame is extended into the page.
#[cfg(windows)]
fn extend_frame(window: &tauri::WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let margins = Margins {
        left: -1,
        right: -1,
        top: -1,
        bottom: -1,
    };
    // SAFETY: `hwnd` is the live window handle and `margins` outlives the call.
    let hr = unsafe { DwmExtendFrameIntoClientArea(hwnd.0.cast(), &margins) };
    if hr < 0 {
        log::warn!("could not extend the window frame for the backdrop: HRESULT {hr:#x}");
    } else {
        log::debug!("{} window: backdrop frame extended", window.label());
    }
}

/// Applies the Windows 11 Mica backdrop. Fails on Windows 10 (and anywhere Mica is unavailable),
/// in which case the UI keeps its solid background.
#[cfg(windows)]
pub fn apply_backdrop(window: &tauri::WebviewWindow) -> BackdropDto {
    // `None` follows the system dark/light setting.
    match window_vibrancy::apply_mica(window, None) {
        Ok(()) => {
            if window.is_decorated().is_ok_and(|decorated| !decorated) {
                extend_frame(window);
            }
            BackdropDto::Mica
        }
        Err(_) => BackdropDto::None,
    }
}

#[cfg(not(windows))]
pub fn apply_backdrop(_window: &tauri::WebviewWindow) -> BackdropDto {
    BackdropDto::None
}
