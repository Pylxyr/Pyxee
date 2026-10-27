//! In-app updates for Pyxee itself and for yt-dlp. This file is the network and process glue; every decision (is it
//! newer, which file, does the checksum match, replace safely) lives in `pyxee_shell::update` and is tested there.
//!
//! * Pyxee: looks at the GitHub releases (see docs/releasing.md), downloads the installer, checks it against the
//!   release's `SHA256SUMS.txt`, starts it and quits so the installer can replace the files. When this copy of Pyxee
//!   is itself validly code-signed, the installer must also be validly signed by the same signer subject (see
//!   `update::check_publisher`); an unsigned copy keeps the checksum-only check, so it can still update.
//! * yt-dlp: checks its published SHA-256 list, downloads, verifies, runs `--version` on the new file and only then
//!   puts it in the data folder, where `YtDlpSource` prefers it over the installed copy.

use crate::state::AppState;
use pyxee_shell::update::{self, ToolStatus};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, State};
use ts_rs::TS;

const REPO: &str = "Pylxyr/Pyxee";
const YTDLP_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const YTDLP_FILE: &str = "yt-dlp.exe";
/// Nothing we download is anywhere near this; it stops a wrong URL from filling the disk.
const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;
/// The release list and the checksum lists are small; anything bigger is a wrong answer, not a file to keep in memory.
const MAX_TEXT: u64 = 2 * 1024 * 1024;
/// The Tauri event that carries `UpdateProgressDto`.
pub const UPDATE_PROGRESS_EVENT: &str = "update-progress";
/// A download reports its progress at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppUpdateDto {
    pub current: String,
    pub latest: String,
    pub available: bool,
    // The release page, always present so there is something to open even when it cannot be installed in place.
    pub notes_url: String,
    // The release description (Markdown), shown as the changelog when an update is found. Empty when there is none.
    pub notes: String,
    // True when the release carries an installer and a checksum list, so "Install" can verify it, and this copy is an
    // installed one (a portable copy is replaced by downloading the new zip, never by running an installer).
    pub installable: bool,
    // True for a portable copy (`portable.txt` next to the program).
    pub portable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct YtdlpUpdateDto {
    pub available: bool,
    pub current_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateTarget {
    App,
    Ytdlp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdatePhase {
    Downloading,
    Verifying,
    Testing,
    Starting,
}

/// Where an install is: sent as the `update-progress` event. `total` is unknown when the server sends no length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateProgressDto {
    #[ts(type = "\"app\" | \"ytdlp\"")]
    pub target: UpdateTarget,
    #[ts(type = "\"downloading\" | \"verifying\" | \"testing\" | \"starting\"")]
    pub phase: UpdatePhase,
    #[ts(type = "number")]
    pub received: u64,
    #[ts(type = "number | null")]
    pub total: Option<u64>,
}

fn emit_progress(
    app: &AppHandle,
    target: UpdateTarget,
    phase: UpdatePhase,
    received: u64,
    total: Option<u64>,
) {
    let dto = UpdateProgressDto {
        target,
        phase,
        received,
        total,
    };
    if let Err(e) = crate::shell::emit_noted(app, UPDATE_PROGRESS_EVENT, dto) {
        log::warn!("could not send the update progress: {e}");
    }
}

/// Only one install runs at a time (Pyxee or yt-dlp): a second one would fight over the same files.
static INSTALLING: AtomicBool = AtomicBool::new(false);

struct InstallGuard;

impl InstallGuard {
    fn acquire() -> Result<Self, String> {
        INSTALLING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| Self)
            .map_err(|_| "An update is already being installed.".to_string())
    }
}

