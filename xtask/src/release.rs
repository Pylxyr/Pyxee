//! Release packaging beyond the installer: the portable zip and code signing.
//!
//! Everything that decides something (names, what goes in the zip, what must not, the signtool command line, the
//! Tauri signing config) is a plain function with a test and runs on any platform. The calls that need Windows
//! (signtool, PowerShell) are thin and live in `main.rs`.
//!
//! Signing is driven by environment variables so that no secret is ever written to a file in the repository, to a
//! Tauri config, or to a command line that is printed:
//!
//! | Variable | Meaning |
//! |---|---|
//! | `PYXEE_SIGN_CERT_FILE` | path to a `.pfx` code-signing certificate (with `PYXEE_SIGN_CERT_PASSWORD`) |
//! | `PYXEE_SIGN_THUMBPRINT` | or: SHA-1 thumbprint of a certificate already in the Windows certificate store |
//! | `PYXEE_SIGN_TIMESTAMP_URL` | RFC 3161 timestamp server (default DigiCert's), so signatures outlive the certificate |
//! | `PYXEE_SIGNTOOL` | path to `signtool.exe` (default: newest in the Windows SDK, then PATH) |
//! | `PYXEE_REQUIRE_SIGNING` | `1`: a build without a certificate is an error instead of an unsigned build |

use serde_json::{json, Value};
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

/// The Tauri config that makes `tauri build` sign everything it produces (the program, the uninstaller, the
/// installers) by calling `<xtask> sign-file <file>`. The secret is read by that call from the environment; nothing
/// secret is in this config. `base` is `tauri.bundle.conf.json`, whose relative paths stay relative to `crates/app`.
pub fn signed_tauri_config(base: &Value, xtask_exe: &Path) -> Value {
    let mut conf = base.clone();
    merge(
        &mut conf,
        &json!({"bundle": {"windows": {"signCommand": {
            "cmd": xtask_exe.to_string_lossy(),
            "args": ["sign-file", "%1"],
        }}}}),
    );
    conf
}

/// Recursive JSON merge: objects are merged key by key, anything else is replaced.
pub fn merge(into: &mut Value, patch: &Value) {
    match (into, patch) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

/// What to say when a build ends up unsigned.
pub fn unsigned_notice(required: bool) -> Result<&'static str, &'static str> {
    if required {
        Err("signing is required (PYXEE_REQUIRE_SIGNING=1) but no certificate is configured: set PYXEE_SIGN_CERT_FILE (+ PYXEE_SIGN_CERT_PASSWORD) or PYXEE_SIGN_THUMBPRINT")
    } else {
        Ok("not signed (no PYXEE_SIGN_CERT_FILE / PYXEE_SIGN_THUMBPRINT): Windows SmartScreen will warn on first run")
    }
}

/// The installer formats to build. A pre-release version such as `0.1.1-rc1` cannot be an MSI (its version numbers
/// must be numeric), so candidates are NSIS only; a final release gets both.
pub fn bundles_for(version: &str) -> &'static str {
    if version.contains('-') {
        "nsis"
    } else {
        "nsis,msi"
    }
}

// ---------------------------------------------------------------------------------------- portable

pub fn portable_folder_name(version: &str) -> String {
    format!("Pyxee_{version}_x64-portable")
}

pub fn portable_zip_name(version: &str) -> String {
    format!("{}.zip", portable_folder_name(version))
}

pub const PORTABLE_README: &str = "Pyxee (portable)\r\n\
\r\n\
Run Pyxee.exe. Nothing is installed and nothing is written outside this folder:\r\n\
your library, settings and logs are kept in the data folder next to Pyxee.exe\r\n\
(it exists because of the file portable.txt; delete portable.txt to use the\r\n\
normal %LOCALAPPDATA% folder instead).\r\n\
\r\n\
Needs the Microsoft Edge WebView2 runtime (already part of Windows 11 and of\r\n\
current Windows 10).\r\n\
\r\n\
To update: download the new zip from https://github.com/Pylxyr/Pyxee/releases and\r\n\
unpack it over this folder. The data folder is not in the zip, so it stays.\r\n\
Settings > Updates still updates yt-dlp, into data\\tools.\r\n\
\r\n\
Licences: THIRD_PARTY_NOTICES.txt, FFmpeg-LGPL-2.1.txt,\r\n\
GPL-3.0.txt and
yt-dlp-THIRD_PARTY_LICENSES.txt (yt-dlp.exe is a GPLv3+ combined work).\r\n";

