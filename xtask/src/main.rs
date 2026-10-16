//! Developer tasks. `cargo xtask tools` downloads yt-dlp (with checksum verification);
//! `cargo xtask installer` builds the Windows installer with it bundled. Audio is decoded in-process
//! (Symphonia), so yt-dlp is the only helper program.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const YTDLP_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
/// Tauri finds sidecars by `<name>-<target triple>.exe`.
const TRIPLE: &str = "x86_64-pc-windows-msvc";

type Res<T> = Result<T, String>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("tools") => tools(&args[1..]),
        Some("installer") => installer(),
        _ => Err("usage:\n  cargo xtask tools [--update] [--dir <folder>]   download yt-dlp\n  cargo xtask installer                           build the Windows installer".into()),
    };
    if let Err(e) = result {
        eprintln!("\nerror: {e}");
        std::process::exit(1);
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the workspace")
        .to_path_buf()
}

// ------------------------------------------------------------------ helpers (tested)

/// Finds `name`'s hash in a checksum file (`<hash>  <name>` or `<hash> *<name>`).
pub fn parse_sums(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        let (hash, file) = (parts.next()?, parts.next()?);
        (file.trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn verify(what: &str, bytes: &[u8], expected: &str) -> Res<()> {
    let got = sha256_hex(bytes);
    if got == expected.to_ascii_lowercase() {
        Ok(())
    } else {
        Err(format!("{what}: checksum mismatch (expected {expected}, got {got}). The download is corrupt or was tampered with; nothing was installed."))
    }
}

/// Windows executables start with "MZ".
pub fn is_windows_exe(bytes: &[u8]) -> bool {
    bytes.starts_with(b"MZ")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Res<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("exe.part");
    std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("cannot move into place {}: {e}", path.display()))
}

// ---------------------------------------------------------------------- network

fn get(url: &str) -> Res<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(20))
        .timeout_read(std::time::Duration::from_secs(120))
        .build();
    let resp = agent.get(url).call().map_err(|e| match e {
        ureq::Error::Status(c, _) => format!("{url} answered HTTP {c}"),
        e => format!("could not reach {url}: {e}. Check your internet connection (and any proxy or firewall)."),
    })?;
    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("download of {url} was interrupted: {e}"))?;
    Ok(bytes)
}

fn mb(bytes: &[u8]) -> String {
    format!("{:.1} MB", bytes.len() as f64 / 1_048_576.0)
}

fn fetch_verified(label: &str, url: &str, sums_url: &str, file_in_sums: &str) -> Res<Vec<u8>> {
    println!("{label}: downloading {url}");
    let sums = String::from_utf8_lossy(&get(sums_url)?).into_owned();
    let expected = parse_sums(&sums, file_in_sums)
        .ok_or_else(|| format!("{file_in_sums} is not listed in {sums_url}"))?;
    let bytes = get(url)?;
    verify(label, &bytes, &expected)?;
    println!(
        "{label}: {} downloaded, sha256 verified ({expected})",
        mb(&bytes)
    );
    Ok(bytes)
}

// ------------------------------------------------------------------------ tools

fn tools(args: &[String]) -> Res<()> {
    let mut update = false;
    let mut dir = workspace_root().join("crates/app/binaries");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--update" => update = true,
            "--dir" => dir = PathBuf::from(it.next().ok_or("--dir needs a folder")?),
            other => return Err(format!("unknown option {other}")),
        }
    }

    let ytdlp = dir.join(format!("yt-dlp-{TRIPLE}.exe"));
    if ytdlp.exists() && !update {
        println!(
            "yt-dlp: already present ({}). Use --update to refresh it.",
            ytdlp.display()
        );
    } else {
        let bytes = fetch_verified(
            "yt-dlp",
            &format!("{YTDLP_BASE}/yt-dlp.exe"),
            &format!("{YTDLP_BASE}/SHA2-256SUMS"),
            "yt-dlp.exe",
        )?;
        if !is_windows_exe(&bytes) {
            return Err("the yt-dlp download is not a Windows executable".into());
        }
        write_atomic(&ytdlp, &bytes)?;
        println!("yt-dlp: installed {}", ytdlp.display());
    }

    println!("\nDone. The app finds it automatically in development; the installer bundles it.");
    println!("Licenses: yt-dlp is Unlicense (see crates/app/licenses/THIRD_PARTY.txt).");
    Ok(())
}

// --------------------------------------------------------------------- installer

