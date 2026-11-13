//! Release packaging beyond building the app: the staged app folder, the portable zip, the installer's file list and
//! code signing.
//!
//! Everything that decides something (names, what goes in the folder, what must not, the signtool command line, the
//! uninstall list) is a plain function with a test and runs on any platform. The calls that need Windows (signtool,
//! makensis, PowerShell) are thin and live in `main.rs`.
//!
//! Signing is driven by environment variables so that no secret is ever written to a file in the repository, to an
//! installer script, or to a command line that is printed:
//!
//! | Variable | Meaning |
//! |---|---|
//! | `PYXEE_SIGN_CERT_FILE` | path to a `.pfx` code-signing certificate (with `PYXEE_SIGN_CERT_PASSWORD`) |
//! | `PYXEE_SIGN_THUMBPRINT` | or: SHA-1 thumbprint of a certificate already in the Windows certificate store |
//! | `PYXEE_SIGN_TIMESTAMP_URL` | RFC 3161 timestamp server (default DigiCert's), so signatures outlive the certificate |
//! | `PYXEE_SIGNTOOL` | path to `signtool.exe` (default: newest in the Windows SDK, then PATH) |
//! | `PYXEE_REQUIRE_SIGNING` | `1`: a build without a certificate is an error instead of an unsigned build |

use std::io::Write;
use std::path::{Path, PathBuf};

pub type Res<T> = Result<T, String>;

pub const DEFAULT_TIMESTAMP_URL: &str = "http://timestamp.digicert.com";
pub const PORTABLE_MARKER: &str = "portable.txt";

