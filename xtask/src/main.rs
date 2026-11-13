//! Developer tasks. `cargo xtask tools` downloads yt-dlp (with checksum verification);
//! `cargo xtask ffmpeg` builds the minimal static FFmpeg the audio crate links. Audio is decoded in-process (libav),
//! so yt-dlp is the only helper program.
//!
//! Packaging (Windows): `cargo xtask stage` builds the Rust bridge and the Flutter app and gathers the folder that
//! is shipped; `cargo xtask installer` and `cargo xtask portable` turn that folder into the NSIS installer and the
//! portable zip; `cargo xtask smoke` starts a staged or installed copy. The installer, its uninstaller, `Pyxee.exe`
//! and the installer are code-signed when a certificate is configured (see `release.rs`).

mod release;

use release::{
    check_app_files, check_portable_entries, collect_files, installer_name, numeric_version,
    portable_folder_name, portable_zip_name, redacted, uninstall_script, unsigned_notice,
    write_zip, zip_names, Cert, Signing, Source, APP_EXE, FLUTTER_EXE, FLUTTER_RELEASE_DIRS,
    NOTICE_FILES, PORTABLE_MARKER, PORTABLE_MARKER_BODY, PORTABLE_README, VC_RUNTIME_DLLS,
    YTDLP_EXE,
};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The yt-dlp release that is bundled into installers and the portable zip. Bump it together with `YTDLP_SHA256`
/// (docs/releasing.md has the procedure).
const YTDLP_VERSION: &str = "2026.10.21";
/// SHA-256 of that release's `yt-dlp.exe` (from its published SHA2-256SUMS). `cargo xtask tools` refuses any other bytes.
const YTDLP_SHA256: &str = "33121d192d706e62f684a06f80918e3f3a8ba52ac3a65213b6951c39c7d9f555";

/// Where the pinned release's files are. The in-app updater (removed with the old shell, see docs/migration.md) was separate on purpose: it
/// pulled the latest release at run time, which is how users get a yt-dlp that keeps up with YouTube.
fn ytdlp_base() -> String {
    format!("https://github.com/yt-dlp/yt-dlp/releases/download/{YTDLP_VERSION}")
}
/// `cargo xtask tools` saves yt-dlp as `yt-dlp-<target triple>.exe` (the bridge looks for that name in a development tree).
const TRIPLE: &str = "x86_64-pc-windows-msvc";

type Res<T> = Result<T, String>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("tools") => tools(&args[1..]),
        Some("ffmpeg") => ffmpeg(),
        Some("stage") => stage().map(|_| ()),
        Some("installer") => installer(),
        Some("portable") => portable(),
        Some("smoke") => smoke(&args[1..]),
        Some("sign-file") => sign_file(&args[1..]),
        Some("update-keygen") => update_keygen(),
        Some("sign-sums") => sign_sums(&args[1..]),
        _ => Err("usage:\n  cargo xtask tools [--update] [--dir <folder>]   download the pinned yt-dlp (--update: download it again)\n  cargo xtask ffmpeg                              build the minimal static FFmpeg (once per FFmpeg change)\n  cargo xtask stage                               build the bridge and the Flutter app and gather the folder that is shipped (target/stage/Pyxee)\n  cargo xtask installer                           build the Windows installer (signed when a certificate is configured)\n  cargo xtask portable                            build the portable zip (signed program when a certificate is configured)\n  cargo xtask smoke [folder]                      start a staged or installed copy for a few seconds and run its yt-dlp\n  cargo xtask sign-file <file>                    sign one file with the configured certificate (the installer script calls it for the uninstaller)\n  cargo xtask update-keygen                       make the key pair that signs release checksum lists (once; see docs/releasing.md)\n  cargo xtask sign-sums <SHA256SUMS.txt>          write <file>.sig, signed with PYXEE_UPDATE_SIGNING_SEED (used by the release workflow)".into()),
    };
    if let Err(e) = result {
        eprintln!("\nerror: {e}");
        std::process::exit(1);
    }
}

// ------------------------------------------------------------------ update signing

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

/// The key pair the release workflow signs `SHA256SUMS.txt` with: the seed (64 hex characters) is the secret, the
/// public key (64 hex characters) is built into the program.
pub fn keypair_from_seed(seed_hex: &str) -> Res<ring::signature::Ed25519KeyPair> {
    let seed = from_hex(seed_hex)
        .filter(|s| s.len() == 32)
        .ok_or("the signing seed must be 64 hex characters")?;
    ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed)
        .map_err(|_| "the signing seed was rejected".to_string())
}

