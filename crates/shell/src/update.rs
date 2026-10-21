//! Decisions for the in-app updaters: is a release newer, which release file is the installer, does a download match
//! its published SHA-256, and how a verified program replaces the old one. No network and no Tauri here; the app
//! downloads, this module judges and writes, so all of it is tested on any OS.
//!
//! Nothing is installed until it has been checked: the checksum must match, the file must be a Windows program, and
//! (for yt-dlp) the new program must run and answer `--version`. A failure at any step leaves the working program
//! exactly as it was.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("{0} is not listed in the published checksums, so it was not installed.")]
    NotListed(String),
    #[error("The download does not match its published checksum (expected {expected}, got {got}). It is corrupt or was tampered with; nothing was installed.")]
    ChecksumMismatch { expected: String, got: String },
    #[error("The download is not a Windows program; nothing was installed.")]
    NotAnExecutable,
    #[error("The new program did not start ({0}); the old one is still in place.")]
    ProbeFailed(String),
    #[error("Could not write the update: {0}")]
    Io(String),
    #[error("The release information could not be read: {0}")]
    BadRelease(String),
}

// ------------------------------------------------------------------ versions

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// `rc1`, `beta.2`... Empty for a stable release.
    pub pre: Vec<String>,
}

impl Version {
    /// Reads `1.2.3`, `v1.2.3`, `1.2.3-rc1`. Anything else (including build metadata after `+`, which is ignored) is `None`.
    pub fn parse(text: &str) -> Option<Version> {
        let t = text.trim().trim_start_matches(['v', 'V']);
        let t = t.split('+').next()?;
        let (nums, pre) = match t.split_once('-') {
            Some((n, p)) => (n, Some(p)),
            None => (t, None),
        };
        let mut it = nums.split('.');
        let major = it.next()?.parse().ok()?;
        let minor = it.next()?.parse().ok()?;
        let patch = it.next()?.parse().ok()?;
        if it.next().is_some() {
            return None;
        }
        let pre: Vec<String> = pre
            .map(|p| p.split('.').map(str::to_string).collect())
            .unwrap_or_default();
        if pre.iter().any(String::is_empty) {
            return None;
        }
        Some(Version {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(o.major, o.minor, o.patch))
            .then_with(|| match (self.pre.is_empty(), o.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater, // 1.0.0 is newer than 1.0.0-rc1
                (false, true) => Ordering::Less,
                (false, false) => cmp_pre(&self.pre, &o.pre),
            })
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Semver's rule for pre-release parts: numbers compare as numbers and sort below words; a longer list wins a tie.
fn cmp_pre(a: &[String], b: &[String]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(n), Ok(m)) => n.cmp(&m),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            // `rc2` before `rc10`: split a trailing number off so it compares as a number (plain semver would not).
            (Err(_), Err(_)) => {
                let (px, nx) = split_number(x);
                let (py, ny) = split_number(y);
                px.cmp(py).then(nx.cmp(&ny)).then_with(|| x.cmp(y))
            }
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    a.len().cmp(&b.len())
}

/// `"rc10"` -> (`"rc"`, 10); an identifier without trailing digits has the number 0.
fn split_number(id: &str) -> (&str, u64) {
    let cut = id.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    (&id[..cut], id[cut..].parse().unwrap_or(0))
}

/// True only when both parse and `latest` is strictly newer: an unreadable version never offers an update.
pub fn is_newer(current: &str, latest: &str) -> bool {
    matches!((Version::parse(current), Version::parse(latest)), (Some(c), Some(l)) if l > c)
}

// ------------------------------------------------------------------ releases

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub page_url: String,
    /// The release description (Markdown), shown as the changelog. Empty when the release has none.
    pub notes: String,
    pub installer: Option<Asset>,
    pub sums_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    html_url: String,
    // GitHub sends `null` for a release without a description.
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
}
#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
}

/// The installer to offer: the NSIS `-setup.exe` first, then any other `.exe`, then an `.msi`.
fn pick_installer(assets: &[RawAsset]) -> Option<Asset> {
    let find = |f: &dyn Fn(&str) -> bool| {
        assets
            .iter()
            .find(|a| f(&a.name.to_ascii_lowercase()))
            .map(|a| Asset {
                name: a.name.clone(),
                url: a.browser_download_url.clone(),
            })
    };
    find(&|n| n.ends_with("-setup.exe"))
        .or_else(|| find(&|n| n.ends_with(".exe")))
        .or_else(|| find(&|n| n.ends_with(".msi")))
}