impl Drop for InstallGuard {
    fn drop(&mut self) {
        INSTALLING.store(false, Ordering::SeqCst);
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .https_only(true)
        .user_agent(concat!("Pyxee/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(120))
        .build()
}

/// Updates are only ever fetched over https (the agent also refuses a redirect to plain http).
fn require_https(url: &str) -> Result<(), String> {
    if url.to_ascii_lowercase().starts_with("https://") {
        Ok(())
    } else {
        Err(format!(
            "Refusing to download from an insecure address: {url}"
        ))
    }
}

fn request(url: &str) -> Result<ureq::Response, String> {
    require_https(url)?;
    agent()
        .get(url)
        .set("Accept", "application/vnd.github+json, */*")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => format!(
                "{url} was not found. The release may have been removed, or the repository is private."
            ),
            ureq::Error::Status(403 | 429, _) => {
                "GitHub is limiting requests right now. Try again in a few minutes.".to_string()
            }
            ureq::Error::Status(c, _) => format!("{url} answered HTTP {c}"),
            e => format!("Could not reach {url}: {e}. Check your internet connection (and any proxy or firewall)."),
        })
}

/// Reads at most `limit` bytes; more than that is an error, never a silently cut-off answer.
fn read_limited(reader: impl Read, limit: u64, what: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("The download of {what} was interrupted: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{what} is larger than expected."));
    }
    Ok(bytes)
}

fn get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let resp = request(url)?;
    read_limited(resp.into_reader(), limit, url)
}

/// Downloads a large file in 64 KiB steps and reports `(received, total)` at most every 100 ms and once at the end.
/// A file above `MAX_DOWNLOAD` is an error, never a silently cut-off download.
fn download(url: &str, on_progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<Vec<u8>, String> {
    const TOO_BIG: &str = "The download is larger than expected";
    let resp = request(url)?;
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.trim().parse::<u64>().ok());
    if total.is_some_and(|t| t > MAX_DOWNLOAD) {
        return Err(TOO_BIG.into());
    }
    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let mut last: Option<Instant> = None;
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("The download of {url} was interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        if bytes.len() as u64 + n as u64 > MAX_DOWNLOAD {
            return Err(TOO_BIG.into());
        }
        bytes.extend_from_slice(&chunk[..n]);
        if last.is_none_or(|t| t.elapsed() >= PROGRESS_EVERY) {
            on_progress(bytes.len() as u64, total);
            last = Some(Instant::now());
        }
    }
    on_progress(bytes.len() as u64, total);
    Ok(bytes)
}

fn text(url: &str) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&get(url, MAX_TEXT)?).into_owned())
}

// ------------------------------------------------------------------ Pyxee

fn latest_release(current: &str) -> Result<Option<update::Release>, String> {
    let beta = update::Version::parse(current).is_some_and(|v| v.is_prerelease());
    let json = text(&format!(
        "https://api.github.com/repos/{REPO}/releases?per_page=20"
    ))?;
    update::newest_release(&json, beta).map_err(|e| e.to_string())
}

/// The data folder of a portable copy, or `None` for an installed one (see `pyxee_shell::datadir::portable_dir`).
pub fn portable_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    pyxee_shell::datadir::portable_dir(exe.parent()?)
}

fn check_app_blocking() -> Result<AppUpdateDto, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let portable = portable_dir().is_some();
    Ok(match latest_release(&current)? {
        Some(r) => AppUpdateDto {
            available: update::is_newer(&current, &r.tag),
            latest: r.version_text(),
            notes_url: r.page_url.clone(),
            notes: r.notes.clone(),
            installable: !portable && r.installer.is_some() && r.sums_url.is_some(),
            portable,
            current,
        },
        None => AppUpdateDto {
            latest: current.clone(),
            available: false,
            notes_url: format!("https://github.com/{REPO}/releases"),
            notes: String::new(),
            installable: false,
            portable,
            current,
        },
    })
}

/// Downloads and verifies the installer; returns where it is and the checksum it was verified against.
/// Where downloaded installers wait to be started.
fn update_dir() -> PathBuf {
    std::env::temp_dir().join("pyxee-update")
}

/// Empties `dir` (and removes it). A file that is still in use, such as an installer that is only now finishing,
/// stays; it goes the next time.
fn clear_dir(dir: &Path) {
    if let Err(e) = std::fs::remove_dir_all(dir) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::debug!("update: could not clear {}: {e}", dir.display());
        }
    }
}

/// Removes the installers earlier updates downloaded. Run once at start-up: by then the installer that started
/// this copy has done its work.
pub fn clear_leftovers() {
    clear_dir(&update_dir());
}

