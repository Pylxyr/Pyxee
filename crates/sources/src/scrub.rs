//! Keeps cookie locations and user-profile paths out of logs and error text.
//!
//! yt-dlp's stderr, and the arguments we start it with, can name the cookie file, the browser
//! profile that cookies are read from, or folders under the user's profile. `scrub` replaces those,
//! so nothing built from them reaches the log file or the UI.

use std::sync::{Mutex, OnceLock, PoisonError};

const REDACTED: &str = "<redacted>";
/// Environment variables whose values are replaced by their `%NAME%` form.
const ENV_PATHS: [&str; 4] = ["USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP"];

fn registry() -> &'static Mutex<Vec<String>> {
    static SECRETS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    SECRETS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Makes `scrub` replace `secret` (the active cookie file path or browser spec) from now on.
pub(crate) fn register(secret: &str) {
    let secret = secret.trim();
    if secret.is_empty() {
        return;
    }
    let mut known = registry().lock().unwrap_or_else(PoisonError::into_inner);
    if !known.iter().any(|k| k == secret) {
        known.push(secret.to_string());
    }
}

/// `text` without the active cookie file or browser spec, without the user's profile folders
/// (`%USERPROFILE%`, `%APPDATA%`, `%LOCALAPPDATA%`, `%TEMP%`) and without the value of any
/// `--cookies` or `--cookies-from-browser` argument.
pub fn scrub(text: &str) -> String {
    let env: Vec<(&str, String)> = ENV_PATHS
        .iter()
        .filter_map(|name| Some((*name, std::env::var(name).ok()?)))
        .collect();
    scrub_with(text, &env)
}

fn scrub_with(text: &str, env: &[(&str, String)]) -> String {
    let mut out = text.to_string();
    let secrets = registry()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    for secret in secrets {
        out = replace_ci(&out, &secret, REDACTED);
    }
    // The most specific folder first: %TEMP% and %APPDATA% usually sit inside %USERPROFILE%.
    let mut folders: Vec<(&str, &str)> = env
        .iter()
        .map(|(name, value)| (*name, value.trim().trim_end_matches(['\\', '/'])))
        .filter(|(_, value)| value.len() >= 3)
        .collect();
    folders.sort_by_key(|(_, value)| std::cmp::Reverse(value.len()));
    for (name, value) in folders {
        out = replace_ci(&out, value, &format!("%{name}%"));
    }
    redact_flag_values(&out)
}

/// Replaces every occurrence of `needle` in `text` with `with`, ignoring ASCII case and treating
/// `/` and `\` as the same character.
fn replace_ci(text: &str, needle: &str, with: &str) -> String {
    let norm = |b: u8| match b {
        b'\\' => b'/',
        b => b.to_ascii_lowercase(),
    };
    let needle: Vec<u8> = needle.bytes().map(norm).collect();
    if needle.is_empty() {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let end = i + needle.len();
        if end <= bytes.len()
            && bytes[i..end]
                .iter()
                .zip(&needle)
                .all(|(b, n)| norm(*b) == *n)
        {
            out.extend_from_slice(with.as_bytes());
            i = end;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    // Matches start and end on whole characters (the needle is valid UTF-8), so this stays valid.
    String::from_utf8_lossy(&out).into_owned()
}

/// Replaces the word after `--cookies` or `--cookies-from-browser`.
fn redact_flag_values(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut redact_next = false;
    let mut rest = text;
    while !rest.is_empty() {
        let space = rest.starts_with(char::is_whitespace);
        let end = rest
            .find(|c: char| c.is_whitespace() != space)
            .unwrap_or(rest.len());
        let (chunk, tail) = rest.split_at(end);
        if space {
            out.push_str(chunk);
        } else if redact_next {
            out.push_str(REDACTED);
            redact_next = false;
        } else {
            out.push_str(chunk);
            redact_next = matches!(chunk, "--cookies" | "--cookies-from-browser");
        }
        rest = tail;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cookie_file_path_is_replaced_in_either_slash_style_and_case() {
        register(r"D:\Secret Stuff\cookies.txt");
        let a = scrub(r"could not open D:\Secret Stuff\cookies.txt: denied");
        let b = scrub("could not open d:/secret stuff/COOKIES.txt: denied");
        assert_eq!(a, "could not open <redacted>: denied");
        assert_eq!(b, a);
    }

    #[test]
    fn a_browser_profile_is_replaced() {
        register("chrome:Profile 7");
        let out = scrub("ERROR: could not read chrome:Profile 7 cookie database");
        assert_eq!(out, "ERROR: could not read <redacted> cookie database");
    }

    #[test]
    fn user_profile_folders_become_their_variable_names() {
        let env = [
            ("USERPROFILE", r"C:\Users\Jo Doe".to_string()),
            ("APPDATA", r"C:\Users\Jo Doe\AppData\Roaming".to_string()),
            ("TEMP", r"C:\Users\Jo Doe\AppData\Local\Temp\".to_string()),
        ];
        assert_eq!(
            scrub_with(r"read C:\Users\Jo Doe\Music\a.webm", &env),
            r"read %USERPROFILE%\Music\a.webm"
        );
        assert_eq!(
            scrub_with("wrote c:/users/jo doe/appdata/local/temp/x.part", &env),
            "wrote %TEMP%/x.part"
        );
        assert_eq!(
            scrub_with(r"in C:\Users\Jo Doe\AppData\Roaming\Pyxee", &env),
            r"in %APPDATA%\Pyxee"
        );
    }

    #[test]
    fn the_value_of_a_cookies_argument_is_replaced() {
        assert_eq!(
            scrub("yt-dlp --cookies /somewhere/c.txt -j x"),
            "yt-dlp --cookies <redacted> -j x"
        );
        assert_eq!(
            scrub("--cookies-from-browser vivaldi+gnomekeyring:Default --no-warnings"),
            "--cookies-from-browser <redacted> --no-warnings"
        );
        assert_eq!(scrub("nothing to hide"), "nothing to hide");
    }
}