/// Picks the newest release from GitHub's `/releases` list. Drafts never count; pre-releases count only when
/// `include_prereleases` (the running build is itself one), so a stable user is not moved onto a release candidate.
pub fn newest_release(
    json: &str,
    include_prereleases: bool,
) -> Result<Option<Release>, UpdateError> {
    let raw: Vec<RawRelease> =
        serde_json::from_str(json).map_err(|e| UpdateError::BadRelease(e.to_string()))?;
    let mut best: Option<Release> = None;
    for r in raw {
        if r.draft || (r.prerelease && !include_prereleases) {
            continue;
        }
        let Some(version) = Version::parse(&r.tag_name) else {
            continue;
        };
        if version.is_prerelease() && !include_prereleases {
            continue;
        }
        if best.as_ref().is_some_and(|b| b.version >= version) {
            continue;
        }
        let sums_url = r
            .assets
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
            .map(|a| a.browser_download_url.clone());
        best = Some(Release {
            installer: pick_installer(&r.assets),
            version,
            tag: r.tag_name,
            page_url: r.html_url,
            notes: clip_notes(r.body.unwrap_or_default()),
            sums_url,
        });
    }
    Ok(best)
}

/// A changelog longer than this is cut (at a character boundary): it only has to be readable in a panel, and the
/// full text is one click away on the release page.
const MAX_NOTES_CHARS: usize = 20_000;

fn clip_notes(body: String) -> String {
    let body = body.replace("\r\n", "\n");
    let body = body.trim();
    match body.char_indices().nth(MAX_NOTES_CHARS) {
        Some((cut, _)) => format!("{}…", body[..cut].trim_end()),
        None => body.to_string(),
    }
}

impl Release {
    /// The version as people write it: `0.2.0` or `0.2.0-rc1`, without the tag's `v`.
    pub fn version_text(&self) -> String {
        self.tag.trim_start_matches(['v', 'V']).to_string()
    }
}

// ----------------------------------------------------------------- checksums

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `name`'s hash in a checksum file (`<hash>  <name>` or `<hash> *<name>`), lower-cased.
pub fn parse_sums(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        let (hash, file) = (parts.next()?, parts.next()?);
        (file.trim_start_matches('*') == name && hash.len() == 64)
            .then(|| hash.to_ascii_lowercase())
    })
}

pub fn verify(bytes: &[u8], expected: &str) -> Result<(), UpdateError> {
    let got = sha256_hex(bytes);
    if got.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(UpdateError::ChecksumMismatch {
            expected: expected.trim().to_ascii_lowercase(),
            got,
        })
    }
}

/// Windows programs start with "MZ".
pub fn is_windows_exe(bytes: &[u8]) -> bool {
    bytes.starts_with(b"MZ")
}

/// What yt-dlp's published checksum says about the program in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStatus {
    UpToDate,
    /// The published program differs from the one in use; this is its expected SHA-256.
    Available {
        expected: String,
    },
}

/// Compares the SHA-256 of the program in use with the one `sums` publishes for `file`. Comparing hashes instead of
/// version strings means no date format to parse and no way to be fooled by a renamed binary.
pub fn tool_status(current_sha: &str, sums: &str, file: &str) -> Result<ToolStatus, UpdateError> {
    let expected =
        parse_sums(sums, file).ok_or_else(|| UpdateError::NotListed(file.to_string()))?;
    Ok(if expected.eq_ignore_ascii_case(current_sha) {
        ToolStatus::UpToDate
    } else {
        ToolStatus::Available { expected }
    })
}

// ------------------------------------------------------------------- install

fn io<E: std::fmt::Display>(e: E) -> UpdateError {
    UpdateError::Io(e.to_string())
}

/// Renames `from` over `to`. Windows refuses while the old file is running or being scanned, which usually
/// ends within a second, so a refusal is tried again a few times before it counts as a failure.
fn rename_when_free(from: &Path, to: &Path) -> std::io::Result<()> {
    const RETRIES: u32 = 5;
    let mut tries = 0;
    loop {
        match fs::rename(from, to) {
            Err(e) if tries < RETRIES && is_in_use(&e) => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            other => return other,
        }
    }
}

/// Access denied or a sharing violation (Windows error 32): the file is in use.
fn is_in_use(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32)
}

