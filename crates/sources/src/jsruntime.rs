//! Which JavaScript runtime yt-dlp is told to use.
//!
//! Pyxee starts yt-dlp with `--ignore-config`, so a `--js-runtimes` line in the user's yt-dlp config file is never
//! read. This module decides what to pass on the command line instead: the `PYXEE_JS_RUNTIME` setting if there is
//! one, otherwise Deno (yt-dlp's own default), otherwise Node, whichever is found on `PATH`. With neither, nothing is
//! passed and yt-dlp behaves exactly as before.

use crate::scrub;
use std::ffi::OsStr;
use std::path::Path;

/// The environment variable (and `.env` line) that overrides the choice.
pub const ENV_VAR: &str = "PYXEE_JS_RUNTIME";
/// The runtimes yt-dlp knows, as it spells them in `--js-runtimes`.
pub const RUNTIMES: [&str; 4] = ["deno", "node", "quickjs", "bun"];
/// What is looked for on `PATH`, in this order.
const AUTO: [&str; 2] = ["deno", "node"];
const EXE: &str = if cfg!(windows) { ".exe" } else { "" };

/// What to pass for `--js-runtimes`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum JsRuntime {
    /// Pass nothing: yt-dlp uses its own default (Deno, when it can find it).
    #[default]
    Off,
    /// `RUNTIME` or `RUNTIME:PATH`, exactly as `--js-runtimes` takes it.
    On(String),
}

/// A parsed `PYXEE_JS_RUNTIME` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    /// `none` (or `off`): do not pass `--js-runtimes` at all.
    None,
    Runtime {
        name: &'static str,
        path: Option<String>,
    },
}

/// Reads `RUNTIME`, `RUNTIME:PATH` (a binary or the folder holding it, as yt-dlp accepts) or `none`.
pub fn parse_setting(value: &str) -> Result<Setting, String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("off") {
        return Ok(Setting::None);
    }
    let (name, path) = match value.split_once(':') {
        Some((n, p)) => (n.trim(), Some(p.trim())),
        None => (value, None),
    };
    let Some(known) = RUNTIMES.iter().find(|r| r.eq_ignore_ascii_case(name)) else {
        return Err(format!(
            "use deno, node, quickjs or bun (optionally followed by :path), or none; not \"{name}\""
        ));
    };
    match path {
        Some("") => Err(format!("a path is needed after \"{known}:\"")),
        Some(p) => Ok(Setting::Runtime {
            name: known,
            path: Some(p.to_string()),
        }),
        None => Ok(Setting::Runtime {
            name: known,
            path: None,
        }),
    }
}

/// True when `name` (with `.exe` on Windows) is a file in one of the folders of `path_var`.
fn on_path(name: &str, path_var: Option<&OsStr>, exists: &dyn Fn(&Path) -> bool) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    let file = format!("{name}{EXE}");
    std::env::split_paths(path_var)
        .any(|dir| !dir.as_os_str().is_empty() && exists(&dir.join(&file)))
}

impl JsRuntime {
    /// From the environment (`PYXEE_JS_RUNTIME`, then `PATH`).
    pub fn from_env() -> Self {
        Self::resolve(
            std::env::var(ENV_VAR).ok().as_deref(),
            std::env::var_os("PATH").as_deref(),
            &|p| p.exists(),
        )
    }

    /// The choice for `setting` (the override, if any), `path_var` (`PATH`) and a file-exists check; all three are
    /// parameters so tests need no real environment. Logs which runtime is used, never a path.
    pub fn resolve(
        setting: Option<&str>,
        path_var: Option<&OsStr>,
        exists: &dyn Fn(&Path) -> bool,
    ) -> Self {
        if let Some(raw) = setting.map(str::trim).filter(|s| !s.is_empty()) {
            match parse_setting(raw) {
                Ok(Setting::None) => {
                    log::info!("yt-dlp: no --js-runtimes is passed ({ENV_VAR}=none)");
                    return Self::Off;
                }
                Ok(Setting::Runtime { name, path: None }) => {
                    log::info!("yt-dlp: JavaScript runtime {name} ({ENV_VAR})");
                    return Self::On(name.to_string());
                }
                Ok(Setting::Runtime {
                    name,
                    path: Some(p),
                }) if exists(Path::new(&p)) => {
                    log::info!("yt-dlp: JavaScript runtime {name} from a path ({ENV_VAR})");
                    scrub::register(&p);
                    return Self::On(format!("{name}:{p}"));
                }
                Ok(Setting::Runtime { .. }) => {
                    log::warn!(
                        "{ENV_VAR} points to a path that does not exist; looking on PATH instead"
                    );
                }
                Err(e) => log::warn!("{ENV_VAR}: {e}; ignoring it"),
            }
        }
        for name in AUTO {
            if on_path(name, path_var, exists) {
                log::info!("yt-dlp: JavaScript runtime {name} found on PATH");
                return Self::On(name.to_string());
            }
        }
        log::info!("yt-dlp: no deno or node on PATH; leaving the JavaScript runtime to yt-dlp");
        Self::Off
    }