fn run(cmd: &mut Command, what: &str) -> Res<()> {
    println!("\n> {what}");
    let status = cmd
        .status()
        .map_err(|e| format!("could not run {what}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what} failed ({status})"))
    }
}

fn npm() -> Command {
    Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" })
}

/// Where npm puts the Tauri CLI launcher for this repo.
pub fn tauri_cli(root: &Path) -> PathBuf {
    root.join("ui/node_modules/.bin")
        .join(if cfg!(windows) { "tauri.cmd" } else { "tauri" })
}

fn installer() -> Res<()> {
    if !cfg!(windows) {
        return Err("the installer can only be built on Windows".into());
    }
    let root = workspace_root();
    tools(&[])?;
    run(
        npm().args(["ci", "--prefix", "ui"]).current_dir(&root),
        "npm ci (UI dependencies)",
    )?;
    run(
        npm()
            .args(["run", "build", "--prefix", "ui"])
            .current_dir(&root),
        "npm run build (UI)",
    )?;
    // The Tauri CLI comes from the UI's npm dependencies (installed by `npm ci` above): no `cargo install`.
    let cli = tauri_cli(&root);
    if !cli.is_file() {
        return Err(format!(
            "the Tauri CLI was not found at {} (did `npm ci --prefix ui` succeed?)",
            cli.display()
        ));
    }
    let conf = root.join("crates/app/tauri.bundle.conf.json");
    run(
        Command::new(&cli)
            .args(["build", "--config"])
            .arg(&conf)
            .current_dir(root.join("crates/app")),
        "tauri build (installer with yt-dlp bundled)",
    )?;
    let bundle = root.join("target/release/bundle");
    println!("\nInstallers are in {}:", bundle.display());
    for sub in ["nsis", "msi"] {
        if let Ok(rd) = std::fs::read_dir(bundle.join(sub)) {
            for e in rd.flatten().filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x == "exe" || x == "msi")
            }) {
                println!("  {}", e.path().display());
            }
        }
    }
    println!("\nThe installer is unsigned: Windows SmartScreen will warn on first run (More info -> Run anyway).");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMS: &str = "AAAA1111  yt-dlp\nbbbb2222  yt-dlp.exe\ncccc3333 *tool-win64.zip\n\ndddd4444  tool-win64-shared.zip\n";

    #[test]
    fn finds_the_exact_file_in_a_checksum_list_in_both_formats() {
        assert_eq!(parse_sums(SUMS, "yt-dlp.exe").as_deref(), Some("bbbb2222"));
        assert_eq!(
            parse_sums(SUMS, "yt-dlp").as_deref(),
            Some("aaaa1111"),
            "lowercased, exact name only"
        );
        assert_eq!(
            parse_sums(SUMS, "tool-win64.zip").as_deref(),
            Some("cccc3333"),
            "binary-mode '*' prefix"
        );
        assert_eq!(
            parse_sums(SUMS, "tool-win64-shared.zip").as_deref(),
            Some("dddd4444")
        );
        assert_eq!(parse_sums(SUMS, "missing.zip"), None);
        assert_eq!(parse_sums("", "x"), None);
    }

    #[test]
    fn sha256_matches_the_known_test_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_wrong_checksum_is_refused_with_both_hashes_in_the_message() {
        assert!(
            verify(
                "x",
                b"abc",
                "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
            )
            .is_ok(),
            "case-insensitive"
        );
        let err = verify("yt-dlp", b"abc", "deadbeef").unwrap_err();
        assert!(
            err.contains("deadbeef")
                && err.contains("ba7816bf")
                && err.contains("nothing was installed"),
            "{err}"
        );
    }

    #[test]
    fn the_tauri_cli_is_taken_from_the_ui_dependencies() {
        let p = tauri_cli(Path::new("repo"));
        assert!(
            p.starts_with("repo/ui/node_modules/.bin")
                || p.starts_with("repo\\ui\\node_modules\\.bin"),
            "{p:?}"
        );
        assert!(p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("tauri"));
    }

    #[test]
    fn recognises_windows_executables() {
        assert!(is_windows_exe(b"MZ\x90\x00"));
        assert!(!is_windows_exe(b"<html>rate limited</html>"));
        assert!(!is_windows_exe(b""));
    }

    #[test]
    fn writing_replaces_the_file_without_leaving_partial_files() {
        let dir = std::env::temp_dir().join(format!("pyxee-xtask-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested").join("tool.exe");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("tool.exe")]);
    }
}