fn fetch_installer(app: &AppHandle) -> Result<(PathBuf, String), String> {
    if portable_dir().is_some() {
        return Err("This is a portable copy: download the new zip from the release page and unpack it over this folder (your data folder stays).".into());
    }
    let current = env!("CARGO_PKG_VERSION");
    let release = latest_release(current)?.ok_or("No release was found.")?;
    if !update::is_newer(current, &release.tag) {
        return Err("Pyxee is already up to date.".into());
    }
    let (Some(installer), Some(sums_url)) = (release.installer.as_ref(), release.sums_url.as_ref())
    else {
        return Err("This release has no installer with a checksum list attached. Open the release page to download it yourself.".into());
    };
    let sums = text(sums_url)?;
    let expected = update::parse_sums(&sums, &installer.name)
        .ok_or_else(|| update::UpdateError::NotListed(installer.name.clone()).to_string())?;
    let bytes = download(&installer.url, &mut |received, total| {
        emit_progress(
            app,
            UpdateTarget::App,
            UpdatePhase::Downloading,
            received,
            total,
        );
    })?;
    let size = bytes.len() as u64;
    emit_progress(
        app,
        UpdateTarget::App,
        UpdatePhase::Verifying,
        size,
        Some(size),
    );
    update::verify(&bytes, &expected).map_err(|e| e.to_string())?;
    let dir = update_dir();
    // Whatever an earlier update left here goes first, so only this installer is kept.
    clear_dir(&dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    // The name comes from the release; keep only its file name so it can never point outside the folder.
    let name = Path::new(&installer.name)
        .file_name()
        .ok_or("The installer has no usable file name.")?;
    let path = dir.join(name);
    std::fs::write(&path, &bytes).map_err(|e| format!("Could not save the installer: {e}"))?;
    // The checksum above proves the file is the one the release lists; this proves, for a signed copy of Pyxee,
    // that it comes from the same publisher.
    gate_installer(&path)?;
    log::info!(
        "update: {} downloaded and verified ({expected})",
        path.display()
    );
    Ok((path, expected))
}

/// The environment variable that carries the file name to the signature script (see `update::signature_script`).
const SIGNATURE_PATH_VAR: &str = "PYXEE_SIGCHECK_PATH";

/// Reads Authenticode signatures with Windows' own verification (`Get-AuthenticodeSignature`, which uses
/// WinVerifyTrust). Windows PowerShell is started from the system folder, hidden, with the file name passed through
/// the environment.
struct WindowsSignatures;

impl update::SignatureChecker for WindowsSignatures {
    fn signature(&self, file: &Path) -> Result<update::Signature, String> {
        if !cfg!(windows) {
            // Pyxee is a Windows program; a development build elsewhere has no Authenticode to compare.
            return Ok(update::Signature::Unsigned);
        }
        let root = std::env::var_os("SystemRoot").ok_or("SystemRoot is not set")?;
        let powershell = Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let mut cmd = Command::new(powershell);
        cmd.args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(update::signature_script(SIGNATURE_PATH_VAR))
            .env(SIGNATURE_PATH_VAR, file)
            // Windows PowerShell must use its own modules. A PSModulePath inherited from PowerShell 7 (as on CI
            // runners, or from a developer's shell) makes it load an incompatible Security module, so
            // `Get-AuthenticodeSignature` yields nothing and the status comes back empty.
            .env_remove("PSModulePath")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let out = cmd
            .output()
            .map_err(|e| format!("could not start PowerShell: {e}"))?;
        Ok(update::parse_signature_output(&String::from_utf8_lossy(
            &out.stdout,
        )))
    }
}

/// The publisher gate for a downloaded installer: see `update::check_publisher`. A refused installer is deleted.
fn gate_installer(path: &Path) -> Result<(), String> {
    let running = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log::warn!("update: could not find the running program to compare signers ({e}); the checksum alone was checked");
            return Ok(());
        }
    };
    match update::gate_installer_file(&WindowsSignatures, &running, path) {
        Ok(update::PublisherCheck::Matched { subject }) => {
            log::info!(
                "update: the installer is signed by the same signer as this copy ({subject})"
            );
            Ok(())
        }
        Ok(update::PublisherCheck::NotRequired) => {
            log::info!("update: this copy is not signed, so only the checksum was checked");
            Ok(())
        }
        Err(e) => {
            log::warn!("update: installer refused: {e}");
            Err(e.to_string())
        }
    }
}

