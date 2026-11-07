//! The C interface the Flutter shell (`ui_flutter/`) loads. For now it only proves that the library loads and that
//! the core can be called through it.

/// Changes whenever an exported function changes incompatibly, so the shell can refuse a library that does not match.
pub const ABI_VERSION: u32 = 1;

#[no_mangle]
pub extern "C" fn pyxee_bridge_abi_version() -> u32 {
    ABI_VERSION
}

/// Runs a real call into the core (a new player, its session serialised). 1 when it worked, 0 when it did not.
#[no_mangle]
pub extern "C" fn pyxee_bridge_core_probe() -> u32 {
    let player = pyxee_core::Player::new(1);
    u32::from(serde_json::to_string(&player.persisted()).is_ok())
}