/// Puts `bytes` at `target` only if they are the program `expected` describes and the new program runs.
/// `probe` starts the written file (given its temporary path) and returns what it printed, e.g. its `--version`.
/// The file is written beside `target` and renamed over it, so an interrupted update never leaves a half-written
/// program, and every failure removes the temporary file and leaves `target` untouched. Returns the probe's output.
pub fn install_verified(
    target: &Path,
    bytes: &[u8],
    expected: &str,
    probe: impl FnOnce(&Path) -> Result<String, String>,
) -> Result<String, UpdateError> {
    verify(bytes, expected)?;
    if !is_windows_exe(bytes) {
        return Err(UpdateError::NotAnExecutable);
    }
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp: PathBuf = target.with_extension("exe.part");
    let result = (|| {
        fs::write(&tmp, bytes).map_err(io)?;
        let out = probe(&tmp).map_err(UpdateError::ProbeFailed)?;
        let out = out.trim().to_string();
        if out.is_empty() {
            return Err(UpdateError::ProbeFailed("it printed nothing".into()));
        }
        rename_when_free(&tmp, target).map_err(|e| {
            UpdateError::Io(format!(
                "could not replace {} ({e}); close anything that is using it and try again",
                target.display()
            ))
        })?;
        Ok(out)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn versions_parse_with_or_without_a_v_and_reject_nonsense() {
        assert_eq!(v("v1.2.3"), v("1.2.3"));
        assert_eq!(v("0.2.0-rc1").pre, vec!["rc1"]);
        assert_eq!(v("1.0.0+build5"), v("1.0.0"));
        for bad in ["", "1.2", "1.2.3.4", "a.b.c", "1.2.3-", "v"] {
            assert!(Version::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn newer_means_strictly_newer_and_a_release_beats_its_own_candidates() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.9.0", "0.10.0"), "numbers compare as numbers");
        assert!(is_newer("0.2.0-rc1", "0.2.0"));
        assert!(is_newer("0.2.0-rc1", "0.2.0-rc2"));
        assert!(is_newer("0.2.0-rc2", "0.2.0-rc10"));
        assert!(is_newer("0.2.0-beta", "0.2.0-rc1"));
        assert!(!is_newer("0.2.0", "0.2.0-rc3"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.3.0", "0.2.9"));
        assert!(
            !is_newer("0.1.0", "garbage"),
            "an unreadable version never offers an update"
        );
    }

    const RELEASES: &str = r#"[
      {"tag_name":"v0.3.0-rc1","html_url":"https://x/rc1","draft":false,"prerelease":true,"assets":[
        {"name":"Pyxee_0.3.0-rc1_x64-setup.exe","browser_download_url":"https://x/rc1.exe"},
        {"name":"SHA256SUMS.txt","browser_download_url":"https://x/rc1.sums"}]},
      {"tag_name":"v0.2.0","html_url":"https://x/v020","draft":false,"prerelease":false,"assets":[
        {"name":"Pyxee_0.2.0_x64_en-US.msi","browser_download_url":"https://x/020.msi"},
        {"name":"Pyxee_0.2.0_x64-setup.exe","browser_download_url":"https://x/020.exe"},
        {"name":"SHA256SUMS.txt","browser_download_url":"https://x/020.sums"}]},
      {"tag_name":"v0.9.9","html_url":"https://x/draft","draft":true,"prerelease":false,"assets":[]},
      {"tag_name":"nightly","html_url":"https://x/n","draft":false,"prerelease":false,"assets":[]},
      {"tag_name":"v0.1.0","html_url":"https://x/v010","draft":false,"prerelease":false,"assets":[]}
    ]"#;

    #[test]
    fn a_stable_build_is_offered_the_newest_stable_release_not_a_candidate_or_a_draft() {
        let r = newest_release(RELEASES, false).unwrap().unwrap();
        assert_eq!(r.tag, "v0.2.0");
        assert_eq!(
            r.installer.unwrap().name,
            "Pyxee_0.2.0_x64-setup.exe",
            "the NSIS installer beats the MSI"
        );
        assert_eq!(r.sums_url.as_deref(), Some("https://x/020.sums"));
    }

    #[test]
    fn a_prerelease_build_also_sees_prereleases() {
        let r = newest_release(RELEASES, true).unwrap().unwrap();
        assert_eq!(r.tag, "v0.3.0-rc1");
    }

    #[test]
    fn no_usable_release_is_none_and_garbage_is_a_typed_error() {
        assert_eq!(newest_release("[]", false).unwrap(), None);
        assert!(matches!(
            newest_release("{not json", false),
            Err(UpdateError::BadRelease(_))
        ));
    }

    #[test]
    fn a_release_without_an_installer_still_reports_its_page() {
        let r = newest_release(
            r#"[{"tag_name":"v1.0.0","html_url":"https://x/p","assets":[]}]"#,
            false,
        )
        .unwrap()
        .unwrap();
        assert!(r.installer.is_none() && r.sums_url.is_none());
        assert_eq!(r.page_url, "https://x/p");
    }

    #[test]
    fn the_release_description_becomes_the_changelog() {
        let r = newest_release(
            r###"[{"tag_name":"v1.0.0","html_url":"https://x/p","body":"## New\r\n- one\r\n- two\r\n\r\n","assets":[]}]"###,
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.notes, "## New\n- one\n- two");
    }

    #[test]
    fn a_release_without_a_description_has_empty_notes() {
        for body in [r#""body":null,"#, ""] {
            let json = format!(
                r#"[{{"tag_name":"v1.0.0","html_url":"https://x/p",{body}"assets":[]}}]"#
            );
            let r = newest_release(&json, false).unwrap().unwrap();
            assert_eq!(r.notes, "");
        }
    }

    #[test]
    fn a_very_long_changelog_is_cut_on_a_character_boundary() {
        let long = "é".repeat(MAX_NOTES_CHARS + 50);
        let clipped = clip_notes(long);
        assert_eq!(clipped.chars().count(), MAX_NOTES_CHARS + 1);
        assert!(clipped.ends_with('…'));
    }

    const SUMS: &str =
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA *yt-dlp.exe\n\
                        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  yt-dlp\n";

    #[test]
    fn checksums_are_found_by_exact_name_in_both_formats() {
        assert_eq!(parse_sums(SUMS, "yt-dlp.exe").unwrap(), "a".repeat(64));
        assert_eq!(parse_sums(SUMS, "yt-dlp").unwrap(), "b".repeat(64));
        assert_eq!(parse_sums(SUMS, "nope"), None);
        assert_eq!(
            parse_sums("tooshort  yt-dlp.exe", "yt-dlp.exe"),
            None,
            "not a SHA-256"
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
    fn a_tool_is_up_to_date_when_its_hash_is_the_published_one() {
        let current = "A".repeat(64);
        assert_eq!(
            tool_status(&current, SUMS, "yt-dlp.exe").unwrap(),
            ToolStatus::UpToDate
        );
        assert_eq!(
            tool_status(&"c".repeat(64), SUMS, "yt-dlp.exe").unwrap(),
            ToolStatus::Available {
                expected: "a".repeat(64)
            }
        );
        assert_eq!(
            tool_status("x", SUMS, "other.exe"),
            Err(UpdateError::NotListed("other.exe".into()))
        );
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pyxee-update-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_verified_program_replaces_the_old_one_and_reports_what_it_printed() {
        let dir = temp("ok");
        let target = dir.join("tools").join("yt-dlp.exe");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"MZ old").unwrap();
        let new = b"MZ new program";
        let out = install_verified(&target, new, &sha256_hex(new), |p| {
            assert_eq!(
                fs::read(p).unwrap(),
                new,
                "the probe runs the new file, not the old one"
            );
            Ok("2026.10.01\n".into())
        })
        .unwrap();
        assert_eq!(out, "2026.10.01");
        assert_eq!(fs::read(&target).unwrap(), new);
        assert!(!target.with_extension("exe.part").exists());
    }

    #[test]
    fn a_wrong_checksum_installs_nothing_and_never_runs_the_download() {
        let dir = temp("sum");
        let target = dir.join("yt-dlp.exe");
        fs::write(&target, b"MZ old").unwrap();
        let err = install_verified(&target, b"MZ new", &"0".repeat(64), |_| {
            panic!("must not run")
        })
        .unwrap_err();
        assert!(matches!(err, UpdateError::ChecksumMismatch { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"MZ old");
        assert!(!target.with_extension("exe.part").exists());
    }

    #[test]
    fn something_that_is_not_a_windows_program_is_refused() {
        let dir = temp("mz");
        let target = dir.join("yt-dlp.exe");
        let html = b"<html>rate limited</html>";
        let err = install_verified(&target, html, &sha256_hex(html), |_| panic!("must not run"))
            .unwrap_err();
        assert_eq!(err, UpdateError::NotAnExecutable);
        assert!(!target.exists());
    }

    #[test]
    fn a_program_that_will_not_run_leaves_the_old_one_in_place() {
        let dir = temp("probe");
        let target = dir.join("yt-dlp.exe");
        fs::write(&target, b"MZ old").unwrap();
        let new = b"MZ broken";
        let err = install_verified(
            &target,
            new,
            &sha256_hex(new),
            |_| Err("exit code 1".into()),
        )
        .unwrap_err();
        assert_eq!(err, UpdateError::ProbeFailed("exit code 1".into()));
        assert_eq!(fs::read(&target).unwrap(), b"MZ old");
        assert!(
            !target.with_extension("exe.part").exists(),
            "no partial file is left behind"
        );

        let err =
            install_verified(&target, new, &sha256_hex(new), |_| Ok("  \n".into())).unwrap_err();
        assert!(
            matches!(err, UpdateError::ProbeFailed(_)),
            "silence is a failure too"
        );
        assert_eq!(fs::read(&target).unwrap(), b"MZ old");
    }
}
