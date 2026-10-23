//! Settings > Advanced: the environment variables Pyxee reads, kept in a `.env` file in the data folder.
//! The file is applied once at start-up (see `main`), so a saved change is used from the next start.

use crate::dto::{EnvSettingsDto, EnvVarDto};
use pyxee_shell::envfile::{self, VARS};
use std::path::{Path, PathBuf};
use tauri::State;

const LOG_LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

pub struct EnvState {
    /// `<data folder>\.env`; `None` when there is no data folder.
    pub file: Option<PathBuf>,
    /// Variables Windows sets as well: their value wins over the file.
    pub from_windows: Vec<String>,
}

fn read_file(state: &EnvState) -> Result<String, String> {
    let Some(path) = &state.file else {
        return Ok(String::new());
    };
    envfile::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))
}

fn snapshot(state: &EnvState, text: &str) -> EnvSettingsDto {
    let mut vars = Vec::new();
    for name in VARS {
        vars.push(EnvVarDto {
            name: name.to_string(),
            value: envfile::lookup(text, name).unwrap_or_default(),
            from_windows: state.from_windows.iter().any(|w| w == name),
        });
    }
    EnvSettingsDto {
        file: state.file.as_ref().map(|p| p.display().to_string()),
        vars,
    }
}

/// Why `value` cannot be saved for `name`, in words for the person who typed it. An empty value is always fine
/// (it removes the variable).
fn check(name: &str, value: &str) -> Result<(), String> {
    if value.chars().any(|c| c == '"' || c.is_control()) {
        return Err(format!("{name} cannot contain quotes or line breaks."));
    }
    let problem = match name {
        _ if value.is_empty() => None,
        "PYXEE_COOKIES" if !Path::new(value).is_file() => Some("that file was not found"),
        "PYXEE_JS_RUNTIME" => match pyxee_sources::jsruntime::parse_setting(value) {
            Err(e) => return Err(format!("{name}: {e}.")),
            Ok(pyxee_sources::jsruntime::Setting::Runtime { path: Some(p), .. })
                if !Path::new(&p).exists() =>
            {
                Some("that path was not found")
            }
            Ok(_) => None,
        },
        "PYXEE_YTMUSIC_DIRECT" => match pyxee_sources::ytmusic::parse_switch(value) {
            Err(e) => return Err(format!("{name}: {e}.")),
            Ok(_) => None,
        },
        "PYXEE_LOG" if !LOG_LEVELS.contains(&value.to_ascii_lowercase().as_str()) => {
            Some("use error, warn, info, debug or trace")
        }
        _ => None,
    };
    problem.map_or(Ok(()), |p| Err(format!("{name}: {p}.")))
}

#[tauri::command]
pub fn env_get(state: State<EnvState>) -> Result<EnvSettingsDto, String> {
    let text = read_file(&state)?;
    Ok(snapshot(&state, &text))
}

/// Saves the values to `.env`; they are used from the next start. Lines in the file that are not one of the
/// settings are left alone. An empty value removes that variable's line.
#[tauri::command]
pub fn env_set(state: State<EnvState>, vars: Vec<EnvVarDto>) -> Result<EnvSettingsDto, String> {
    let Some(file) = &state.file else {
        return Err("Pyxee has no data folder, so there is nowhere to save these.".into());
    };
    let mut values = Vec::new();
    for var in &vars {
        if !VARS.contains(&var.name.as_str()) {
            return Err(format!("{} is not a setting Pyxee knows.", var.name));
        }
        let value = var.value.trim();
        check(&var.name, value)?;
        values.push((var.name.as_str(), value));
    }
    // A file that cannot be read is left untouched rather than overwritten.
    let text = read_file(&state)?;
    let out = envfile::render(&text, &values);
    envfile::write(file, &out).map_err(|e| format!("Could not save {}: {e}", file.display()))?;
    let saved = read_file(&state)?;
    Ok(snapshot(&state, &saved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_js_runtime_setting_takes_a_runtime_a_path_that_exists_or_none() {
        for ok in ["deno", "node", "NONE", "bun", ""] {
            assert_eq!(check("PYXEE_JS_RUNTIME", ok), Ok(()), "{ok}");
        }
        let here = std::env::temp_dir().display().to_string();
        assert_eq!(check("PYXEE_JS_RUNTIME", &format!("node:{here}")), Ok(()));
    }

    #[test]
    fn a_bad_js_runtime_setting_is_explained() {
        assert!(check("PYXEE_JS_RUNTIME", "python")
            .unwrap_err()
            .contains("deno, node"));
        assert!(check("PYXEE_JS_RUNTIME", "node:")
            .unwrap_err()
            .contains("path is needed"));
        assert!(check("PYXEE_JS_RUNTIME", "node:/no/such/place/node")
            .unwrap_err()
            .contains("not found"));
    }

    #[test]
    fn the_ytmusic_switch_takes_zero_or_one() {
        for ok in ["0", "1", "off", "ON", ""] {
            assert_eq!(check("PYXEE_YTMUSIC_DIRECT", ok), Ok(()), "{ok}");
        }
        assert!(check("PYXEE_YTMUSIC_DIRECT", "maybe")
            .unwrap_err()
            .contains("0 or 1"));
    }

    #[test]
    fn every_setting_in_the_file_list_is_known_to_the_ui_list() {
        // ui/src/lib/envvars.ts must list the same names in the same order.
        let ts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/lib/envvars.ts"
        ))
        .expect("ui/src/lib/envvars.ts");
        let listed: Vec<&str> = ts
            .lines()
            .filter_map(|l| l.trim().strip_prefix("name: '")?.strip_suffix("',"))
            .collect();
        assert_eq!(listed, VARS);
    }
}
