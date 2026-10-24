//! Developer tasks. `cargo xtask tools` downloads yt-dlp (with checksum verification);
//! `cargo xtask ffmpeg` builds the minimal static FFmpeg the audio crate links; `cargo xtask installer` builds the Windows installer with it bundled. Audio is decoded in-process
//! (libav), so yt-dlp is the only helper program. `cargo xtask portable` makes the portable zip; both it and the
//! installer are code-signed when a certificate is configured (see `release.rs`).

mod kit;
mod release;

use release::{
    bundles_for, check_portable_entries, portable_entries, portable_folder_name, portable_zip_name,
    redacted, signed_tauri_config, unsigned_notice, write_zip, zip_names, Cert, Signing,
    PORTABLE_MARKER, PORTABLE_README,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The yt-dlp release that is bundled into installers and the portable zip. Bump it with `YTDLP_SHA256` and the
/// licence pins below, and update crates/app/licenses/THIRD_PARTY.txt (docs/releasing.md has the procedure).
const YTDLP_VERSION: &str = "2026.10.21";
/// SHA-256 of that release's `yt-dlp.exe` (from its published SHA2-256SUMS). `cargo xtask tools` refuses any other bytes.
const YTDLP_SHA256: &str = "33121d192d706e62f684a06f80918e3f3a8ba52ac3a65213b6951c39c7d9f555";

/// Where the pinned release's files are. The in-app updater (crates/app/src/updater.rs) is separate on purpose: it
/// pulls the latest release at run time, which is how users get a yt-dlp that keeps up with YouTube.
fn ytdlp_base() -> String {
    format!("https://github.com/yt-dlp/yt-dlp/releases/download/{YTDLP_VERSION}")
}
/// yt-dlp's Windows PyInstaller executable includes GPLv3+ code, so the GPL text and upstream's list of bundled
/// licences ship with it. `(file in crates/app/licenses/, url, sha256 of the LF-normalised text)`. The hashes pin what
/// was read from upstream for `YTDLP_VERSION`; a mismatch stops the build.
pub fn ytdlp_license_files() -> Vec<(&'static str, String, &'static str)> {
    vec![
        (
            "yt-dlp-THIRD_PARTY_LICENSES.txt",
            format!("https://raw.githubusercontent.com/yt-dlp/yt-dlp/{YTDLP_VERSION}/THIRD_PARTY_LICENSES.txt"),
            "472aefe951c7db35e1657c1d13fd337140511ed6f2b329205105ad441c5a02b7",
        ),
        (
            // The verbatim FSF text of the GPL v3, read from COPYING.GPLv3 at the FFmpeg commit pinned in
            // scripts/build-ffmpeg.sh (a fixed commit, so the bytes cannot change under us).
            "GPL-3.0.txt",
            "https://raw.githubusercontent.com/FFmpeg/FFmpeg/1041abdc962f4cc4f394aa8de9dc5236c0c3b9e7/COPYING.GPLv3".to_string(),
            "8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903",
        ),
    ]
}

/// Hash of a licence text with Windows line endings folded to `\n`, so a checkout with autocrlf still matches.
pub fn license_sha256(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    sha256_hex(text.as_bytes())
}
/// Tauri finds sidecars by `<name>-<target triple>.exe`.
const TRIPLE: &str = "x86_64-pc-windows-msvc";

type Res<T> = Result<T, String>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("tools") => tools(&args[1..]),
        Some("ffmpeg") => ffmpeg(),
        Some("ffmpeg-kit") => ffmpeg_kit(),
        Some("installer") => installer(),
        Some("portable") => portable(),
        Some("sign-file") => sign_file(&args[1..]),
        Some("signing-config") => signing_config(),
        _ => Err("usage:\n  cargo xtask tools [--update] [--dir <folder>]   download the pinned yt-dlp (--update: download it again)\n  cargo xtask ffmpeg                              build the minimal static FFmpeg (once per FFmpeg change)\n  cargo xtask ffmpeg-kit                          write the FFmpeg build kit zip (LGPL relinking material) to target/kit\n  cargo xtask installer                           build the Windows installer (signed when a certificate is configured)\n  cargo xtask portable                            build the portable zip (signed program when a certificate is configured)\n  cargo xtask signing-config                      print the tauri bundle config to use (CI builds the installer with it)\n  cargo xtask sign-file <file>                    sign one file with the configured certificate (used by tauri build)".into()),
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

// ------------------------------------------------------------------------- FFmpeg

/// The `host:` line of `rustc -vV`, e.g. `x86_64-pc-windows-msvc`.
pub fn parse_host(rustc_vv: &str) -> Option<String> {
    rustc_vv
        .lines()
        .find_map(|l| l.strip_prefix("host:"))
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
}

/// Where the audio crate's `build.rs` looks for the libraries: `PYXEE_FFMPEG_DIR`, else
/// `target/ffmpeg/<host triple>`. Keep in sync with `crates/audio/build.rs`.
fn ffmpeg_prefix(root: &Path) -> Res<PathBuf> {
    if let Some(dir) = std::env::var_os("PYXEE_FFMPEG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|e| format!("cannot run rustc: {e}"))?;
    let host = parse_host(&String::from_utf8_lossy(&out.stdout))
        .ok_or("rustc -vV did not report a host triple")?;
    Ok(root.join("target/ffmpeg").join(host))
}

/// The arguments for bash to run the build script. On Windows bash must be a *login* shell (`-l`): that is
/// what puts MSYS2's `/usr/bin` (mkdir, make, sha256sum...) on PATH; without it the script dies on its first
/// command with "mkdir: command not found".
pub fn bash_args(windows: bool, script: &Path, prefix: &Path) -> Vec<String> {
    let mut args = Vec::new();
    if windows {
        args.push("-l".to_string());
    }
    args.push(bash_path(script));
    args.push(bash_path(prefix));
    args
}

/// bash needs forward slashes even for Windows paths.
fn bash_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// What to run once to get the Windows build tools for FFmpeg.
const MSYS2_SETUP: &str = r#"winget install --id MSYS2.MSYS2 -e   then   C:\msys64\usr\bin\bash.exe -lc "pacman -S --noconfirm --needed make nasm diffutils pkgconf""#;
/// MSYS2 programs FFmpeg's `configure` and `make` need.
const MSYS2_TOOLS: [&str; 3] = ["make", "nasm", "diff"];

/// The MSYS2 bash to use: `PYXEE_BASH`, else the default install. `exists` is injected so this is testable.
pub fn locate_bash(from_env: Option<String>, exists: impl Fn(&Path) -> bool) -> Res<PathBuf> {
    let path = PathBuf::from(from_env.unwrap_or_else(|| r"C:\msys64\usr\bin\bash.exe".into()));
    if exists(&path) {
        Ok(path)
    } else {
        Err(format!(
            "MSYS2 (bash) was not found at {}. FFmpeg's build needs it, once: {MSYS2_SETUP}. \
             (Or set PYXEE_BASH to your MSYS2 bash.exe.)",
            path.display()
        ))
    }
}

/// An error naming the MSYS2 tools that are missing, from the one-per-line output of the probe.
pub fn missing_tools_error(probe_output: &str) -> Option<String> {
    let missing: Vec<&str> = probe_output
        .lines()
        .map(str::trim)
        // A first MSYS2 login shell also prints its "Copying skeleton files" notice to stdout; only tool names count.
        .filter(|l| MSYS2_TOOLS.contains(l))
        .collect();
    (!missing.is_empty()).then(|| {
        format!(
            "MSYS2 is missing {}. Install them once: C:\\msys64\\usr\\bin\\bash.exe -lc \"pacman -S --noconfirm --needed make nasm diffutils pkgconf\"",
            missing.join(", ")
        )
    })
}

// ------------------------------------------------------------ MSVC environment (tested)

const MSVC_MISSING: &str = "the MSVC compiler (cl.exe) is not on PATH and no Visual Studio C++ build tools were found (vswhere). Install them, or run this from an \"x64 Native Tools Command Prompt for VS\".";

/// The Visual Studio folder: the first non-empty line of `vswhere -property installationPath`.
pub fn parse_vswhere_install(stdout: &str) -> Option<PathBuf> {
    stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(PathBuf::from)
}

/// The `NAME=value` lines printed by cmd's `set`. Lines without a name (cmd's own `=C:=...` entries) are skipped.
pub fn parse_env_dump(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (name, value) = l.trim_end_matches('\r').split_once('=')?;
            (!name.is_empty()).then(|| (name.to_string(), value.to_string()))
        })
        .collect()
}

