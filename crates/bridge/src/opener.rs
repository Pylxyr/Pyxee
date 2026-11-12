//! Starts the system's own programs: the file manager for a folder, the default browser for an address.

use std::process::{Command, Stdio};

/// What the "Open release page" button opens. A fixed address, so the page can never ask for another one.
pub const RELEASES_URL: &str = "https://github.com/Pylxyr/Pyxee/releases/latest";

/// Starts the program without waiting for it, and reaps it when it ends: a child that is never waited for stays in the
/// process table as a zombie until Pyxee quits (`xdg-open` ends within moments, so every use would leave one).
/// Its output is dropped so a chatty `xdg-open` cannot write into the terminal Pyxee was started from.
pub fn spawn_reaped(mut command: Command) -> std::io::Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Opens the address in the default browser. On Windows that is the shell's own address handler, started directly
/// (no `cmd`, so the `&` in the address is safe); `explorer <address>` can open a File Explorer window instead.
pub fn open_url(url: &str) -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut c = Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        Command::new("xdg-open")
    };
    command.arg(url);
    spawn_reaped(command).map_err(|e| format!("could not start the browser: {e}"))
}
