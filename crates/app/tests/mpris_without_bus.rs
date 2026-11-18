//! Without a D-Bus session bus the media controls must say so when they are attached, not panic later.
#![cfg(target_os = "linux")]
// Pedantic lints this project does not follow, allowed once per crate root. (A workspace-level allow loses to the
// `-D clippy::pedantic` that is passed on the command line, so it has to be an attribute.) Each is a trade-off for
// application code: `# Errors` / `# Panics` sections and `#[must_use]` on internal items, long match-heavy functions,
// flag structs, short names in numeric code, and the casts between sample counts, milliseconds and pixels (their
// ranges are bounded by what the device and the files can hold) are not worth the noise.
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

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