/// The detached signature of `message`, in hex.
pub fn sign_hex(seed_hex: &str, message: &[u8]) -> Res<String> {
    Ok(to_hex(keypair_from_seed(seed_hex)?.sign(message).as_ref()))
}

fn update_keygen() -> Res<()> {
    use ring::signature::KeyPair;
    let mut seed = [0u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut seed)
        .map_err(|_| "no random numbers available")?;
    let seed_hex = to_hex(&seed);
    let public = to_hex(keypair_from_seed(&seed_hex)?.public_key().as_ref());
    println!("Signing seed (SECRET: store it as the repository secret PYXEE_UPDATE_SIGNING_SEED, then delete this output):");
    println!("{seed_hex}\n");
    println!("Public key (store it as the repository variable PYXEE_UPDATE_PUBLIC_KEY; it is built into releases):");
    println!("{public}");
    Ok(())
}

fn sign_sums(args: &[String]) -> Res<()> {
    let file = args
        .first()
        .ok_or("usage: cargo xtask sign-sums <SHA256SUMS.txt>")?;
    let seed = std::env::var("PYXEE_UPDATE_SIGNING_SEED")
        .map_err(|_| "PYXEE_UPDATE_SIGNING_SEED is not set")?;
    // The exact bytes that are published: the app verifies the file as downloaded.
    let bytes = std::fs::read(file).map_err(|e| format!("could not read {file}: {e}"))?;
    let signature = sign_hex(&seed, &bytes)?;
    let out = format!("{file}.sig");
    std::fs::write(&out, format!("{signature}\n"))
        .map_err(|e| format!("could not write {out}: {e}"))?;
    println!("signed {file} -> {out}");
    Ok(())
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
        Err(format!("{what}: checksum mismatch (expected {expected}, got {got}). The download is corrupt or is not the file the release lists; nothing was installed."))
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
    let agent: ureq::Agent = pyxee_net::agent_config()
        .timeout_connect(Some(std::time::Duration::from_secs(20)))
        .timeout_per_read(Some(std::time::Duration::from_secs(120)))
        .build()
        .into();
    let resp = pyxee_net::retry_stale(|| agent.get(url).call()).map_err(|e| match e {
        ureq::Error::StatusCode(c) => format!("{url} answered HTTP {c}"),
        e => format!("could not reach {url}: {e}. Check your internet connection (and any proxy or firewall)."),
    })?;
    let mut bytes = Vec::new();
    resp.into_body()
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("download of {url} was interrupted: {e}"))?;
    Ok(bytes)
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
    let mut dir = workspace_root().join("tools");
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

    println!("\nDone. The app finds it automatically in development; the installer bundles it.");
    println!("Licenses: yt-dlp.exe is a GPLv3+ combined work (a separate program, not linked); the FFmpeg libraries are LGPL v2.1+ (see licenses/).");
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

/// `cargo xtask sign-file <file>`: signs one file. The installer script calls this for the uninstaller it generates
/// (`!uninstfinalize`), so the certificate password is read from the environment here and never appears in a script.
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

// ------------------------------------------------------- build, stage, installer, portable

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

fn flutter() -> Command {
    Command::new(if cfg!(windows) {
        "flutter.bat"
    } else {
        "flutter"
    })
}