/// Hashes the installer again right before it is started. It sits in the temp folder, which other programs of this
/// user can write to, so what was checked after the download is not necessarily what is on disk now.
fn recheck_installer(path: &Path, expected: &str) -> Result<(), String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Could not read the installer again before starting it: {e}"))?;
    update::verify(&bytes, expected).map_err(|e| e.to_string())
}

fn launch_installer(path: &Path, expected: &str) -> Result<(), String> {
    recheck_installer(path, expected)?;
    Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start the installer: {e}"))
}

#[tauri::command]
pub async fn update_check_app() -> Result<AppUpdateDto, String> {
    tauri::async_runtime::spawn_blocking(check_app_blocking)
        .await
        .map_err(|e| e.to_string())?
}

/// Downloads, verifies and starts the installer, then quits so it can replace the files.
#[tauri::command]
pub async fn update_install_app(app: AppHandle) -> Result<(), String> {
    let _guard = InstallGuard::acquire()?;
    let handle = app.clone();
    let fetched = tauri::async_runtime::spawn_blocking(move || fetch_installer(&handle))
        .await
        .map_err(|e| e.to_string())??;
    let (path, expected) = fetched;
    emit_progress(&app, UpdateTarget::App, UpdatePhase::Starting, 0, None);
    launch_installer(&path, &expected)?;
    crate::shell::quit(&app);
    Ok(())
}

// ----------------------------------------------------------------- yt-dlp

/// Where the updater keeps its copy of yt-dlp, which `YtDlpSource` prefers over the installed one.
pub fn managed_ytdlp_path(data_dir: &Path) -> PathBuf {
    data_dir.join("tools").join(YTDLP_FILE)
}

fn data_dir(state: &AppState) -> Result<PathBuf, String> {
    state
        .session_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "Pyxee has no data folder to keep an updated yt-dlp in.".to_string())
}

fn guard_env() -> Result<(), String> {
    if std::env::var_os("PYXEE_YTDLP").is_some_and(|v| !v.is_empty()) {
        return Err("yt-dlp is set by the PYXEE_YTDLP variable, so Pyxee leaves it alone. Update that copy yourself.".into());
    }
    Ok(())
}

/// The yt-dlp the next search would start: the updater's copy if there is one, else the installed one.
fn current_ytdlp(data_dir: &Path) -> PathBuf {
    let managed = managed_ytdlp_path(data_dir);
    if managed.is_file() {
        managed
    } else {
        crate::tools::locate("yt-dlp", "PYXEE_YTDLP")
    }
}

