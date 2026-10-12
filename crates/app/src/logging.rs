//! Logging to a file the user can find: `<app data>\logs\pyxee.log` (one older copy, `pyxee.log.1`, is kept
//! when it passes 2 MB). Everything is also copied to the console, which is what `tauri dev` shows.
//!
//! Set `PYXEE_LOG` to `error`, `warn`, `info` (default), `debug` or `trace` for more or less detail.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LogInfo {
    pub dir: PathBuf,
    pub file: PathBuf,
}

pub struct FileLogger {
    path: PathBuf,
    file: Mutex<File>,
    level: LevelFilter,
    max_bytes: u64,
    to_stderr: bool,
}

/// `1700000000123` -> `2023-11-14 22:13:20.123Z` (UTC; no time zone database needed).
pub fn format_utc(ms: i64) -> String {
    let (secs, millis) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (days, tod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}.{millis:03}Z",
        tod / 3600,
        tod % 3600 / 60,
        tod % 60
    )
}

pub fn format_line(now_ms: i64, level: Level, target: &str, message: &str) -> String {
    // Keep one record on one line, so the file can be searched and pasted.
    format!(
        "{} {:<5} {} {}\n",
        format_utc(now_ms),
        level,
        target,
        message.replace('\n', "\n    ")
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl FileLogger {
    pub fn open(
        path: &Path,
        level: LevelFilter,
        max_bytes: u64,
        to_stderr: bool,
    ) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            level,
            max_bytes,
            to_stderr,
        })
    }

    fn rotate(&self, file: &mut File) {
        if file.metadata().map_or(0, |m| m.len()) <= self.max_bytes {
            return;
        }
        let old = self.path.with_extension("log.1");
        let _ = fs::remove_file(&old);
        if fs::rename(&self.path, &old).is_ok() {
            if let Ok(fresh) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                *file = fresh;
            }
        }
    }
}

impl Log for FileLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        // Our own crates follow the chosen level; libraries only speak up about problems.
        let ours = m.target().starts_with("pyxee") || m.target().starts_with("xtask");
        m.level()
            <= if ours {
                self.level
            } else {
                self.level.min(LevelFilter::Warn)
            }
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format_line(now_ms(), r.level(), r.target(), &r.args().to_string());
        if self.to_stderr {
            eprint!("{line}");
        }
        if let Ok(mut f) = self.file.lock() {
            self.rotate(&mut f);
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut f) = self.file.lock() {
            let _ = f.flush();
        }
    }
}

pub fn level_from(text: Option<&str>) -> LevelFilter {
    match text.map(|t| t.trim().to_ascii_lowercase()).as_deref() {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

/// Starts logging for the whole process. If the log folder cannot be used the app still runs (console only)
/// and the reason is returned.
pub fn init(dir: &Path) -> (LogInfo, Option<String>) {
    let file = dir.join("pyxee.log");
    let info = LogInfo {
        dir: dir.to_path_buf(),
        file: file.clone(),
    };
    let level = level_from(std::env::var("PYXEE_LOG").ok().as_deref());
    match FileLogger::open(&file, level, MAX_BYTES, true) {
        Ok(logger) => {
            if log::set_boxed_logger(Box::new(logger)).is_ok() {
                log::set_max_level(level);
            }
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |p| {
                log::error!("panic: {p}\n{}", std::backtrace::Backtrace::force_capture());
                default_hook(p);
            }));
            log::info!(
                "Pyxee {} starting on {} (log level {level}); log file: {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                file.display()
            );
            (info, None)
        }
        Err(e) => (
            info,
            Some(format!(
                "could not open the log file {}: {e}",
                file.display()
            )),
        ),
    }
}