/// Where Visual Studio keeps the script that puts the x64 compiler on PATH.
pub fn vcvars_path(vs: &Path) -> PathBuf {
    vs.join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("vcvars64.bat")
}

/// A batch file that runs `vcvars64.bat` and prints the resulting environment. (A file, not `cmd /c "..."`: the
/// path has spaces and often parentheses, which `cmd /c` mis-handles when Rust quotes the argument.)
pub fn msvc_env_script(vcvars: &Path) -> String {
    [
        "@echo off".to_string(),
        // `set` prints in the console code page; UTF-8 keeps non-ASCII profile paths intact.
        "chcp 65001 >nul".to_string(),
        format!("call \"{}\" >nul", vcvars.display()),
        "if errorlevel 1 exit /b 1".to_string(),
        "set".to_string(),
    ]
    .join("\r\n")
        + "\r\n"
}

/// The variables to add to child processes so the MSVC tools are found: nothing when `cl` is already on PATH (an
/// "x64 Native Tools" prompt), else what `vcvars64.bat` sets. The lookups are injected so this is testable.
pub fn msvc_environment(
    cl_on_path: bool,
    vs_install: impl FnOnce() -> Option<PathBuf>,
    dump_env: impl FnOnce(&Path) -> Res<String>,
) -> Res<Vec<(String, String)>> {
    if cl_on_path {
        return Ok(Vec::new());
    }
    let vs = vs_install().ok_or(MSVC_MISSING)?;
    let env = parse_env_dump(&dump_env(&vcvars_path(&vs))?);
    if !env.iter().any(|(k, _)| k.eq_ignore_ascii_case("path")) {
        return Err(format!(
            "vcvars64.bat under {} did not produce an environment (no PATH was printed)",
            vs.display()
        ));
    }
    Ok(env)
}