/// `PYXEE_BUILD_READY=1` says the Rust bridge and the Flutter app were built just before (the release workflow does
/// that in steps of its own, so its log shows which one failed), so staging does not build them again. Unset, empty
/// or `0`: it always builds them, as it does on a developer's machine.
fn build_ready(value: Option<std::ffi::OsString>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

/// The first of `FLUTTER_RELEASE_DIRS` that `exists`.
fn pick_release_dir(root: &Path, exists: impl Fn(&Path) -> bool) -> Res<PathBuf> {
    FLUTTER_RELEASE_DIRS
        .iter()
        .map(|d| root.join(d))
        .find(|d| exists(d))
        .ok_or_else(|| {
            format!(
                "the Flutter release build was not found in {} (run `flutter build windows --release` in ui_flutter)",
                FLUTTER_RELEASE_DIRS.join(" or ")
            )
        })
}

/// The Rust bridge (`pyxee_bridge.dll`, which the Flutter build copies in through `ui_flutter/hook/build.dart`),
/// then the Flutter app. The two build-time variables `PYXEE_UPDATE_PUBLIC_KEY` and `PYXEE_DISCORD_CLIENT_ID` are
/// compiled into the bridge, so they are read from the environment here.
fn build_app(root: &Path) -> Res<()> {
    run(
        cargo()
            .args(["build", "-p", "pyxee-bridge", "--release", "--locked"])
            .current_dir(root),
        "cargo build -p pyxee-bridge --release",
    )?;
    let ui = root.join("ui_flutter");
    run(
        flutter().args(["pub", "get"]).current_dir(&ui),
        "flutter pub get",
    )?;
    run(
        flutter()
            .args(["build", "windows", "--release"])
            .current_dir(&ui),
        "flutter build windows --release",
    )
}

fn copy_tree(src: &Path, dst: &Path) -> Res<()> {
    std::fs::create_dir_all(dst).map_err(|e| format!("cannot create {}: {e}", dst.display()))?;
    let rd = std::fs::read_dir(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
    for entry in rd.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path, what: &str) -> Res<()> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("cannot copy {what} ({}): {e}", from.display()))
}

/// The folder that is shipped, gathered under `target/stage/Pyxee`.
struct Staged {
    root: PathBuf,
    files: Vec<(String, PathBuf)>,
    signing: Option<Signing>,
}

/// What the installer and the portable zip both start from: yt-dlp, the built bridge and Flutter app, the Visual C++
/// runtime, the licence texts, `Pyxee.exe` signed when a certificate is configured, and a check of the result.
fn stage() -> Res<Staged> {
    if !cfg!(windows) {
        return Err("the app can only be packaged on Windows".into());
    }
    let root = workspace_root();
    let signing = signing_from_env()?;
    if signing.is_none() {
        unsigned_notice(signing_required())?;
    }
    tools(&[])?;
    if build_ready(std::env::var_os("PYXEE_BUILD_READY")) {
        println!("Build: the bridge and the Flutter app are already built (PYXEE_BUILD_READY), not building them again.");
    } else {
        ffmpeg()?;
        build_app(&root)?;
    }
    let release = pick_release_dir(&root, Path::is_dir)?;
    let stage = root.join("target/stage/Pyxee");
    let _ = std::fs::remove_dir_all(&stage);
    copy_tree(&release, &stage)?;

    let flutter_exe = stage.join(FLUTTER_EXE);
    if !flutter_exe.is_file() {
        return Err(format!(
            "{FLUTTER_EXE} is not in {} (did BINARY_NAME in ui_flutter/windows/CMakeLists.txt change?)",
            release.display()
        ));
    }
    std::fs::rename(&flutter_exe, stage.join(APP_EXE))
        .map_err(|e| format!("cannot rename {FLUTTER_EXE} to {APP_EXE}: {e}"))?;
    // yt-dlp sits next to the program, where the bridge looks for it first after PYXEE_YTDLP.
    copy_file(
        &root.join("tools").join(format!("yt-dlp-{TRIPLE}.exe")),
        &stage.join(YTDLP_EXE),
        "yt-dlp (did `cargo xtask tools` run?)",
    )?;
    let system = std::env::var_os("SystemRoot")
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
        .join("System32");
    for dll in VC_RUNTIME_DLLS {
        let dest = stage.join(dll);
        if !dest.is_file() {
            copy_file(
                &system.join(dll),
                &dest,
                &format!("{dll} (install the Visual C++ Redistributable)"),
            )?;
        }
    }
    for (from, name) in NOTICE_FILES {
        copy_file(&root.join(from), &stage.join(name), name)?;
    }

    // Only Pyxee.exe is signed here. yt-dlp.exe is someone else's program and is left as published.
    let exe = stage.join(APP_EXE);
    if signing.is_some() {
        sign_file(&[exe.to_string_lossy().into_owned()])?;
        verify_signed(&[exe])?;
    } else {
        println!(
            "\nPyxee.exe is {}.",
            unsigned_notice(false).unwrap_or_default()
        );
    }
    let files = collect_files(&stage)?;
    let names: Vec<String> = files.iter().map(|f| f.0.clone()).collect();
    check_app_files(&names, false)?;
    println!("\nStaged {} files in {}", files.len(), stage.display());
    Ok(Staged {
        root,
        files,
        signing,
    })
}

