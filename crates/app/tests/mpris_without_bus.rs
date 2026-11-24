//! Without a D-Bus session bus the media controls must say so when they are attached, not panic later.
#![cfg(target_os = "linux")]

use souvlaki::{MediaControls, PlatformConfig};

#[test]
fn attach_reports_a_missing_session_bus() {
    // SAFETY: the only test of this process, and nothing has started a thread that reads the environment.
    unsafe { std::env::remove_var("DBUS_SESSION_BUS_ADDRESS") };
    // SAFETY: as above.
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/nonexistent") };
    let mut controls = MediaControls::new(PlatformConfig {
        display_name: "Pyxee test",
        dbus_name: "pyxee_test_no_bus",
        desktop_entry: None,
        hwnd: None,
    })
    .unwrap();
    assert!(controls.attach(|_| {}).is_err());
}