/// Windows only (not compiled to run anywhere else): the newest Visual Studio with the x64 C++ tools, via vswhere.
fn find_vs_install() -> Option<PathBuf> {
    let pf = std::env::var_os("ProgramFiles(x86)")
        .map_or_else(|| PathBuf::from(r"C:\Program Files (x86)"), PathBuf::from);
    let vswhere = pf.join(r"Microsoft Visual Studio\Installer\vswhere.exe");
    let out = Command::new(vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()?;
    parse_vswhere_install(&String::from_utf8_lossy(&out.stdout))
}

/// Runs `vcvars64.bat` in a throwaway cmd and returns the environment it ends with.
fn dump_msvc_env(vcvars: &Path) -> Res<String> {
    if !vcvars.is_file() {
        return Err(format!("{} does not exist", vcvars.display()));
    }
    let bat = std::env::temp_dir().join(format!("pyxee-msvc-env-{}.cmd", std::process::id()));
    std::fs::write(&bat, msvc_env_script(vcvars))
        .map_err(|e| format!("cannot write {}: {e}", bat.display()))?;
    let out = Command::new(&bat).output();
    let _ = std::fs::remove_file(&bat);
    let out = out.map_err(|e| format!("could not run vcvars64.bat: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("vcvars64.bat failed ({})", out.status))
    }
}

/// The commit a git checkout is at. Looks only at the checkout's own `.git`: a folder that has none (or a broken one)
/// reads as "no commit" instead of reporting the commit of whatever repository it happens to sit inside.
fn git_head(src: &Path) -> Res<String> {
    let out = Command::new("git")
        .arg("--git-dir")
        .arg(src.join(".git"))
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The FFmpeg source at the pinned commit as a `.tar.gz`, from the checkout `build-ffmpeg.sh` made if it is still there,
/// otherwise from a fresh shallow clone. A leftover clone in `target/kit` (a CI cache can bring one back from before
/// the pin moved) is deleted and cloned again; a checkout that is still not exactly the pinned commit is refused.
fn ffmpeg_source_archive(root: &Path, prefix: &Path, pin: &kit::Pin) -> Res<Vec<u8>> {
    let built = PathBuf::from(format!("{}.build/src", prefix.display()));
    let src = if built.join(".git").exists() {
        built
    } else {
        let dir = root.join("target/kit/ffmpeg-src");
        if dir.join(".git").exists() && git_head(&dir)? != pin.commit {
            println!(
                "the FFmpeg source in {} is not at the pinned commit; fetching it again",
                dir.display()
            );
            std::fs::remove_dir_all(&dir)
                .map_err(|e| format!("cannot delete {}: {e}", dir.display()))?;
        }
        if !dir.join(".git").exists() {
            run(
                Command::new("git")
                    .args(["clone", "--depth", "1", "--branch", &pin.tag, &pin.url])
                    .arg(&dir),
                "git clone of the pinned FFmpeg",
            )?;
        }
        dir
    };
    let head = git_head(&src)?;
    if head != pin.commit {
        return Err(format!(
            "the FFmpeg source in {} is at {head}, not the pinned {}; delete it and retry",
            src.display(),
            pin.commit
        ));
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(&src)
        .args(["archive", "--format=tar.gz"])
        .arg(format!("--prefix=FFmpeg-{}/", pin.commit))
        .arg("HEAD")
        .output()
        .map_err(|e| format!("could not run git archive: {e}"))?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(format!(
            "git archive failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

/// Writes `target/kit/Pyxee_<version>_ffmpeg-build-kit.zip`: the exact static libraries and headers linked into this
/// build, the unmodified FFmpeg source, the build script and a README with the relink steps (see `kit.rs`). Run it
/// after `cargo xtask ffmpeg` (the installer build does that).
fn ffmpeg_kit() -> Res<()> {
    let root = workspace_root();
    let prefix = ffmpeg_prefix(&root)?;
    let read =
        |rel: &str| std::fs::read(root.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"));
    let script = read("scripts/build-ffmpeg.sh")?;
    let pin = kit::pin(&String::from_utf8_lossy(&script))?;
    let version = env!("CARGO_PKG_VERSION");
    let prefix_files = kit::prefix_files(&prefix)?;
    println!(
        "FFmpeg {} ({}): {} files from {}",
        pin.tag,
        pin.commit,
        prefix_files.len(),
        prefix.display()
    );
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("README.txt".into(), kit::readme(version, &pin).into_bytes()),
        (
            "FFmpeg-LGPL-2.1.txt".into(),
            read("crates/app/licenses/FFmpeg-LGPL-2.1.txt")?,
        ),
        (
            format!("ffmpeg-source/{}", kit::source_archive_name(&pin)),
            ffmpeg_source_archive(&root, &prefix, &pin)?,
        ),
        ("build-inputs/build-ffmpeg.sh".into(), script),
        (
            "build-inputs/ffmpeg-action.yml".into(),
            read(".github/actions/ffmpeg/action.yml")?,
        ),
    ];
    let mut prefix_names = Vec::new();
    for (name, path) in &prefix_files {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        files.push((format!("prefix/{name}"), bytes));
        prefix_names.push(name.clone());
    }
    let out_dir = root.join("target/kit");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let zip = out_dir.join(kit::kit_zip_name(version));
    let top = kit::kit_folder_name(version);
    release::write_zip(&zip, &top, &files)?;
    // The zip must hold exactly what the README promises.
    let got = release::zip_names(&zip)?;
    for want in kit::kit_names(&prefix_names, &pin) {
        if !got.iter().any(|g| g == &format!("{top}/{want}")) {
            return Err(format!("{top}/{want} is missing from the kit zip"));
        }
    }
    println!(
        "Wrote {} ({} MB)",
        zip.display(),
        std::fs::metadata(&zip)
            .map(|m| m.len() / 1_000_000)
            .unwrap_or(0)
    );
    Ok(())
}

/// Builds the minimal static FFmpeg with `scripts/build-ffmpeg.sh` (skipped when it is current).
/// Windows: needs MSYS2 (`make`, `nasm`, `diffutils`) and the MSVC tools. When `cl` is not on PATH (a plain shell,
/// a CI step) the Visual Studio environment is imported with vswhere + `vcvars64.bat` for the build only.
fn ffmpeg() -> Res<()> {
    let root = workspace_root();
    let prefix = ffmpeg_prefix(&root)?;
    let script = root.join("scripts/build-ffmpeg.sh");
    let mut cmd;
    if cfg!(windows) {
        let msvc_env = msvc_environment(
            Command::new("cl").output().is_ok(),
            find_vs_install,
            dump_msvc_env,
        )?;
        let bash = locate_bash(std::env::var("PYXEE_BASH").ok(), |p| p.is_file())?;
        // Keep the MSVC tools on PATH inside MSYS2 (and its `link.exe` must not shadow MSVC's).
        let probe = format!(
            "for t in {}; do command -v $t >/dev/null || echo $t; done",
            MSYS2_TOOLS.join(" ")
        );
        let out = Command::new(&bash)
            .args(["-lc", &probe])
            .envs(msvc_env.iter().map(|(k, v)| (k, v)))
            .env("MSYS2_PATH_TYPE", "inherit")
            .output()
            .map_err(|e| format!("could not run {}: {e}", bash.display()))?;
        if let Some(e) = missing_tools_error(&String::from_utf8_lossy(&out.stdout)) {
            return Err(e);
        }
        cmd = Command::new(bash);
        cmd.envs(msvc_env.iter().map(|(k, v)| (k, v)))
            .env("MSYS2_PATH_TYPE", "inherit")
            .env("PYXEE_FF_TOOLCHAIN", "msvc")
            // A login shell starts in $HOME unless told otherwise.
            .env("CHERE_INVOKING", "1");
    } else {
        cmd = Command::new("bash");
    }
    cmd.args(bash_args(cfg!(windows), &script, &prefix));
    run(cmd.current_dir(&root), "build the minimal static FFmpeg")
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

/// Makes sure crates/app/licenses/ holds the pinned yt-dlp licence files: keeps a file that already matches its pin,
/// otherwise downloads it and refuses anything that does not.
fn ensure_licenses(dir: &Path) -> Res<()> {
    for (name, url, sha) in ytdlp_license_files() {
        let path = dir.join(name);
        if std::fs::read(&path).is_ok_and(|b| license_sha256(&b) == sha) {
            println!("{name}: present, matches its pin");
            continue;
        }
        println!("{name}: downloading {url}");
        let bytes = get(&url)?;
        let got = license_sha256(&bytes);
        if got != sha {
            return Err(format!(
                "{name} from {url} does not match its pinned SHA-256 {sha} (got {got})"
            ));
        }
        write_atomic(&path, &bytes)?;
    }
    Ok(())
}

fn mb(bytes: &[u8]) -> String {
    format!("{:.1} MB", bytes.len() as f64 / 1_048_576.0)
}

/// Downloads `url` and refuses it unless its SHA-256 is exactly `expected` (a pinned value, not one read from the
/// same server).
fn fetch_pinned(label: &str, url: &str, expected: &str) -> Res<Vec<u8>> {
    println!("{label}: downloading {url}");
    let bytes = get(url)?;
    verify(label, &bytes, expected)?;
    println!(
        "{label}: {} downloaded, sha256 matches the pin ({expected})",
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
    // A copy that is already here is kept only when it is exactly the pinned release; anything else (an older or a
    // newer yt-dlp from an earlier build) is replaced, so every build of a tag bundles the same yt-dlp.
    let present = std::fs::read(&ytdlp).is_ok_and(|b| sha256_hex(&b) == YTDLP_SHA256);
    if present && !update {
        println!(
            "yt-dlp: {YTDLP_VERSION} already present ({}).",
            ytdlp.display()
        );
    } else {
        let bytes = fetch_pinned(
            &format!("yt-dlp {YTDLP_VERSION}"),
            &format!("{}/yt-dlp.exe", ytdlp_base()),
            YTDLP_SHA256,
        )?;
        if !is_windows_exe(&bytes) {
            return Err("the yt-dlp download is not a Windows executable".into());
        }
        write_atomic(&ytdlp, &bytes)?;
        println!("yt-dlp: installed {}", ytdlp.display());
    }

    // yt-dlp.exe includes GPLv3+ code: its licence texts are fetched (and pinned) next to the Pyxee notices.
    ensure_licenses(&workspace_root().join("crates/app/licenses"))?;

    println!("\nDone. The app finds it automatically in development; the installer bundles it.");
    println!("Licenses: yt-dlp.exe is a GPLv3+ combined work (a separate program, not linked); the FFmpeg libraries are LGPL v2.1+ (see crates/app/licenses/).");
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

// ---------------------------------------------------------------------- signing

fn signing_from_env() -> Res<Option<Signing>> {
    Signing::from_env(|k| std::env::var(k).ok())
}

fn signing_required() -> bool {
    std::env::var("PYXEE_REQUIRE_SIGNING").is_ok_and(|v| v.trim() == "1")
}

/// `PYXEE_SIGNTOOL`, else the newest signtool in the Windows SDK, else whatever is on PATH.
fn find_signtool() -> PathBuf {
    if let Some(p) = std::env::var_os("PYXEE_SIGNTOOL").filter(|p| !p.is_empty()) {
        return p.into();
    }
    let pf = std::env::var_os("ProgramFiles(x86)")
        .map_or_else(|| PathBuf::from(r"C:\Program Files (x86)"), PathBuf::from);
    let kits = pf.join(r"Windows Kits\10\bin");
    let names: Vec<String> = std::fs::read_dir(&kits)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    release::newest_sdk(&names)
        .map(|n| kits.join(n).join("x64").join("signtool.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("signtool"))
}

/// `cargo xtask sign-file <file>`: signs one file. `tauri build` calls this for every file it signs (its
/// `signCommand`), so the certificate password is read from the environment here and never appears in a config.
fn sign_file(args: &[String]) -> Res<()> {
    let file = args.first().ok_or("usage: cargo xtask sign-file <file>")?;
    let signing = signing_from_env()?.ok_or(
        "no signing certificate is configured (PYXEE_SIGN_CERT_FILE or PYXEE_SIGN_THUMBPRINT)",
    )?;
    if let Cert::File { path, .. } = &signing.cert {
        if !path.is_file() {
            return Err(format!(
                "the certificate file {} does not exist",
                path.display()
            ));
        }
    }
    let argv = signing.signtool_args(Path::new(file));
    println!("signing: signtool {}", redacted(&argv).join(" "));
    let out = Command::new(find_signtool())
        .args(&argv)
        .output()
        .map_err(|e| format!("could not run signtool: {e}. Install the Windows SDK (signtool.exe) or set PYXEE_SIGNTOOL."))?;
    if out.status.success() {
        return Ok(());
    }
    Err(format!(
        "signtool failed ({}):\n{}{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
}

/// Single-quotes `s` for a PowerShell command line.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The Authenticode status of `file` (`Valid`, `NotSigned`, `HashMismatch`, `NotTrusted`...).
fn signature_status(file: &Path) -> Res<String> {
    let script = format!(
        "(Get-AuthenticodeSignature -LiteralPath {}).Status",
        ps_quote(&file.to_string_lossy())
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("could not run PowerShell: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Every file must verify as `Valid`; a build that was meant to be signed and is not is an error.
fn verify_signed(files: &[PathBuf]) -> Res<()> {
    for f in files {
        let status = signature_status(f)?;
        if status != "Valid" {
            return Err(format!(
                "{} is not validly signed (Authenticode status: {status}). A certificate that Windows does not trust (self-signed, or the chain is not installed) also reports this.",
                f.display()
            ));
        }
        println!("signed: {} (Valid)", f.display());
    }
    Ok(())
}

/// The bundle config for `tauri build`: the plain one, or (when signing) one that also makes Tauri call `sign-file`.
fn bundle_config(root: &Path, signing: &Option<Signing>) -> Res<PathBuf> {
    let base_path = root.join("crates/app/tauri.bundle.conf.json");
    if signing.is_none() {
        return Ok(base_path);
    }
    let text = std::fs::read_to_string(&base_path)
        .map_err(|e| format!("cannot read {}: {e}", base_path.display()))?;
    let base: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not valid JSON: {e}", base_path.display()))?;
    let me = std::env::current_exe().map_err(|e| format!("cannot find xtask itself: {e}"))?;
    let conf = signed_tauri_config(&base, &me);
    let out = root.join("target/signing/tauri.bundle.signed.conf.json");
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let pretty = serde_json::to_string_pretty(&conf).map_err(|e| e.to_string())?;
    std::fs::write(&out, pretty).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    Ok(out)
}

/// Prints the path of the bundle config `tauri build` should use: the plain one, or the signing one when a
/// certificate is configured (and an error when signing is required but nothing is configured).
fn signing_config() -> Res<()> {
    let signing = signing_from_env()?;
    if signing.is_none() {
        unsigned_notice(signing_required())?;
    }
    println!("{}", bundle_config(&workspace_root(), &signing)?.display());
    Ok(())
}

// ---------------------------------------------------------- installer and portable

struct Prepared {
    root: PathBuf,
    cli: PathBuf,
    signing: Option<Signing>,
}

/// What both packages need first: yt-dlp, FFmpeg libraries, the built UI, the Tauri CLI, the signing setup.
fn prepare() -> Res<Prepared> {
    let root = workspace_root();
    let signing = signing_from_env()?;
    if signing.is_none() {
        unsigned_notice(signing_required())?;
    }
    tools(&[])?;
    ffmpeg()?;
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
    Ok(Prepared { root, cli, signing })
}

fn installer() -> Res<()> {
    if !cfg!(windows) {
        return Err("the installer can only be built on Windows".into());
    }
    let p = prepare()?;
    let conf = bundle_config(&p.root, &p.signing)?;
    run(
        Command::new(&p.cli)
            .args([
                "build",
                "--bundles",
                bundles_for(env!("CARGO_PKG_VERSION")),
                "--config",
            ])
            .arg(&conf)
            .current_dir(p.root.join("crates/app")),
        "tauri build (installer with yt-dlp bundled)",
    )?;
    let bundle = p.root.join("target/release/bundle");
    println!("\nInstallers are in {}:", bundle.display());
    let mut built = Vec::new();
    for sub in ["nsis", "msi"] {
        if let Ok(rd) = std::fs::read_dir(bundle.join(sub)) {
            for e in rd.flatten().filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x == "exe" || x == "msi")
            }) {
                println!("  {}", e.path().display());
                built.push(e.path());
            }
        }
    }
    if p.signing.is_some() {
        verify_signed(&built)?;
    } else {
        println!(
            "\nThe installer is {}.",
            unsigned_notice(false).unwrap_or_default()
        );
    }
    Ok(())
}

/// The portable zip: `Pyxee.exe`, `yt-dlp.exe`, the licence texts, `portable.txt` (which turns on portable mode: the
/// data folder is `data\` beside the program) and a readme, all in one folder inside the zip.
fn portable() -> Res<()> {
    if !cfg!(windows) {
        return Err("the portable build can only be made on Windows".into());
    }
    let p = prepare()?;
    let conf = bundle_config(&p.root, &p.signing)?;
    run(
        Command::new(&p.cli)
            .args(["build", "--no-bundle", "--config"])
            .arg(&conf)
            .current_dir(p.root.join("crates/app")),
        "tauri build --no-bundle (the program, no installer)",
    )?;
    let version = env!("CARGO_PKG_VERSION");
    let top = portable_folder_name(version);
    let out_dir = p.root.join("target/portable");
    let stage = out_dir.join(&top);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)
        .map_err(|e| format!("cannot create {}: {e}", stage.display()))?;
    for e in portable_entries() {
        let dest = stage.join(e.name);
        match e.from {
            Some(rel) => {
                let src = p.root.join(rel);
                std::fs::copy(&src, &dest).map_err(|err| {
                    format!(
                        "cannot copy {} for the portable zip: {err} (did the build above produce it?)",
                        src.display()
                    )
                })?;
            }
            None => {
                let body = if e.name == PORTABLE_MARKER {
                    "This file turns on Pyxee's portable mode: the library, settings and logs are kept in the data folder next to Pyxee.exe. Delete it to use the normal per-user folder.\r\n"
                } else {
                    PORTABLE_README
                };
                std::fs::write(&dest, body)
                    .map_err(|err| format!("cannot write {}: {err}", dest.display()))?;
            }
        }
    }
    // yt-dlp.exe is somebody else's program and is left as published. Only Pyxee.exe is signed.
    let exe = stage.join("Pyxee.exe");
    if p.signing.is_some() {
        sign_file(&[exe.to_string_lossy().into_owned()])?;
        verify_signed(&[exe])?;
    } else {
        println!(
            "\nPyxee.exe is {}.",
            unsigned_notice(false).unwrap_or_default()
        );
    }
    let mut files = Vec::new();
    for e in portable_entries() {
        let path = stage.join(e.name);
        let bytes =
            std::fs::read(&path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        files.push((e.name.to_string(), bytes));
    }
    let zip = out_dir.join(portable_zip_name(version));
    write_zip(&zip, &top, &files)?;
    check_portable_entries(&top, &zip_names(&zip)?)?;
    let bytes = std::fs::read(&zip).map_err(|e| format!("cannot read {}: {e}", zip.display()))?;
    println!(
        "\nPortable build: {}  ({}, sha256 {})",
        zip.display(),
        mb(&bytes),
        sha256_hex(&bytes)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn a_leftover_ffmpeg_clone_at_another_commit_is_replaced_not_refused() {
        let base = std::env::temp_dir().join(format!("pyxee-xtask-src-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let origin = base.join("origin");
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "-q", "-b", "main"]);
        std::fs::write(origin.join("a.txt"), "one").unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-q", "-m", "one"]);
        let old = git(&origin, &["rev-parse", "HEAD"]);
        std::fs::write(origin.join("a.txt"), "two").unwrap();
        git(&origin, &["commit", "-q", "-am", "two"]);
        git(&origin, &["tag", "v2"]);
        let pinned = git(&origin, &["rev-parse", "HEAD"]);
        let url = origin.to_string_lossy().replace('\\', "/");
        let pin = kit::Pin {
            tag: "v2".into(),
            commit: pinned.clone(),
            url,
        };
        let root = base.join("root");
        // A cached clone from before the pin moved: the first commit.
        let stale = root.join("target/kit/ffmpeg-src");
        std::fs::create_dir_all(&root.join("target/kit")).unwrap();
        git(
            &base,
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                stale.to_str().unwrap(),
            ],
        );
        git(&stale, &["checkout", "-q", &old]);
        assert_ne!(git_head(&stale).unwrap(), pinned);
        let prefix = root.join("target/ffmpeg/x");
        let tgz = ffmpeg_source_archive(&root, &prefix, &pin).expect("stale clone is replaced");
        assert!(!tgz.is_empty());
        assert_eq!(git_head(&stale).unwrap(), pinned);

        // A folder with a broken `.git`, inside a repository of its own (the CI checkout): it must not be read as that
        // repository's commit, and it is replaced too.
        git(&root, &["init", "-q", "-b", "main"]);
        std::fs::write(root.join("f.txt"), "x").unwrap();
        git(&root, &["add", "f.txt"]);
        git(&root, &["commit", "-q", "-m", "workspace"]);
        std::fs::remove_dir_all(&stale).unwrap();
        std::fs::create_dir_all(stale.join(".git")).unwrap();
        assert_eq!(git_head(&stale).unwrap(), "");
        ffmpeg_source_archive(&root, &prefix, &pin).expect("broken clone is replaced");
        assert_eq!(git_head(&stale).unwrap(), pinned);
        let _ = std::fs::remove_dir_all(&base);
    }

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
    fn the_shipped_yt_dlp_licence_files_match_their_pins() {
        let dir = workspace_root().join("crates/app/licenses");
        for (name, url, sha) in ytdlp_license_files() {
            let bytes =
                std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name} is missing: {e}"));
            assert_eq!(
                license_sha256(&bytes),
                sha,
                "{name} differs from its pin; run `cargo xtask tools`"
            );
            assert!(
                url.starts_with("https://raw.githubusercontent.com/"),
                "{url}"
            );
            assert_eq!(sha.len(), 64);
        }
    }

    #[test]
    fn the_licence_pin_follows_the_pinned_yt_dlp_version() {
        let (_, url, _) = &ytdlp_license_files()[0];
        assert!(url.contains(YTDLP_VERSION), "{url}");
    }

    #[test]
    fn yt_dlp_is_pinned_to_one_release_never_latest() {
        assert!(
            ytdlp_base().ends_with(&format!("/download/{YTDLP_VERSION}")),
            "{}",
            ytdlp_base()
        );
        assert!(!ytdlp_base().contains("latest"));
        assert!(
            YTDLP_VERSION
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.'),
            "{YTDLP_VERSION}"
        );
        assert_eq!(YTDLP_SHA256.len(), 64);
        assert!(
            YTDLP_SHA256
                .chars()
                .all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "lower-case hex"
        );
    }

    #[test]
    fn licence_hashes_ignore_windows_line_endings() {
        assert_eq!(license_sha256(b"a\r\nb\r\n"), license_sha256(b"a\nb\n"));
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
    fn the_host_triple_is_read_from_rustc_output() {
        let text = "rustc 1.91.1\nbinary: rustc\nhost: x86_64-pc-windows-msvc\nrelease: 1.91.1\n";
        assert_eq!(parse_host(text).as_deref(), Some("x86_64-pc-windows-msvc"));
        assert_eq!(parse_host("no host here"), None);
    }

    #[test]
    fn a_missing_msys2_is_explained_with_the_one_time_setup() {
        let missing = locate_bash(None, |_| false).unwrap_err();
        assert!(
            missing.contains("MSYS2")
                && missing.contains("winget install")
                && missing.contains("pacman"),
            "{missing}"
        );
        assert!(missing.contains("PYXEE_BASH"), "{missing}");
        // An explicit path is used as given, and found when it exists.
        let custom = locate_bash(Some(r"D:\tools\bash.exe".into()), |p| {
            p == Path::new(r"D:\tools\bash.exe")
        })
        .unwrap();
        assert_eq!(custom, PathBuf::from(r"D:\tools\bash.exe"));
        assert!(locate_bash(Some(r"D:\nope\bash.exe".into()), |_| false)
            .unwrap_err()
            .contains(r"D:\nope\bash.exe"));
    }

    #[test]
    fn missing_msys2_tools_are_named() {
        assert_eq!(
            missing_tools_error("\n"),
            None,
            "nothing printed means nothing is missing"
        );
        assert_eq!(
            missing_tools_error("Copying skeleton files.\nThese files are for the users to personalise their msys2 experience.\n"),
            None
        );
        let e = missing_tools_error("nasm\nmake\n").unwrap();
        assert!(e.contains("nasm, make") && e.contains("pacman"), "{e}");
    }

    #[test]
    fn on_windows_the_build_script_runs_in_a_login_shell_so_msys2_tools_are_on_path() {
        let (script, prefix) = (
            Path::new(r"D:\r\scripts\build-ffmpeg.sh"),
            Path::new(r"D:\r\target\ffmpeg\x"),
        );
        assert_eq!(
            bash_args(true, script, prefix),
            ["-l", "D:/r/scripts/build-ffmpeg.sh", "D:/r/target/ffmpeg/x"]
        );
        assert_eq!(
            bash_args(false, script, prefix),
            ["D:/r/scripts/build-ffmpeg.sh", "D:/r/target/ffmpeg/x"]
        );
    }

    #[test]
    fn bash_gets_forward_slashes() {
        assert_eq!(
            bash_path(Path::new(r"C:\repo\scripts\build.sh")),
            "C:/repo/scripts/build.sh"
        );
    }

    #[test]
    fn powershell_arguments_are_single_quoted_with_quotes_doubled() {
        assert_eq!(ps_quote(r"C:\a b\Pyxee.exe"), r"'C:\a b\Pyxee.exe'");
        assert_eq!(ps_quote("it's"), "'it''s'");
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

    #[test]
    fn vswhere_output_gives_the_first_non_empty_line() {
        assert_eq!(
            parse_vswhere_install(
                "\r\nC:\\Program Files\\Microsoft Visual Studio\\2022\\Enterprise\r\n"
            )
            .as_deref(),
            Some(Path::new(
                r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise"
            ))
        );
        assert_eq!(parse_vswhere_install(""), None);
        assert_eq!(parse_vswhere_install("  \r\n\r\n"), None);
    }

    #[test]
    fn an_environment_dump_is_split_at_the_first_equals_sign() {
        let dump = "Path=C:\\VC\\bin;C:\\Windows\r\nINCLUDE=C:\\VC\\include\r\nodd=a=b\r\n=C:=C:\\work\r\nnot a variable\r\nEMPTY=\r\n";
        assert_eq!(
            parse_env_dump(dump),
            vec![
                ("Path".to_string(), r"C:\VC\bin;C:\Windows".to_string()),
                ("INCLUDE".to_string(), r"C:\VC\include".to_string()),
                ("odd".to_string(), "a=b".to_string()),
                ("EMPTY".to_string(), String::new()),
            ],
            "CRLF stripped, '=' inside a value kept, nameless and non-variable lines skipped"
        );
    }

    #[test]
    fn the_vcvars_script_is_quoted_and_prints_the_environment() {
        let vcvars = Path::new(
            r"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        );
        let script = msvc_env_script(vcvars);
        let lines: Vec<&str> = script.split("\r\n").filter(|l| !l.is_empty()).collect();
        assert!(
            lines.contains(&format!("call \"{}\" >nul", vcvars.display()).as_str()),
            "a path with spaces and parentheses must be quoted: {script}"
        );
        assert_eq!(
            lines.last(),
            Some(&"set"),
            "the environment is printed last"
        );
        assert!(
            lines.iter().any(|l| l.starts_with("if errorlevel 1")),
            "a failing vcvars64.bat must fail the script"
        );
    }

    const VS: &str = r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise";

    #[test]
    fn vcvars_lives_under_vc_auxiliary_build() {
        let p = vcvars_path(Path::new(VS));
        let tail: Vec<_> = p.components().rev().take(4).collect();
        let names: Vec<String> = tail
            .iter()
            .rev()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["VC", "Auxiliary", "Build", "vcvars64.bat"]);
        assert!(p.starts_with(VS));
    }

    #[test]
    fn a_native_tools_prompt_is_left_alone() {
        let env = msvc_environment(
            true,
            || panic!("vswhere must not run when cl is already on PATH"),
            |_| panic!("vcvars must not run when cl is already on PATH"),
        );
        assert_eq!(env, Ok(Vec::new()));
    }

    #[test]
    fn a_plain_shell_gets_the_vcvars_environment() {
        // The GitHub Actions case: Visual Studio is installed but cl.exe is not on PATH.
        let env = msvc_environment(
            false,
            || Some(PathBuf::from(VS)),
            |vcvars| {
                assert_eq!(vcvars, vcvars_path(Path::new(VS)));
                Ok(
                    "Path=C:\\VC\\bin\\Hostx64\\x64;C:\\Windows\r\nINCLUDE=C:\\VC\\include\r\n"
                        .into(),
                )
            },
        )
        .expect("the environment is imported");
        assert!(env
            .iter()
            .any(|(k, v)| k == "Path" && v.contains(r"C:\VC\bin\Hostx64\x64")));
        assert!(env.iter().any(|(k, _)| k == "INCLUDE"));
    }

    #[test]
    fn no_visual_studio_keeps_the_helpful_error() {
        let e = msvc_environment(false, || None, |_| panic!("nothing to run")).unwrap_err();
        assert!(e.contains("cl.exe") && e.contains("Native Tools"), "{e}");
    }

    #[test]
    fn a_vcvars_run_that_prints_no_path_is_an_error_not_a_silent_no_op() {
        for dump in ["", "INCLUDE=C:\\VC\\include\r\n"] {
            let e = msvc_environment(false, || Some(PathBuf::from(VS)), |_| Ok(dump.to_string()));
            assert!(e.is_err(), "dump {dump:?} must be refused");
        }
        let e = msvc_environment(false, || Some(PathBuf::from(VS)), |_| Err("boom".into()));
        assert_eq!(e, Err("boom".to_string()));
    }
}