// ---------------------------------------------------------------------------------------- signing

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cert {
    File {
        path: PathBuf,
        password: Option<String>,
    },
    Thumbprint(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signing {
    pub cert: Cert,
    pub timestamp_url: String,
}

fn set(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl Signing {
    /// Reads the signing setup. `Ok(None)`: nothing is configured (an unsigned build). `Err`: configured wrongly.
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Res<Option<Signing>> {
        let file = set(get("PYXEE_SIGN_CERT_FILE"));
        let thumb = set(get("PYXEE_SIGN_THUMBPRINT"));
        let cert = match (file, thumb) {
            (None, None) => return Ok(None),
            (Some(_), Some(_)) => {
                return Err("set PYXEE_SIGN_CERT_FILE or PYXEE_SIGN_THUMBPRINT, not both".into())
            }
            (Some(path), None) => Cert::File {
                path: PathBuf::from(path),
                password: get("PYXEE_SIGN_CERT_PASSWORD").filter(|p| !p.is_empty()),
            },
            (None, Some(t)) => {
                let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
                if t.len() != 40 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(
                        "PYXEE_SIGN_THUMBPRINT must be the 40 hex digits of the certificate's SHA-1 thumbprint"
                            .into(),
                    );
                }
                Cert::Thumbprint(t.to_ascii_uppercase())
            }
        };
        let timestamp_url =
            set(get("PYXEE_SIGN_TIMESTAMP_URL")).unwrap_or_else(|| DEFAULT_TIMESTAMP_URL.into());
        if !timestamp_url.starts_with("http://") && !timestamp_url.starts_with("https://") {
            return Err("PYXEE_SIGN_TIMESTAMP_URL must be an http(s) URL".into());
        }
        Ok(Some(Signing {
            cert,
            timestamp_url,
        }))
    }

    /// The `signtool sign` arguments for `file`: SHA-256 file digest and SHA-256 RFC 3161 timestamp.
    pub fn signtool_args(&self, file: &Path) -> Vec<String> {
        let mut a: Vec<String> = [
            "sign",
            "/fd",
            "sha256",
            "/tr",
            &self.timestamp_url,
            "/td",
            "sha256",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        match &self.cert {
            Cert::File { path, password } => {
                a.push("/f".into());
                a.push(path.to_string_lossy().into_owned());
                if let Some(p) = password {
                    a.push("/p".into());
                    a.push(p.clone());
                }
            }
            Cert::Thumbprint(t) => {
                a.push("/sha1".into());
                a.push(t.clone());
            }
        }
        a.push(file.to_string_lossy().into_owned());
        a
    }
}

/// The same arguments with the certificate password hidden, for anything that is printed or logged.
pub fn redacted(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut hide = false;
    for a in args {
        out.push(if hide { "********".into() } else { a.clone() });
        hide = a.eq_ignore_ascii_case("/p");
    }
    out
}

/// Orders Windows SDK folder names such as `10.0.22621.0` numerically (so `10.0.9` < `10.0.22621.0`).
pub fn sdk_version_key(name: &str) -> Vec<u64> {
    name.split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// The newest SDK folder name that looks like a version.
pub fn newest_sdk(names: &[String]) -> Option<&String> {
    names
        .iter()
        .filter(|n| n.starts_with("10.") && sdk_version_key(n).len() >= 3)
        .max_by_key(|n| sdk_version_key(n))
}

/// What to say when a build ends up unsigned.
pub fn unsigned_notice(required: bool) -> Result<&'static str, &'static str> {
    if required {
        Err("signing is required (PYXEE_REQUIRE_SIGNING=1) but no certificate is configured: set PYXEE_SIGN_CERT_FILE (+ PYXEE_SIGN_CERT_PASSWORD) or PYXEE_SIGN_THUMBPRINT")
    } else {
        Ok("not signed (no PYXEE_SIGN_CERT_FILE / PYXEE_SIGN_THUMBPRINT): Windows SmartScreen will warn on first run")
    }
}

// ---------------------------------------------------------------------------------------- the app folder

/// What the app is called on disk. The Flutter runner builds `pyxee_ui.exe` (`BINARY_NAME` in
/// `ui_flutter/windows/CMakeLists.txt`); the staged folder renames it, because the updater, the shortcuts and the
/// release notes all say `Pyxee.exe`.
pub const APP_EXE: &str = "Pyxee.exe";
pub const FLUTTER_EXE: &str = "pyxee_ui.exe";
pub const YTDLP_EXE: &str = "yt-dlp.exe";
pub const BRIDGE_DLL: &str = "pyxee_bridge.dll";
pub const FLUTTER_DLL: &str = "flutter_windows.dll";
/// The Visual C++ runtime, copied next to the program (Flutter's documentation: a PC without the redistributable
/// needs them there).
pub const VC_RUNTIME_DLLS: [&str; 3] = ["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"];
/// Where `flutter build windows --release` leaves the app (older Flutter versions leave out the `x64`).
pub const FLUTTER_RELEASE_DIRS: [&str; 2] = [
    "ui_flutter/build/windows/x64/runner/Release",
    "ui_flutter/build/windows/runner/Release",
];
/// Licence texts and notices that ship with every build: where they are in the repository, and their name next to
/// the program.
pub const NOTICE_FILES: [(&str, &str); 4] = [
    ("licenses/THIRD_PARTY.txt", "THIRD_PARTY_NOTICES.txt"),
    ("licenses/FFmpeg-LGPL-2.1.txt", "FFmpeg-LGPL-2.1.txt"),
    ("licenses/GPL-3.0.txt", "GPL-3.0.txt"),
    (
        "licenses/yt-dlp-THIRD_PARTY_LICENSES.txt",
        "yt-dlp-THIRD_PARTY_LICENSES.txt",
    ),
];

/// The installer's file name. The in-app updater picks the release asset that ends in `-setup.exe`.
pub fn installer_name(version: &str) -> String {
    format!("Pyxee_{version}_x64-setup.exe")
}

/// `0.5.2` or `0.5.2-rc1` as the four numbers a Windows file version needs: `0.5.2.0`.
pub fn numeric_version(version: &str) -> String {
    let base = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts: Vec<&str> = base.split('.').collect();
    while parts.len() < 4 {
        parts.push("0");
    }
    parts.truncate(4);
    parts.join(".")
}

/// A file Pyxee itself writes into its data folder. A portable copy keeps that folder next to the program, and it
/// is also where Flutter keeps `app.so`, `icudtl.dat` and `flutter_assets` (the two never use the same names), so
/// what is refused is what the user would have made, never the folder as such. `rel` is relative to the app folder,
/// with forward slashes.
pub fn is_user_data(rel: &str) -> bool {
    let Some(rest) = rel.strip_prefix("data/") else {
        return false;
    };
    let first = rest.split('/').next().unwrap_or(rest);
    first.starts_with("library.db")
        || matches!(
            first,
            "shell.json" | "session.json" | "ui_prefs.json" | ".env" | "logs" | "tools"
        )
        || first.ends_with(".tmp")
}

/// Checks the file list of an app folder (`names` relative to it, forward slashes): everything the program needs is
/// there, no ffmpeg program or libav DLL slipped in (libav is linked in statically; the licence text is meant to
/// be there), no user data, and `portable.txt` is there exactly when the folder is the portable one.
pub fn check_app_files(names: &[String], portable: bool) -> Res<()> {
    for n in names {
        let file = n.rsplit('/').next().unwrap_or(n);
        if is_ffmpeg_binary(file) {
            return Err(format!(
                "{n}: an ffmpeg program or libav DLL, but libav is linked in statically"
            ));
        }
        if is_user_data(n) {
            return Err(format!("{n}: the user's data must never be shipped"));
        }
    }
    let mut required: Vec<&str> = vec![
        APP_EXE,
        YTDLP_EXE,
        FLUTTER_DLL,
        BRIDGE_DLL,
        "data/icudtl.dat",
    ];
    required.extend(VC_RUNTIME_DLLS);
    required.extend(NOTICE_FILES.iter().map(|(_, name)| *name));
    if portable {
        required.extend([PORTABLE_MARKER, "README.txt"]);
    }
    for want in required {
        if !names.iter().any(|n| n == want) {
            return Err(format!("{want} is missing"));
        }
    }
    if !names.iter().any(|n| n.starts_with("data/flutter_assets/")) {
        return Err("data/flutter_assets is missing (is this a Flutter release build?)".into());
    }
    if !portable && names.iter().any(|n| n == PORTABLE_MARKER) {
        return Err(format!(
            "{PORTABLE_MARKER} must not be in the installed copy: it would turn portable mode on"
        ));
    }
    Ok(())
}

/// An ffmpeg program or a libav DLL.
pub fn is_ffmpeg_binary(file_name: &str) -> bool {
    let n = file_name.to_ascii_lowercase();
    if matches!(n.as_str(), "ffmpeg.exe" | "ffprobe.exe" | "ffplay.exe") {
        return true;
    }
    let Some(stem) = n.strip_suffix(".dll") else {
        return false;
    };
    let stem = stem.split('-').next().unwrap_or(stem);
    matches!(stem, "avcodec" | "avformat" | "avutil" | "swresample")
}

/// Every file under `dir`, as `(name relative to dir with forward slashes, full path)`, sorted by name so the same
/// folder always gives the same list (and the same zip).
pub fn collect_files(dir: &Path) -> Res<Vec<(String, PathBuf)>> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Res<()> {
        let rd =
            std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out)?;
            } else {
                let rel = path
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((rel, path));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The NSIS lines that remove exactly what the installer put in `$INSTDIR`: every file, then every folder that held
/// one, deepest first, and finally the install folder itself if nothing else is in it. The uninstaller never deletes
/// a folder recursively, so an install folder somebody pointed at `D:\Music` loses only Pyxee's own files.
pub fn uninstall_script(files: &[String]) -> Res<String> {
    let mut dirs: Vec<String> = Vec::new();
    let mut out = String::new();
    let mut sorted: Vec<&String> = files.iter().collect();
    sorted.sort();
    for f in &sorted {
        if f.contains(['"', '$', '\r', '\n']) {
            return Err(format!("{f}: a file name NSIS cannot take as it is"));
        }
        let win = f.replace('/', "\\");
        out.push_str(&format!("Delete \"$INSTDIR\\{win}\"\r\n"));
        let mut parts: Vec<&str> = f.split('/').collect();
        parts.pop();
        while !parts.is_empty() {
            let d = parts.join("\\");
            if !dirs.contains(&d) {
                dirs.push(d);
            }
            parts.pop();
        }
    }
    dirs.sort_by(|a, b| {
        b.matches('\\')
            .count()
            .cmp(&a.matches('\\').count())
            .then(a.cmp(b))
    });
    for d in dirs {
        out.push_str(&format!("RMDir \"$INSTDIR\\{d}\"\r\n"));
    }
    out.push_str("RMDir \"$INSTDIR\"\r\n");
    Ok(out)
}

// ---------------------------------------------------------------------------------------- portable

pub fn portable_folder_name(version: &str) -> String {
    format!("Pyxee_{version}_x64-portable")
}

pub fn portable_zip_name(version: &str) -> String {
    format!("{}.zip", portable_folder_name(version))
}

pub const PORTABLE_MARKER_BODY: &str = "This file turns on Pyxee's portable mode: the library, settings and logs are kept in the data folder next to Pyxee.exe. Delete it to use the normal per-user folder.\r\n";

pub const PORTABLE_README: &str = "Pyxee (portable)\r\n\
\r\n\
Run Pyxee.exe. Nothing is installed and nothing is written outside this folder:\r\n\
your library, settings and logs are kept in the data folder next to Pyxee.exe\r\n\
(it exists because of the file portable.txt; delete portable.txt to use the\r\n\
normal %LOCALAPPDATA% folder instead).\r\n\
\r\n\
Do not delete or empty the data folder: it also holds the files Pyxee's window\r\n\
needs to start (app.so, icudtl.dat and flutter_assets).\r\n\
\r\n\
To update: download the new zip from https://github.com/Pylxyr/Pyxee/releases and\r\n\
unpack it over this folder. Your library and settings are not in the zip, so they\r\n\
stay. Settings > Updates still updates yt-dlp, into data\\tools.\r\n\
\r\n\
Licences: THIRD_PARTY_NOTICES.txt, FFmpeg-LGPL-2.1.txt,\r\n\
GPL-3.0.txt and yt-dlp-THIRD_PARTY_LICENSES.txt (yt-dlp.exe is a GPLv3+ combined work).\r\n";

/// Checks the entry names of a finished zip (`Folder/name`): everything is inside the one top folder and the app
/// folder inside passes `check_app_files` as a portable one.
pub fn check_portable_entries(top: &str, entries: &[String]) -> Res<()> {
    let prefix = format!("{top}/");
    let mut inside = Vec::with_capacity(entries.len());
    for e in entries {
        let Some(rest) = e.strip_prefix(&prefix) else {
            return Err(format!("{e} is not inside {top}/"));
        };
        inside.push(rest.to_string());
    }
    check_app_files(&inside, true).map_err(|e| format!("{top}: {e}"))
}

/// Where a zip entry's bytes come from.
pub enum Source {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

/// Writes a zip with every file under `top/`, forward slashes, deflate, a fixed timestamp (so the same inputs give
/// the same bytes). `files` are `(name inside the folder, source)`.
pub fn write_zip(out: &Path, top: &str, files: &[(String, Source)]) -> Res<()> {
    let tmp = out.with_extension("zip.part");
    let result = (|| -> Res<()> {
        let file = std::fs::File::create(&tmp)
            .map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (name, source) in files {
            zip.start_file(format!("{top}/{name}"), options)
                .map_err(|e| format!("zip: {e}"))?;
            match source {
                Source::Bytes(bytes) => zip.write_all(bytes).map_err(|e| format!("zip: {e}"))?,
                Source::Path(path) => {
                    let mut f = std::fs::File::open(path)
                        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
                    std::io::copy(&mut f, &mut zip).map_err(|e| format!("zip: {e}"))?;
                }
            }
        }
        zip.finish().map_err(|e| format!("zip: {e}"))?;
        Ok(())
    })();
    match result {
        Ok(()) => std::fs::rename(&tmp, out)
            .map_err(|e| format!("cannot move into place {}: {e}", out.display())),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// The entry names of a zip, in order.
pub fn zip_names(path: &Path) -> Res<Vec<String>> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("not a zip: {e}"))?;
    Ok((0..archive.len())
        .filter_map(|i| {
            let f = archive.by_index(i).ok()?;
            let name = f.name().ok()?.into_owned();
            Some(name)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn nothing_configured_is_an_unsigned_build_not_an_error() {
        assert_eq!(Signing::from_env(env(&[])).unwrap(), None);
        assert_eq!(
            Signing::from_env(env(&[("PYXEE_SIGN_CERT_FILE", "  ")])).unwrap(),
            None,
            "blank counts as unset (an empty secret in CI)"
        );
    }

    #[test]
    fn a_certificate_file_with_password_and_the_default_timestamp_server() {
        let s = Signing::from_env(env(&[
            ("PYXEE_SIGN_CERT_FILE", r"C:\certs\pyxee.pfx"),
            ("PYXEE_SIGN_CERT_PASSWORD", "hunter2"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(s.timestamp_url, DEFAULT_TIMESTAMP_URL);
        let args = s.signtool_args(Path::new(r"D:\out\Pyxee.exe"));
        assert_eq!(
            args,
            [
                "sign",
                "/fd",
                "sha256",
                "/tr",
                DEFAULT_TIMESTAMP_URL,
                "/td",
                "sha256",
                "/f",
                r"C:\certs\pyxee.pfx",
                "/p",
                "hunter2",
                r"D:\out\Pyxee.exe"
            ]
        );
    }

    #[test]
    fn a_store_certificate_is_chosen_by_thumbprint_and_needs_no_password() {
        let t = "ab cd ef 01 23 45 67 89 ab cd ef 01 23 45 67 89 ab cd ef 01";
        let s = Signing::from_env(env(&[("PYXEE_SIGN_THUMBPRINT", t)]))
            .unwrap()
            .unwrap();
        let args = s.signtool_args(Path::new("a.exe"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "/sha1" && w[1] == "ABCDEF0123456789ABCDEF0123456789ABCDEF01"));
        assert!(!args.contains(&"/p".to_string()));
    }

    #[test]
    fn wrong_setups_are_refused_with_a_reason() {
        let both = Signing::from_env(env(&[
            ("PYXEE_SIGN_CERT_FILE", "a.pfx"),
            ("PYXEE_SIGN_THUMBPRINT", &"a".repeat(40)),
        ]));
        assert!(both.unwrap_err().contains("not both"));
        let short = Signing::from_env(env(&[("PYXEE_SIGN_THUMBPRINT", "1234")]));
        assert!(short.unwrap_err().contains("40 hex"));
        let url = Signing::from_env(env(&[
            ("PYXEE_SIGN_CERT_FILE", "a.pfx"),
            ("PYXEE_SIGN_TIMESTAMP_URL", "ftp://x"),
        ]));
        assert!(url.unwrap_err().contains("http"));
    }

    #[test]
    fn the_password_never_appears_in_what_is_printed() {
        let s = Signing::from_env(env(&[
            ("PYXEE_SIGN_CERT_FILE", "a.pfx"),
            ("PYXEE_SIGN_CERT_PASSWORD", "hunter2"),
        ]))
        .unwrap()
        .unwrap();
        let shown = redacted(&s.signtool_args(Path::new("a.exe"))).join(" ");
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("/p ********"));
    }

    #[test]
    fn the_newest_sdk_is_picked_by_number_not_by_text() {
        let names: Vec<String> = [
            "10.0.19041.0",
            "10.0.9200.0",
            "10.0.22621.0",
            "NETFXSDK",
            "x64",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(newest_sdk(&names).map(String::as_str), Some("10.0.22621.0"));
        assert_eq!(newest_sdk(&[]), None);
    }

    #[test]
    fn an_unsigned_build_is_allowed_unless_signing_is_required() {
        assert!(unsigned_notice(false).unwrap().contains("SmartScreen"));
        assert!(unsigned_notice(true)
            .unwrap_err()
            .contains("PYXEE_SIGN_CERT_FILE"));
    }

    #[test]
    fn the_installer_is_named_the_way_the_updater_looks_for_it() {
        let name = installer_name("0.2.0-rc1");
        assert_eq!(name, "Pyxee_0.2.0-rc1_x64-setup.exe");
        assert!(name.ends_with("-setup.exe"));
    }

    #[test]
    fn a_version_becomes_four_numbers_for_the_file_properties() {
        assert_eq!(numeric_version("0.5.2"), "0.5.2.0");
        assert_eq!(numeric_version("0.5.2-rc1"), "0.5.2.0");
        assert_eq!(numeric_version("1.0"), "1.0.0.0");
    }

    #[test]
    fn portable_names_carry_the_version() {
        assert_eq!(portable_folder_name("0.2.0"), "Pyxee_0.2.0_x64-portable");
        assert_eq!(
            portable_zip_name("0.2.0-rc1"),
            "Pyxee_0.2.0-rc1_x64-portable.zip"
        );
    }

    #[test]
    fn libav_programs_and_dlls_are_recognised() {
        for n in [
            "ffmpeg.exe",
            "FFprobe.EXE",
            "avcodec-61.dll",
            "avformat.dll",
            "swresample-5.dll",
        ] {
            assert!(is_ffmpeg_binary(n), "{n}");
        }
        for n in [
            "Pyxee.exe",
            "yt-dlp.exe",
            "FFmpeg-LGPL-2.1.txt",
            "avatar.dll",
            "flutter_windows.dll",
        ] {
            assert!(!is_ffmpeg_binary(n), "{n}");
        }
    }

    #[test]
    fn only_what_pyxee_writes_is_user_data_not_flutters_own_data_folder() {
        for n in [
            "data/library.db",
            "data/library.db-wal",
            "data/shell.json",
            "data/session.json",
            "data/ui_prefs.json",
            "data/.env",
            "data/logs/pyxee.log",
            "data/tools/yt-dlp.exe",
            "data/shell.json.tmp",
        ] {
            assert!(is_user_data(n), "{n}");
        }
        for n in [
            "data/app.so",
            "data/icudtl.dat",
            "data/flutter_assets/AssetManifest.bin",
            "Pyxee.exe",
            "tools/x",
        ] {
            assert!(!is_user_data(n), "{n}");
        }
    }

    /// A file list that passes: what the staged folder holds, plus the portable extras when asked.
    fn good(portable: bool) -> Vec<String> {
        let mut v: Vec<String> = vec![
            APP_EXE.into(),
            YTDLP_EXE.into(),
            FLUTTER_DLL.into(),
            BRIDGE_DLL.into(),
            "data/icudtl.dat".into(),
            "data/app.so".into(),
            "data/flutter_assets/AssetManifest.bin".into(),
            "tray_manager_plugin.dll".into(),
        ];
        v.extend(VC_RUNTIME_DLLS.iter().map(|s| s.to_string()));
        v.extend(NOTICE_FILES.iter().map(|(_, n)| n.to_string()));
        if portable {
            v.push(PORTABLE_MARKER.into());
            v.push("README.txt".into());
        }
        v
    }

    #[test]
    fn a_complete_app_folder_passes() {
        assert!(check_app_files(&good(false), false).is_ok());
        assert!(check_app_files(&good(true), true).is_ok());
    }

    #[test]
    fn the_notices_that_go_with_yt_dlp_and_ffmpeg_are_required() {
        for n in [
            "GPL-3.0.txt",
            "yt-dlp-THIRD_PARTY_LICENSES.txt",
            "FFmpeg-LGPL-2.1.txt",
        ] {
            let mut files = good(false);
            files.retain(|f| f != n);
            assert!(
                check_app_files(&files, false).unwrap_err().contains(n),
                "{n}"
            );
        }
        assert!(PORTABLE_README.contains("GPL-3.0.txt"));
    }

    #[test]
    fn an_app_folder_that_is_incomplete_or_wrong_is_refused() {
        for (drop, why) in [
            (YTDLP_EXE, "yt-dlp.exe"),
            (BRIDGE_DLL, "pyxee_bridge.dll"),
            ("vcruntime140.dll", "vcruntime140.dll"),
            ("data/icudtl.dat", "icudtl.dat"),
        ] {
            let mut files = good(false);
            files.retain(|f| f != drop);
            assert!(
                check_app_files(&files, false).unwrap_err().contains(why),
                "{drop}"
            );
        }
        let mut no_assets = good(false);
        no_assets.retain(|f| !f.starts_with("data/flutter_assets/"));
        assert!(check_app_files(&no_assets, false)
            .unwrap_err()
            .contains("flutter_assets"));
        let mut ff = good(false);
        ff.push("avcodec-61.dll".into());
        assert!(check_app_files(&ff, false).unwrap_err().contains("libav"));
        let mut data = good(false);
        data.push("data/library.db".into());
        assert!(check_app_files(&data, false)
            .unwrap_err()
            .contains("user's data"));
    }

    #[test]
    fn the_portable_marker_belongs_in_the_portable_zip_only() {
        let mut installed = good(false);
        installed.push(PORTABLE_MARKER.into());
        assert!(check_app_files(&installed, false)
            .unwrap_err()
            .contains("portable mode"));
        let mut no_marker = good(true);
        no_marker.retain(|f| f != PORTABLE_MARKER);
        assert!(check_app_files(&no_marker, true)
            .unwrap_err()
            .contains(PORTABLE_MARKER));
    }

    #[test]
    fn a_portable_zip_must_keep_everything_inside_its_one_folder() {
        let inside: Vec<String> = good(true).iter().map(|n| format!("P/{n}")).collect();
        assert!(check_portable_entries("P", &inside).is_ok());
        let mut stray = inside.clone();
        stray.push("Other/x.txt".into());
        assert!(check_portable_entries("P", &stray)
            .unwrap_err()
            .contains("not inside"));
        let mut missing = inside;
        missing.retain(|e| !e.ends_with("yt-dlp.exe"));
        assert!(check_portable_entries("P", &missing)
            .unwrap_err()
            .contains("yt-dlp.exe"));
    }

    #[test]
    fn the_uninstall_list_deletes_files_then_folders_deepest_first() {
        let files: Vec<String> = [
            "Pyxee.exe",
            "data/app.so",
            "data/flutter_assets/fonts/a.otf",
            "data/flutter_assets/AssetManifest.bin",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let script = uninstall_script(&files).unwrap();
        let lines: Vec<&str> = script.lines().collect();
        assert!(lines.contains(&"Delete \"$INSTDIR\\data\\flutter_assets\\fonts\\a.otf\""));
        let rm = |d: &str| {
            lines
                .iter()
                .position(|l| *l == format!("RMDir \"$INSTDIR\\{d}\""))
        };
        let fonts = rm("data\\flutter_assets\\fonts").unwrap();
        let assets = rm("data\\flutter_assets").unwrap();
        let data = rm("data").unwrap();
        assert!(fonts < assets && assets < data, "{script}");
        let last_delete = lines.iter().rposition(|l| l.starts_with("Delete")).unwrap();
        assert!(last_delete < fonts, "all files go before any folder");
        assert_eq!(*lines.last().unwrap(), "RMDir \"$INSTDIR\"");
        assert!(!script.contains("/r"), "never a recursive delete");
    }

    #[test]
    fn a_file_name_nsis_cannot_take_is_an_error_not_a_broken_script() {
        assert!(uninstall_script(&["a$b.txt".to_string()]).is_err());
        assert!(uninstall_script(&["a\"b.txt".to_string()]).is_err());
    }

    #[test]
    fn collecting_files_gives_sorted_relative_names_with_forward_slashes() {
        let dir = std::env::temp_dir().join(format!("pyxee-xtask-collect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data/flutter_assets")).unwrap();
        std::fs::write(dir.join("b.txt"), "b").unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        std::fs::write(dir.join("data/flutter_assets/x.bin"), "x").unwrap();
        let names: Vec<String> = collect_files(&dir)
            .unwrap()
            .into_iter()
            .map(|f| f.0)
            .collect();
        assert_eq!(names, ["a.txt", "b.txt", "data/flutter_assets/x.bin"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_zip_is_written_atomically_under_one_folder_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("pyxee-xtask-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let on_disk = dir.join("from-disk.bin");
        std::fs::write(&on_disk, b"disk").unwrap();
        let out = dir.join("p.zip");
        let mut files: Vec<(String, Source)> = good(true)
            .into_iter()
            .map(|n| {
                let bytes = n.as_bytes().to_vec();
                (n, Source::Bytes(bytes))
            })
            .collect();
        files.push(("data/extra.bin".into(), Source::Path(on_disk)));
        write_zip(&out, "Pyxee_x", &files).unwrap();
        let names = zip_names(&out).unwrap();
        assert!(
            names
                .iter()
                .all(|n| n.starts_with("Pyxee_x/") && !n.contains('\\')),
            "{names:?}"
        );
        assert!(names.contains(&"Pyxee_x/data/extra.bin".to_string()));
        check_portable_entries("Pyxee_x", &names).unwrap();
        let first = std::fs::read(&out).unwrap();
        write_zip(&out, "Pyxee_x", &files).unwrap();
        assert_eq!(
            first,
            std::fs::read(&out).unwrap(),
            "the same inputs give the same bytes"
        );
        let mut left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["from-disk.bin", "p.zip"],
            "no .part file is left behind"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
