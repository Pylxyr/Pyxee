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