/// A file that goes in the portable folder: where it comes from and its name inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub from: Option<PathBuf>,
    pub name: &'static str,
}

/// The portable folder's contents. `from: None` entries are written by the packer (the marker and the readme).
pub fn portable_entries() -> Vec<Entry> {
    vec![
        Entry {
            from: Some("target/release/pyxee-app.exe".into()),
            name: "Pyxee.exe",
        },
        Entry {
            from: Some("crates/app/binaries/yt-dlp-x86_64-pc-windows-msvc.exe".into()),
            name: "yt-dlp.exe",
        },
        Entry {
            from: Some("crates/app/licenses/THIRD_PARTY.txt".into()),
            name: "THIRD_PARTY_NOTICES.txt",
        },
        Entry {
            from: Some("crates/app/licenses/FFmpeg-LGPL-2.1.txt".into()),
            name: "FFmpeg-LGPL-2.1.txt",
        },
        Entry {
            from: Some("crates/app/licenses/GPL-3.0.txt".into()),
            name: "GPL-3.0.txt",
        },
        Entry {
            from: Some("crates/app/licenses/yt-dlp-THIRD_PARTY_LICENSES.txt".into()),
            name: "yt-dlp-THIRD_PARTY_LICENSES.txt",
        },
        Entry {
            from: None,
            name: PORTABLE_MARKER,
        },
        Entry {
            from: None,
            name: "README.txt",
        },
    ]
}

/// An ffmpeg program or a libav DLL (libav is linked in statically; the licence text is meant to be there).
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

/// Checks the entry names of a finished zip (`Folder/name`): everything required is there, nothing is outside the
/// one top folder, no ffmpeg program or libav DLL slipped in, and no data folder (the user's data is never shipped).
pub fn check_portable_entries(top: &str, entries: &[String]) -> Res<()> {
    let prefix = format!("{top}/");
    for e in entries {
        let Some(rest) = e.strip_prefix(&prefix) else {
            return Err(format!("{e} is not inside {top}/"));
        };
        if rest.starts_with("data/") || rest == "data" {
            return Err(format!("{e}: a data folder must never be shipped"));
        }
        let file = rest.rsplit('/').next().unwrap_or(rest);
        if is_ffmpeg_binary(file) {
            return Err(format!(
                "{e}: an ffmpeg program or libav DLL, but libav is linked in statically"
            ));
        }
    }
    for want in portable_entries() {
        if !entries
            .iter()
            .any(|e| e == &format!("{prefix}{}", want.name))
        {
            return Err(format!("{top}/{} is missing from the zip", want.name));
        }
    }
    Ok(())
}