/// `makensis.exe`: `PYXEE_MAKENSIS`, else the default install folders, else whatever is on PATH.
fn find_makensis() -> PathBuf {
    if let Some(p) = std::env::var_os("PYXEE_MAKENSIS").filter(|p| !p.is_empty()) {
        return p.into();
    }
    ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|pf| PathBuf::from(pf).join("NSIS").join("makensis.exe"))
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("makensis"))
}

fn installer() -> Res<()> {
    let st = stage()?;
    let version = env!("CARGO_PKG_VERSION");
    let out_dir = st.root.join("target/installer");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let names: Vec<String> = st.files.iter().map(|f| f.0.clone()).collect();
    let list = out_dir.join("uninstall-files.nsh");
    std::fs::write(&list, uninstall_script(&names)?)
        .map_err(|e| format!("cannot write {}: {e}", list.display()))?;
    let out = out_dir.join(installer_name(version));
    let _ = std::fs::remove_file(&out);

    let mut cmd = Command::new(find_makensis());
    cmd.args(["/V2", "/INPUTCHARSET", "UTF8"])
        .arg(format!("/DVERSION={version}"))
        .arg(format!("/DVI_VERSION={}", numeric_version(version)))
        .arg(format!(
            "/DSTAGEDIR={}",
            st.root.join("target/stage/Pyxee").display()
        ))
        .arg(format!("/DOUTFILE={}", out.display()))
        .arg(format!("/DUNINSTALL_LIST={}", list.display()))
        .arg(format!(
            "/DASSETS={}",
            st.root.join("assets/icons").display()
        ));
    if st.signing.is_some() {
        // The installer script signs the uninstaller it generates by calling `xtask sign-file`.
        let me = std::env::current_exe().map_err(|e| format!("cannot find xtask itself: {e}"))?;
        cmd.arg(format!("/DSIGN_XTASK={}", me.display()));
    }
    cmd.arg(st.root.join("packaging/windows/installer.nsi"))
        .current_dir(&st.root);
    run(&mut cmd, "makensis (the installer)")
        .map_err(|e| format!("{e}. Install NSIS (https://nsis.sourceforge.io) or set PYXEE_MAKENSIS to makensis.exe."))?;
    if !out.is_file() {
        return Err(format!("makensis did not produce {}", out.display()));
    }
    if st.signing.is_some() {
        sign_file(&[out.to_string_lossy().into_owned()])?;
        verify_signed(std::slice::from_ref(&out))?;
    } else {
        println!(
            "\nThe installer is {}.",
            unsigned_notice(false).unwrap_or_default()
        );
    }
    let bytes = std::fs::read(&out).map_err(|e| format!("cannot read {}: {e}", out.display()))?;
    println!(
        "\nInstaller: {}  ({}, sha256 {})",
        out.display(),
        mb(&bytes),
        sha256_hex(&bytes)
    );
    Ok(())
}

