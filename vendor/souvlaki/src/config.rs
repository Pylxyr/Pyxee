use std::ffi::c_void;

/// OS-specific configuration needed to create media controls.
#[derive(Debug)]
pub struct PlatformConfig<'a> {
    /// The name to be displayed to the user. (*Required on Linux*)
    pub display_name: &'a str,
    /// Should follow [the D-Bus spec](https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus). (*Required on Linux*)
    pub dbus_name: &'a str,
    /// Pyxee change (Linux, MPRIS): the name of the application's .desktop file without `.desktop`, published as the
    /// `DesktopEntry` property so the desktop can show the app's icon and raise it. `None` publishes nothing useful.
    pub desktop_entry: Option<&'a str>,
    /// An HWND. (*Required on Windows*)
    pub hwnd: Option<*mut c_void>,
}