/// Opens the folder in the system file manager.
pub fn open_folder(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let program = if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    // Explorer reports failure even when it opened the folder, so only a failure to start counts.
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map(drop)
        .map_err(|e| format!("could not open {} with {program}: {e}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pyxee-log-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn write(l: &FileLogger, level: Level, target: &str, msg: &str) {
        l.log(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{msg}"))
                .build(),
        );
    }

    #[test]
    fn timestamps_are_correct_utc() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00.000Z");
        assert_eq!(format_utc(1_700_000_000_123), "2023-11-14 22:13:20.123Z");
        assert_eq!(
            format_utc(951_782_400_000),
            "2000-02-29 00:00:00.000Z",
            "leap day"
        );
        assert_eq!(format_utc(1_782_864_000_999), "2026-07-01 00:00:00.999Z");
        assert_eq!(format_utc(-1), "1969-12-31 23:59:59.999Z");
    }

    #[test]
    fn a_line_has_time_level_source_and_message_and_stays_greppable() {
        let l = format_line(
            0,
            Level::Warn,
            "pyxee_app::engine",
            "could not start \"Song\"",
        );
        assert_eq!(
            l,
            "1970-01-01 00:00:00.000Z WARN  pyxee_app::engine could not start \"Song\"\n"
        );
        let multi = format_line(0, Level::Error, "t", "panic\nline two");
        assert!(multi.ends_with("panic\n    line two\n"), "{multi:?}");
    }

    #[test]
    fn our_crates_follow_the_level_and_libraries_only_report_problems() {
        let dir = temp("levels");
        let l = FileLogger::open(&dir.join("p.log"), LevelFilter::Info, MAX_BYTES, false).unwrap();
        write(&l, Level::Debug, "pyxee_app::engine", "too detailed");
        write(&l, Level::Info, "pyxee_app::engine", "visible info");
        write(&l, Level::Info, "hyper::client", "library chatter");
        write(&l, Level::Warn, "hyper::client", "library problem");
        let text = fs::read_to_string(dir.join("p.log")).unwrap();
        assert!(text.contains("visible info") && text.contains("library problem"));
        assert!(!text.contains("too detailed") && !text.contains("library chatter"));
    }

    #[test]
    fn the_file_is_rotated_at_the_limit_and_nothing_recent_is_lost() {
        let dir = temp("rotate");
        let path = dir.join("p.log");
        let l = FileLogger::open(&path, LevelFilter::Info, 300, false).unwrap();
        for i in 0..20 {
            write(
                &l,
                Level::Info,
                "pyxee_app",
                &format!("line number {i:02} with some padding text"),
            );
        }
        let old = fs::read_to_string(dir.join("p.log.1")).expect("an older copy is kept");
        let new = fs::read_to_string(&path).unwrap();
        assert!(!old.is_empty() && !new.is_empty());
        assert!(
            new.contains("line number 19"),
            "the newest line is in the current file"
        );
        assert!(
            fs::metadata(&path).unwrap().len() < 600,
            "the current file stays small"
        );
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            2,
            "only one old copy is kept"
        );
    }

    #[test]
    fn logs_from_earlier_runs_are_kept_when_the_app_restarts() {
        let dir = temp("restart");
        let path = dir.join("p.log");
        write(
            &FileLogger::open(&path, LevelFilter::Info, MAX_BYTES, false).unwrap(),
            Level::Info,
            "pyxee",
            "first run",
        );
        write(
            &FileLogger::open(&path, LevelFilter::Info, MAX_BYTES, false).unwrap(),
            Level::Info,
            "pyxee",
            "second run",
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.find("first run").unwrap() < text.find("second run").unwrap());
    }

    #[test]
    fn the_level_comes_from_the_environment_value() {
        assert_eq!(level_from(None), LevelFilter::Info);
        assert_eq!(level_from(Some(" DEBUG ")), LevelFilter::Debug);
        assert_eq!(level_from(Some("warn")), LevelFilter::Warn);
        assert_eq!(level_from(Some("nonsense")), LevelFilter::Info);
    }

    #[test]
    fn an_unusable_log_folder_is_reported_not_fatal() {
        let dir = temp("blocked");
        fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("not-a-folder");
        fs::write(&blocker, b"x").unwrap();
        assert!(
            FileLogger::open(&blocker.join("p.log"), LevelFilter::Info, MAX_BYTES, false).is_err()
        );
    }
}
