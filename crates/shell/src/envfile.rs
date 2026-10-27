//! The `.env` file in the data folder: `NAME="value"` lines for the few settings that are also environment
//! variables (Settings > Advanced edits it). It is read once at start-up, before anything looks at the
//! environment. A variable that Windows already sets wins over the file.

use std::path::Path;

/// The variables Settings > Advanced shows, in display order. Only these are read from the file.
pub const VARS: [&str; 7] = [
    "PYXEE_COOKIES",
    "PYXEE_COOKIES_FROM_BROWSER",
    "PYXEE_YTDLP",
    "PYXEE_JS_RUNTIME",
    "PYXEE_YTDLP_PLAYER_CLIENT",
    "PYXEE_YTMUSIC_DIRECT",
    "PYXEE_LOG",
];

/// What loading the file did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Variables that have a value in the file but are also set in Windows, so the file's value is not used.
    pub from_windows: Vec<String>,
    /// Set when the file exists but could not be read.
    pub warning: Option<String>,
}

/// The file's text; empty when there is no file yet.
pub fn read(path: &Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        other => other,
    }
}

/// Puts the file's values into the process environment, except for variables Windows already sets.
pub fn apply(path: &Path) -> Applied {
    let text = match read(path) {
        Ok(t) => t,
        Err(e) => {
            let warning = Some(format!("could not read {}: {e}", path.display()));
            return Applied {
                from_windows: Vec::new(),
                warning,
            };
        }
    };
    let mut from_windows = Vec::new();
    for key in VARS {
        let Some(value) = lookup(&text, key).filter(|v| !v.is_empty() && !v.contains('\0')) else {
            continue;
        };
        if std::env::var_os(key).is_some_and(|v| !v.is_empty()) {
            from_windows.push(key.to_string());
        } else {
            std::env::set_var(key, value);
        }
    }
    Applied {
        from_windows,
        warning: None,
    }
}

/// The value of `key` in `.env` text (the last one if it appears twice). Quotes around it are removed;
/// backslashes are kept as typed, so Windows paths work without escaping.
pub fn lookup(text: &str, key: &str) -> Option<String> {
    text.lines().rev().find_map(|line| {
        let (k, v) = split_line(line)?;
        (k == key).then(|| unquote(v).to_string())
    })
}

fn split_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_start_matches('\u{feff}').trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let line = line.strip_prefix("export ").unwrap_or(line);
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}

fn unquote(v: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = v.strip_prefix(quote).and_then(|r| r.strip_suffix(quote)) {
            return inner;
        }
    }
    v
}

fn is_replaced(line: &str, values: &[(&str, &str)]) -> bool {
    split_line(line).is_some_and(|(k, _)| values.iter().any(|(name, _)| *name == k))
}

/// `text` with the lines for the names in `values` replaced (an empty value removes the line). Every other
/// line, comments included, is kept as it is.
pub fn render(text: &str, values: &[(&str, &str)]) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !is_replaced(l, values))
        .map(str::to_string)
        .collect();
    for (name, value) in values {
        if !value.is_empty() {
            lines.push(format!("{name}=\"{value}\""));
        }
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// Saved through [`crate::atomic::write`], so a crash never leaves a half-written file.
pub fn write(path: &Path, text: &str) -> std::io::Result<()> {
    crate::atomic::write(path, text.as_bytes())
}