/// Writes a zip with every file under `top/`, forward slashes, deflate, a fixed timestamp (so the same inputs give
/// the same bytes). `files` are `(name inside the folder, bytes)`.
pub fn write_zip(out: &Path, top: &str, files: &[(String, Vec<u8>)]) -> Res<()> {
    let tmp = out.with_extension("zip.part");
    let result = (|| -> Res<()> {
        let file = std::fs::File::create(&tmp)
            .map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (name, bytes) in files {
            zip.start_file(format!("{top}/{name}"), options)
                .map_err(|e| format!("zip: {e}"))?;
            zip.write_all(bytes).map_err(|e| format!("zip: {e}"))?;
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
    fn the_signing_config_calls_xtask_and_keeps_the_rest_of_the_bundle_config() {
        let base =
            json!({"bundle": {"externalBin": ["binaries/yt-dlp"], "resources": ["licenses/*"]}});
        let conf = signed_tauri_config(&base, Path::new(r"C:\repo\target\debug\xtask.exe"));
        assert_eq!(conf["bundle"]["externalBin"][0], "binaries/yt-dlp");
        assert_eq!(
            conf["bundle"]["windows"]["signCommand"]["cmd"],
            r"C:\repo\target\debug\xtask.exe"
        );
        assert_eq!(
            conf["bundle"]["windows"]["signCommand"]["args"],
            json!(["sign-file", "%1"])
        );
        assert!(
            !conf.to_string().to_lowercase().contains("password"),
            "no secret in the config"
        );
        assert!(
            base["bundle"].get("windows").is_none(),
            "the input is not changed"
        );
    }

    #[test]
    fn an_unsigned_build_is_allowed_unless_signing_is_required() {
        assert!(unsigned_notice(false).unwrap().contains("SmartScreen"));
        assert!(unsigned_notice(true)
            .unwrap_err()
            .contains("PYXEE_SIGN_CERT_FILE"));
    }

    #[test]
    fn a_release_candidate_is_built_as_nsis_only() {
        assert_eq!(bundles_for("0.1.1-rc1"), "nsis");
        assert_eq!(bundles_for("0.2.0"), "nsis,msi");
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
    fn the_portable_zip_carries_the_gpl_texts_that_go_with_yt_dlp() {
        let names: Vec<&str> = portable_entries().iter().map(|e| e.name).collect();
        for n in ["GPL-3.0.txt", "yt-dlp-THIRD_PARTY_LICENSES.txt"] {
            assert!(names.contains(&n), "{n}");
        }
        assert!(PORTABLE_README.contains("GPL-3.0.txt"));
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
            "WebView2Loader.dll",
        ] {
            assert!(!is_ffmpeg_binary(n), "{n}");
        }
    }

    fn good(top: &str) -> Vec<String> {
        portable_entries()
            .iter()
            .map(|e| format!("{top}/{}", e.name))
            .collect()
    }

    #[test]
    fn a_complete_portable_zip_passes() {
        assert!(check_portable_entries("P", &good("P")).is_ok());
    }

    #[test]
    fn a_portable_zip_that_is_incomplete_or_wrong_is_refused() {
        let mut missing = good("P");
        missing.retain(|e| !e.ends_with("yt-dlp.exe"));
        assert!(check_portable_entries("P", &missing)
            .unwrap_err()
            .contains("yt-dlp.exe"));
        let mut no_marker = good("P");
        no_marker.retain(|e| !e.ends_with(PORTABLE_MARKER));
        assert!(check_portable_entries("P", &no_marker)
            .unwrap_err()
            .contains(PORTABLE_MARKER));
        let mut ff = good("P");
        ff.push("P/avcodec-61.dll".into());
        assert!(check_portable_entries("P", &ff)
            .unwrap_err()
            .contains("libav"));
        let mut data = good("P");
        data.push("P/data/library.db".into());
        assert!(check_portable_entries("P", &data)
            .unwrap_err()
            .contains("data folder"));
        let mut stray = good("P");
        stray.push("Other/x.txt".into());
        assert!(check_portable_entries("P", &stray)
            .unwrap_err()
            .contains("not inside"));
    }

    #[test]
    fn the_zip_is_written_atomically_under_one_folder_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("pyxee-xtask-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("p.zip");
        let files: Vec<(String, Vec<u8>)> = portable_entries()
            .iter()
            .map(|e| (e.name.to_string(), e.name.as_bytes().to_vec()))
            .collect();
        write_zip(&out, "Pyxee_x", &files).unwrap();
        let names = zip_names(&out).unwrap();
        assert!(
            names
                .iter()
                .all(|n| n.starts_with("Pyxee_x/") && !n.contains('\\')),
            "{names:?}"
        );
        check_portable_entries("Pyxee_x", &names).unwrap();
        let first = std::fs::read(&out).unwrap();
        write_zip(&out, "Pyxee_x", &files).unwrap();
        assert_eq!(
            first,
            std::fs::read(&out).unwrap(),
            "the same inputs give the same bytes"
        );
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            left,
            vec![std::ffi::OsString::from("p.zip")],
            "no .part file is left behind"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