fn version_of(path: &Path) -> Result<String, String> {
    let mut cmd = Command::new(path);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("exit status {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn check_ytdlp_blocking(dir: &Path) -> Result<YtdlpUpdateDto, String> {
    let current = current_ytdlp(dir);
    let sums = text(&format!("{YTDLP_BASE}/SHA2-256SUMS"))?;
    // A program Windows has to find on PATH is not a file we can hash: offer to install our own copy.
    let status = match std::fs::read(&current) {
        Ok(bytes) => update::tool_status(&update::sha256_hex(&bytes), &sums, YTDLP_FILE)
            .map_err(|e| e.to_string())?,
        Err(_) => ToolStatus::Available {
            expected: String::new(),
        },
    };
    Ok(YtdlpUpdateDto {
        available: matches!(status, ToolStatus::Available { .. }),
        current_version: version_of(&current).ok(),
    })
}

fn install_ytdlp_blocking(app: &AppHandle, dir: &Path) -> Result<String, String> {
    let sums = text(&format!("{YTDLP_BASE}/SHA2-256SUMS"))?;
    let expected = update::parse_sums(&sums, YTDLP_FILE)
        .ok_or_else(|| update::UpdateError::NotListed(YTDLP_FILE.into()).to_string())?;
    let bytes = download(
        &format!("{YTDLP_BASE}/{YTDLP_FILE}"),
        &mut |received, total| {
            emit_progress(
                app,
                UpdateTarget::Ytdlp,
                UpdatePhase::Downloading,
                received,
                total,
            );
        },
    )?;
    let size = bytes.len() as u64;
    emit_progress(
        app,
        UpdateTarget::Ytdlp,
        UpdatePhase::Verifying,
        size,
        Some(size),
    );
    let target = managed_ytdlp_path(dir);
    let version = update::install_verified(&target, &bytes, &expected, |tmp| {
        emit_progress(
            app,
            UpdateTarget::Ytdlp,
            UpdatePhase::Testing,
            size,
            Some(size),
        );
        version_of(tmp)
    })
    .map_err(|e| e.to_string())?;
    log::info!("update: yt-dlp {version} installed at {}", target.display());
    Ok(version)
}

#[tauri::command]
pub async fn update_check_ytdlp(state: State<'_, AppState>) -> Result<YtdlpUpdateDto, String> {
    guard_env()?;
    let dir = data_dir(&state)?;
    tauri::async_runtime::spawn_blocking(move || check_ytdlp_blocking(&dir))
        .await
        .map_err(|e| e.to_string())?
}

/// Installs the verified yt-dlp; returns its version. The next search uses it, no restart needed.
#[tauri::command]
pub async fn update_install_ytdlp(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    guard_env()?;
    let dir = data_dir(&state)?;
    let _guard = InstallGuard::acquire()?;
    tauri::async_runtime::spawn_blocking(move || install_ytdlp_blocking(&app, &dir))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn a_plain_file_has_no_signature_according_to_windows() {
        use pyxee_shell::update::{Signature, SignatureChecker};
        // A real program that nobody has signed: the test binary itself. (A made-up file with an .exe name is not
        // "unsigned" to Windows but unreadable, which it reports as `UnknownError`.)
        let file = std::env::current_exe().unwrap();
        let got = WindowsSignatures.signature(&file);
        assert_eq!(got, Ok(Signature::Unsigned));
    }

    use super::*;

    #[test]
    fn clearing_the_update_folder_removes_what_is_in_it_and_is_fine_when_it_is_not_there() {
        let dir = std::env::temp_dir().join(format!("pyxee-clear-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Pyxee_0.1.0_x64-setup.exe"), b"MZ").unwrap();
        clear_dir(&dir);
        assert!(!dir.exists());
        clear_dir(&dir); // nothing there: no error, no panic
    }

    #[test]
    fn only_https_addresses_are_fetched() {
        assert!(require_https("https://github.com/x").is_ok());
        assert!(require_https("HTTPS://github.com/x").is_ok());
        assert!(require_https("http://github.com/x").is_err());
        assert!(require_https("file:///c:/x").is_err());
        assert!(require_https("https:/x").is_err());
        assert!(require_https("").is_err());
    }

    #[test]
    fn an_answer_over_the_limit_is_an_error_not_a_cut_off_file() {
        let data = [7u8; 11];
        assert_eq!(read_limited(&data[..], 11, "x").unwrap().len(), 11);
        assert!(read_limited(&data[..], 10, "x").is_err());
    }

    #[test]
    fn the_installer_is_hashed_again_before_it_is_started() {
        let name = format!("pyxee-recheck-{}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("setup.exe");
        std::fs::write(&file, b"MZ original").unwrap();
        let good = update::sha256_hex(b"MZ original");
        assert!(recheck_installer(&file, &good).is_ok());
        std::fs::write(&file, b"MZ swapped").unwrap();
        assert!(recheck_installer(&file, &good).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_updated_yt_dlp_lives_in_a_tools_folder_of_the_data_folder() {
        assert_eq!(
            managed_ytdlp_path(Path::new("D:/data")),
            Path::new("D:/data").join("tools").join("yt-dlp.exe")
        );
    }

    #[test]
    fn a_second_install_is_refused_until_the_first_is_done() {
        let first = InstallGuard::acquire().expect("nothing is installing");
        assert_eq!(
            InstallGuard::acquire().err().as_deref(),
            Some("An update is already being installed.")
        );
        drop(first);
        assert!(InstallGuard::acquire().is_ok());
    }

    #[test]
    fn app_update_info_crosses_the_boundary_in_camel_case() {
        let v = serde_json::to_value(AppUpdateDto {
            current: "0.1.0".into(),
            latest: "0.2.0".into(),
            available: true,
            notes_url: "u".into(),
            notes: "- a fix".into(),
            installable: false,
            portable: true,
        })
        .unwrap();
        assert_eq!(v["notesUrl"], "u");
        assert_eq!(v["notes"], "- a fix");
        assert_eq!(v["installable"], false);
        assert_eq!(v["portable"], true);
    }
}
