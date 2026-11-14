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
    fn DwmSetWindowAttribute(
        hwnd: *mut std::ffi::c_void,
        attribute: u32,
        value: *const std::ffi::c_void,
        size: u32,
    ) -> i32;
}

/// `DWMWINDOWATTRIBUTE` values (Windows 11, build 22000 and later) for the window border, the title bar and its text.
#[cfg(windows)]
const DWMWA_BORDER_COLOR: u32 = 34;
#[cfg(windows)]
const DWMWA_CAPTION_COLOR: u32 = 35;
#[cfg(windows)]
const DWMWA_TEXT_COLOR: u32 = 36;
/// The attribute value that gives the colour back to the system.
#[cfg(windows)]
const DWMWA_COLOR_DEFAULT: u32 = 0xFFFF_FFFF;

/// A `#rrggbb` colour as a Win32 `COLORREF` (`0x00BBGGRR`); `None` for anything else.
pub fn colorref(hex: &str) -> Option<u32> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u32::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(byte(0)? | (byte(2)? << 8) | (byte(4)? << 16))
}

/// Colours the window's border, title bar and title bar text; `None` gives that one back to the system. Older
/// Windows versions refuse the attributes, which is fine: the window then keeps its usual look.
#[cfg(windows)]
pub fn set_tint(
    window: &tauri::WebviewWindow,
    border: Option<u32>,
    caption: Option<u32>,
    text: Option<u32>,
) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    for (attribute, colour) in [
        (DWMWA_BORDER_COLOR, border),
        (DWMWA_CAPTION_COLOR, caption),
        (DWMWA_TEXT_COLOR, text),
    ] {
        let value = colour.unwrap_or(DWMWA_COLOR_DEFAULT);
        // SAFETY: `hwnd` is the live window handle and `value` is a 4-byte COLORREF that outlives the call.
        let hr = unsafe {
            DwmSetWindowAttribute(
                hwnd.0.cast(),
                attribute,
                std::ptr::from_ref(&value).cast(),
                std::mem::size_of::<u32>() as u32,
            )
        };
        if hr < 0 {
            log::debug!("window colour {attribute} not applied: HRESULT {hr:#x}");
        }
    }
}

#[cfg(not(windows))]
pub fn set_tint(
    _window: &tauri::WebviewWindow,
    _border: Option<u32>,
    _caption: Option<u32>,
    _text: Option<u32>,
) {
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

#[cfg(test)]
mod tests {
    use super::colorref;

    #[test]
    fn a_hex_colour_becomes_a_colorref_with_red_in_the_low_byte() {
        assert_eq!(colorref("#102030"), Some(0x0030_2010));
        assert_eq!(colorref("#ffffff"), Some(0x00ff_ffff));
        assert_eq!(colorref("#000000"), Some(0));
    }

    #[test]
    fn anything_else_is_not_a_colour() {
        for bad in ["102030", "#12345", "#1234567", "#gg0000", "", "#é12345"] {
            assert_eq!(colorref(bad), None, "{bad}");
        }
    }
}
