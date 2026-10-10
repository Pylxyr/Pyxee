//! Finds the helper programs (yt-dlp, ffmpeg).
//!
//! Order: the environment override, next to the app (where the installer puts them), a `binaries`
//! folder there (portable layouts), the folder `cargo xtask tools` fills (development builds only),
//! then PATH.

use std::path::{Path, PathBuf};

const TRIPLE: &str = "x86_64-pc-windows-msvc";

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

pub fn locate(name: &str, env_var: &str) -> PathBuf {
    let app_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from));
    // Baked in only for debug builds: the source tree's binaries folder, where `cargo xtask tools` writes.
    let dev_dir =
        cfg!(debug_assertions).then(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries"));
    locate_in(name, std::env::var(env_var).ok(), app_dir, dev_dir)
}

fn first_file(dir: &Path, names: &[String]) -> Option<PathBuf> {
    names.iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

fn locate_in(
    name: &str,
    env_value: Option<String>,
    app_dir: Option<PathBuf>,
    dev_dir: Option<PathBuf>,
) -> PathBuf {
    if let Some(v) = env_value.filter(|v| !v.is_empty()) {
        return v.into();
    }
    // Installed sidecars are `<name>.exe`; the development folder keeps Tauri's `<name>-<triple>.exe`.
    let plain = [exe_name(name)];
    let sidecar = [format!("{name}-{TRIPLE}.exe")];
    if let Some(dir) = app_dir {
        if let Some(p) =
            first_file(&dir, &plain).or_else(|| first_file(&dir.join("binaries"), &plain))
        {
            return p;
        }
    }
    if let Some(dir) = dev_dir {
        if let Some(p) = first_file(&dir, &sidecar).or_else(|| first_file(&dir, &plain)) {
            return p;
        }
    }
    PathBuf::from(name) // left to the OS to find on PATH
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pyxee-tools-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_environment_override_always_wins() {
        let app = temp("env");
        std::fs::write(app.join(exe_name("yt-dlp")), b"x").unwrap();
        assert_eq!(
            locate_in("yt-dlp", Some("C:/mine/yt.exe".into()), Some(app), None),
            PathBuf::from("C:/mine/yt.exe")
        );
        assert_eq!(
            locate_in("yt-dlp", Some(String::new()), None, None),
            PathBuf::from("yt-dlp"),
            "an empty override is ignored"
        );
    }

    #[test]
    fn prefers_the_installed_sidecar_then_a_binaries_folder_then_the_dev_folder() {
        let app = temp("app");
        let dev = temp("dev");
        std::fs::write(dev.join(format!("yt-dlp-{TRIPLE}.exe")), b"x").unwrap();
        assert_eq!(
            locate_in("yt-dlp", None, Some(app.clone()), Some(dev.clone())),
            dev.join(format!("yt-dlp-{TRIPLE}.exe"))
        );

        std::fs::create_dir_all(app.join("binaries")).unwrap();
        std::fs::write(app.join("binaries").join(exe_name("yt-dlp")), b"x").unwrap();
        assert_eq!(
            locate_in("yt-dlp", None, Some(app.clone()), Some(dev.clone())),
            app.join("binaries").join(exe_name("yt-dlp"))
        );

        std::fs::write(app.join(exe_name("yt-dlp")), b"x").unwrap();
        assert_eq!(
            locate_in("yt-dlp", None, Some(app.clone()), Some(dev)),
            app.join(exe_name("yt-dlp"))
        );
    }

    #[test]
    fn falls_back_to_the_bare_name_so_path_lookup_still_works() {
        let empty = temp("empty");
        assert_eq!(
            locate_in("ffmpeg", None, Some(empty.clone()), Some(empty)),
            PathBuf::from("ffmpeg")
        );
        assert_eq!(
            locate_in("ffmpeg", None, None, None),
            PathBuf::from("ffmpeg")
        );
    }
}