    /// The command-line arguments (none for [`JsRuntime::Off`]).
    pub fn args(&self) -> Vec<String> {
        match self {
            Self::Off => Vec::new(),
            Self::On(spec) => vec!["--js-runtimes".into(), spec.clone()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn path_of(dirs: &[&str]) -> OsString {
        std::env::join_paths(dirs.iter().map(PathBuf::from)).unwrap()
    }

    /// A file-exists check that says yes only to `files` (folder, file name).
    fn only<'a>(files: &'a [(&'a str, String)]) -> impl Fn(&Path) -> bool + 'a {
        move |p| {
            files.iter().any(|(dir, name)| {
                p.parent() == Some(Path::new(dir)) && p.file_name() == Some(OsStr::new(name))
            })
        }
    }

    fn exe(name: &str) -> String {
        format!("{name}{EXE}")
    }

    #[test]
    fn deno_on_path_is_passed_explicitly() {
        let files = [("bin", exe("deno"))];
        let path = path_of(&["other", "bin"]);
        let got = JsRuntime::resolve(None, Some(&path), &only(&files));
        assert_eq!(got, JsRuntime::On("deno".into()));
        assert_eq!(got.args(), ["--js-runtimes", "deno"]);
    }

    #[test]
    fn node_is_used_when_there_is_no_deno() {
        let files = [("bin", exe("node"))];
        let path = path_of(&["bin"]);
        let got = JsRuntime::resolve(None, Some(&path), &only(&files));
        assert_eq!(got.args(), ["--js-runtimes", "node"]);
    }

    #[test]
    fn deno_wins_when_both_are_there() {
        let files = [("a", exe("node")), ("b", exe("deno"))];
        let path = path_of(&["a", "b"]);
        assert_eq!(
            JsRuntime::resolve(None, Some(&path), &only(&files)).args(),
            ["--js-runtimes", "deno"]
        );
    }

    #[test]
    fn nothing_found_passes_nothing() {
        let path = path_of(&["a", "b"]);
        let got = JsRuntime::resolve(None, Some(&path), &|_| false);
        assert_eq!(got, JsRuntime::Off);
        assert!(got.args().is_empty());
        assert_eq!(
            JsRuntime::resolve(None, None, &|_| true),
            JsRuntime::Off,
            "no PATH at all"
        );
        assert_eq!(JsRuntime::default(), JsRuntime::Off);
    }

    #[test]
    fn the_override_beats_what_is_on_path() {
        let files = [("bin", exe("deno"))];
        let path = path_of(&["bin"]);
        let got = JsRuntime::resolve(Some(" Node "), Some(&path), &only(&files));
        assert_eq!(got.args(), ["--js-runtimes", "node"]);
        let got = JsRuntime::resolve(Some("bun"), None, &|_| false);
        assert_eq!(got.args(), ["--js-runtimes", "bun"]);
    }

    #[test]
    fn the_override_can_name_a_path_that_exists() {
        let spec = if cfg!(windows) {
            r"C:\tools\deno.exe"
        } else {
            "/opt/deno/bin/deno"
        };
        let got = JsRuntime::resolve(Some(&format!("deno:{spec}")), None, &|p| {
            p == Path::new(spec)
        });
        assert_eq!(
            got.args(),
            ["--js-runtimes".to_string(), format!("deno:{spec}")]
        );
    }

    #[test]
    fn an_override_path_that_is_missing_falls_back_to_path_lookup() {
        let files = [("bin", exe("node"))];
        let path = path_of(&["bin"]);
        let got = JsRuntime::resolve(Some("deno:/no/such/deno"), Some(&path), &only(&files));
        assert_eq!(got.args(), ["--js-runtimes", "node"]);
    }

    #[test]
    fn none_turns_it_off_even_when_a_runtime_is_there() {
        let files = [("bin", exe("deno"))];
        let path = path_of(&["bin"]);
        for v in ["none", "OFF"] {
            assert!(JsRuntime::resolve(Some(v), Some(&path), &only(&files))
                .args()
                .is_empty());
        }
    }

    #[test]
    fn a_nonsense_override_is_ignored_not_passed_on() {
        let files = [("bin", exe("node"))];
        let path = path_of(&["bin"]);
        let got = JsRuntime::resolve(Some("python"), Some(&path), &only(&files));
        assert_eq!(
            got.args(),
            ["--js-runtimes", "node"],
            "falls back to detection"
        );
        assert!(JsRuntime::resolve(Some("python"), None, &|_| false)
            .args()
            .is_empty());
    }

    #[test]
    fn a_blank_override_means_not_set() {
        let files = [("bin", exe("deno"))];
        let path = path_of(&["bin"]);
        assert_eq!(
            JsRuntime::resolve(Some("   "), Some(&path), &only(&files)).args(),
            ["--js-runtimes", "deno"]
        );
    }

    #[test]
    fn settings_are_parsed_strictly() {
        assert_eq!(
            parse_setting("NODE"),
            Ok(Setting::Runtime {
                name: "node",
                path: None
            })
        );
        assert_eq!(
            parse_setting(r"deno:C:\Tools\deno.exe"),
            Ok(Setting::Runtime {
                name: "deno",
                path: Some(r"C:\Tools\deno.exe".into())
            })
        );
        assert_eq!(parse_setting("none"), Ok(Setting::None));
        assert!(parse_setting("deno:").is_err());
        assert!(parse_setting("--js-runtimes").is_err());
        assert!(parse_setting("node --x").is_err());
    }
}