/// The portable zip: the staged folder plus `portable.txt` (which turns on portable mode: the data folder is
/// `data\` beside the program) and a readme, all in one folder inside the zip.
fn portable() -> Res<()> {
    let st = stage()?;
    let version = env!("CARGO_PKG_VERSION");
    let top = portable_folder_name(version);
    let out_dir = st.root.join("target/portable");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let mut files: Vec<(String, Source)> = st
        .files
        .iter()
        .map(|(name, path)| (name.clone(), Source::Path(path.clone())))
        .collect();
    files.push((
        PORTABLE_MARKER.into(),
        Source::Bytes(PORTABLE_MARKER_BODY.as_bytes().to_vec()),
    ));
    files.push((
        "README.txt".into(),
        Source::Bytes(PORTABLE_README.as_bytes().to_vec()),
    ));
    files.sort_by(|a, b| a.0.cmp(&b.0));
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

/// How long the smoke test lets the program run: `PYXEE_SMOKE_SECONDS`, default 10.
fn smoke_seconds(value: Option<String>) -> u64 {
    value
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(10)
}

/// `cargo xtask smoke [folder]`: runs the folder's `yt-dlp.exe --version`, then starts `Pyxee.exe` from a portable
/// copy of the folder (so nothing is written to the user's profile, and a copy that was installed is left alone),
/// waits, and fails if it is no longer running. The folder is `target/stage/Pyxee` unless one is given (the
/// release workflow also points it at the folder the installer has just installed into).
fn smoke(args: &[String]) -> Res<()> {
    if !cfg!(windows) {
        return Err("the smoke test can only run on Windows".into());
    }
    let dir = args.first().map_or_else(
        || workspace_root().join("target/stage/Pyxee"),
        PathBuf::from,
    );
    if !dir.join(APP_EXE).is_file() {
        return Err(format!("{} is not in {}", APP_EXE, dir.display()));
    }
    let out = Command::new(dir.join(YTDLP_EXE))
        .arg("--version")
        .output()
        .map_err(|e| format!("cannot run {YTDLP_EXE}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{YTDLP_EXE} --version failed ({})", out.status));
    }
    println!("yt-dlp {}", String::from_utf8_lossy(&out.stdout).trim());

    let copy = std::env::temp_dir().join(format!("pyxee-smoke-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&copy);
    copy_tree(&dir, &copy)?;
    std::fs::write(copy.join(PORTABLE_MARKER), PORTABLE_MARKER_BODY)
        .map_err(|e| format!("cannot write {PORTABLE_MARKER}: {e}"))?;
    let seconds = smoke_seconds(std::env::var("PYXEE_SMOKE_SECONDS").ok());
    println!("Starting {APP_EXE} for {seconds} seconds...");
    let mut child = Command::new(copy.join(APP_EXE))
        .current_dir(&copy)
        .spawn()
        .map_err(|e| format!("cannot start {APP_EXE}: {e}"))?;
    let mut result = Ok(());
    for _ in 0..seconds {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if let Ok(Some(status)) = child.try_wait() {
            result = Err(format!(
                "{APP_EXE} stopped by itself after less than {seconds} seconds ({status})"
            ));
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&copy);
    result?;
    println!("{APP_EXE} was still running after {seconds} seconds.");
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn build_ready_only_for_a_real_value() {
        use std::ffi::OsString;
        assert!(!build_ready(None));
        assert!(!build_ready(Some(OsString::new())));
        assert!(!build_ready(Some(OsString::from("0"))));
        assert!(build_ready(Some(OsString::from("1"))));
    }

    #[test]
    fn the_flutter_release_folder_is_found_with_or_without_x64() {
        let root = Path::new("repo");
        let with_x64 = root.join(FLUTTER_RELEASE_DIRS[0]);
        let without = root.join(FLUTTER_RELEASE_DIRS[1]);
        assert_eq!(pick_release_dir(root, |p| p == with_x64).unwrap(), with_x64);
        assert_eq!(pick_release_dir(root, |p| p == without).unwrap(), without);
        assert_eq!(
            pick_release_dir(root, |_| true).unwrap(),
            with_x64,
            "x64 wins when both exist"
        );
        assert!(pick_release_dir(root, |_| false)
            .unwrap_err()
            .contains("flutter build windows"));
    }

    #[test]
    fn the_smoke_test_waits_ten_seconds_unless_told_otherwise() {
        assert_eq!(smoke_seconds(None), 10);
        assert_eq!(smoke_seconds(Some("25".into())), 25);
        assert_eq!(smoke_seconds(Some("0".into())), 10);
        assert_eq!(smoke_seconds(Some("soon".into())), 10);
    }

    #[test]
    fn a_signed_checksum_list_verifies_with_the_public_key_and_not_after_a_change() {
        use ring::signature::{KeyPair, UnparsedPublicKey, ED25519};
        let seed = "07".repeat(32);
        let public = keypair_from_seed(&seed)
            .unwrap()
            .public_key()
            .as_ref()
            .to_vec();
        let sums = b"abc  Pyxee_1.0.0_x64-setup.exe\n";
        let sig = from_hex(&sign_hex(&seed, sums).unwrap()).unwrap();
        let key = UnparsedPublicKey::new(&ED25519, &public);
        assert!(key.verify(sums, &sig).is_ok());
        assert!(key
            .verify(b"abd  Pyxee_1.0.0_x64-setup.exe\n", &sig)
            .is_err());
        assert!(keypair_from_seed("zz").is_err());
        assert!(keypair_from_seed(&"07".repeat(31)).is_err());
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
