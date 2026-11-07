//! Without a D-Bus session bus the media controls must say so when they are attached, not panic later.
#![cfg(target_os = "linux")]

use souvlaki::{MediaControls, PlatformConfig};

#[test]
fn attach_reports_a_missing_session_bus() {
    // This file is its own test process with one test, so changing the environment is safe.
    std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
    std::env::set_var("XDG_RUNTIME_DIR", "/nonexistent");
    let mut controls = MediaControls::new(PlatformConfig {
        display_name: "Pyxee test",
        dbus_name: "pyxee_test_no_bus",
        desktop_entry: None,
        hwnd: None,
    })
    .unwrap();
    assert!(controls.attach(|_| {}).is_err());
}
