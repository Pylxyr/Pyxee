//! yt-dlp sidecar: search, resolve and related, all through one process wrapper that has a timeout
//! and turns failures into errors the UI can explain.

use crate::suggest::{self, encode};
use crate::{
    classify_stderr, MusicSource, SearchFilter, SearchItem, SourceError, StreamInfo, TrackInfo,
};
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Prefer Opus (best quality YouTube serves without Premium), then AAC, then anything.
pub const FORMAT_SELECTOR: &str = "bestaudio[acodec=opus]/bestaudio[ext=m4a]/bestaudio";
const RESULTS: usize = 20;

pub struct YtDlpSource {
    path: PathBuf,
    timeout: Duration,
}

impl YtDlpSource {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            timeout: Duration::from_secs(40),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn run(&self, args: &[String]) -> Result<String, SourceError> {
        run_tool(&self.path, args, self.timeout)
    }
}

pub(crate) fn run_tool(
    path: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<String, SourceError> {
    let name = path.display().to_string();
    let mut cmd = Command::new(path);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| {
        log::error!("could not start {name}: {e}");
        if e.kind() == std::io::ErrorKind::NotFound {
            SourceError::ToolMissing(name.clone())
        } else {
            SourceError::Failed(format!("could not start {name}: {e}"))
        }
    })?;
    // Drain both pipes on threads so a large result can never block the process.
    let read = |mut r: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = read(Box::new(child.stdout.take().expect("piped")));
    let err = read(Box::new(child.stderr.take().expect("piped")));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                log::warn!("{name} was stopped after {timeout:?}: {args:?}");
                return Err(SourceError::Failed(format!(
                    "{name} took too long and was stopped"
                )));
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(e) => return Err(SourceError::Failed(format!("{name}: {e}"))),
        }
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    // A playlist with a few unavailable entries still exits non-zero but prints the rest.
    if !status.success() {
        let tail: String = stderr
            .chars()
            .rev()
            .take(1500)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        log::warn!(
            "{name} exited with {status} (stdout {} bytes). stderr: {}",
            stdout.len(),
            tail.trim()
        );
    }
    if !status.success() && stdout.trim().is_empty() {
        return Err(classify_stderr(&name, &stderr));
    }
    Ok(stdout)
}

/// What to hand yt-dlp for a search, or why this filter cannot be served.
pub fn search_target(query: &str, filter: SearchFilter) -> Result<String, SourceError> {
    let q = query.trim();
    Ok(match filter {
        SearchFilter::All | SearchFilter::Videos => format!("ytsearch{RESULTS}:{q}"),
        SearchFilter::Songs => format!("https://music.youtube.com/search?q={}", encode(q)),
        SearchFilter::Playlists => {
            format!(
                "https://www.youtube.com/results?search_query={}&sp=EgIQAw%253D%253D",
                encode(q)
            )
        }
        SearchFilter::Artists => {
            format!(
                "https://www.youtube.com/results?search_query={}&sp=EgIQAg%253D%253D",
                encode(q)
            )
        }
        SearchFilter::Albums => {
            return Err(SourceError::Unsupported("Album search".into()));
        }
    })
}

fn flat_args(target: String, count: usize) -> Vec<String> {
    ["--flat-playlist", "-j", "--no-warnings", "--playlist-end"]
        .iter()
        .map(|s| (*s).to_string())
        .chain([count.to_string(), target])
        .collect()
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn thumbnail(v: &Value, id: Option<&str>) -> Option<String> {
    let list = v.get("thumbnails").and_then(Value::as_array);
    let best = list.and_then(|l| {
        l.iter()
            .filter_map(|t| {
                Some((
                    t.get("url")?.as_str()?,
                    t.get("width").and_then(Value::as_u64).unwrap_or(0),
                ))
            })
            .max_by_key(|(_, w)| (*w).min(640))
            .map(|(u, _)| u.to_string())
    });
    best.or_else(|| id.map(|i| format!("https://i.ytimg.com/vi/{i}/hqdefault.jpg")))
}

fn artist_name(v: &Value) -> String {
    let raw = text(v, "uploader")
        .or_else(|| text(v, "channel"))
        .unwrap_or_else(|| "Unknown artist".into());
    raw.strip_suffix(" - Topic").unwrap_or(&raw).to_string()
}

/// One line of `--flat-playlist -j` output -> a search item.
pub fn item_from_entry(v: &Value) -> Option<SearchItem> {
    let url = text(v, "url")
        .or_else(|| text(v, "webpage_url"))
        .unwrap_or_default();
    let id = text(v, "id");
    let title = text(v, "title")?;
    if url.contains("/channel/")
        || url.contains("/@")
        || url.contains("/user/")
        || url.contains("/c/")
    {
        return Some(SearchItem::Artist {
            key: format!("yt-channel:{}", id?),
            name: title,
            thumbnail: thumbnail(v, None),
        });
    }
    if url.contains("playlist?list=") {
        return Some(SearchItem::Playlist {
            key: format!("yt-playlist:{}", id?),
            title,
            uploader: artist_name(v),
            thumbnail: thumbnail(v, None),
        });
    }
    let id = id?;
    Some(SearchItem::Track(TrackInfo {
        key: format!("yt:{id}"),
        title,
        artist: artist_name(v),
        duration_ms: v
            .get("duration")
            .and_then(Value::as_f64)
            .map(|s| (s * 1000.0) as u64),
        thumbnail: thumbnail(v, Some(&id)),
    }))
}

pub fn parse_entries(stdout: &str) -> Result<Vec<SearchItem>, SourceError> {
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .collect();
    let items: Vec<SearchItem> = lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| item_from_entry(&v))
        .collect();
    if items.is_empty() && !lines.is_empty() {
        return Err(SourceError::Parse(
            "results had an unexpected format".into(),
        ));
    }
    Ok(items)
}

#[derive(Deserialize)]
struct Dump {
    id: String,
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    duration: Option<f64>,
    url: Option<String>,
    ext: Option<String>,
    acodec: Option<String>,
    abr: Option<f64>,
}

/// Parses `yt-dlp -j -f ...` output for a single video.
pub fn parse_dump(json: &str) -> Result<Resolved, SourceError> {
    let d: Dump = serde_json::from_str(json).map_err(|e| SourceError::Parse(e.to_string()))?;
    let url = d
        .url
        .ok_or_else(|| SourceError::Parse("no stream url in output".into()))?;
    Ok(Resolved {
        id: d.id,
        title: d.title.unwrap_or_else(|| "Unknown title".into()),
        uploader: d.uploader.or(d.channel),
        duration_secs: d.duration,
        stream_url: url,
        ext: d.ext.unwrap_or_default(),
        acodec: d.acodec,
        abr_kbps: d.abr,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration_secs: Option<f64>,
    pub stream_url: String,
    pub ext: String,
    pub acodec: Option<String>,
    pub abr_kbps: Option<f64>,
}

/// Searches for `query` and resolves the top hit (used by the spike harness).
pub fn resolve_query(ytdlp: &Path, query: &str) -> Result<Resolved, SourceError> {
    let args: Vec<String> = [
        "--no-playlist",
        "--no-warnings",
        "-f",
        FORMAT_SELECTOR,
        "-j",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .chain([format!("ytsearch1:{query}")])
    .collect();
    let out = run_tool(ytdlp, &args, Duration::from_secs(40))?;
    let line = out
        .lines()
        .find(|l| l.starts_with('{'))
        .ok_or_else(|| SourceError::Unavailable(format!("no results for \"{query}\"")))?;
    parse_dump(line)
}

fn video_id(key: &str) -> Result<&str, SourceError> {
    key.strip_prefix("yt:")
        .filter(|i| !i.is_empty())
        .ok_or_else(|| SourceError::Failed(format!("not a YouTube key: {key}")))
}

impl MusicSource for YtDlpSource {
    fn search(&self, query: &str, filter: SearchFilter) -> Result<Vec<SearchItem>, SourceError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let out = self.run(&flat_args(search_target(query, filter)?, RESULTS))?;
        parse_entries(&out)
    }

    fn resolve_stream(&self, key: &str) -> Result<StreamInfo, SourceError> {
        let id = video_id(key)?;
        let args: Vec<String> = [
            "--no-playlist",
            "--no-warnings",
            "-f",
            FORMAT_SELECTOR,
            "-j",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .chain([format!("https://www.youtube.com/watch?v={id}")])
        .collect();
        let out = self.run(&args)?;
        let line = out
            .lines()
            .find(|l| l.starts_with('{'))
            .ok_or_else(|| SourceError::Parse("empty answer".into()))?;
        let r = parse_dump(line)?;
        Ok(StreamInfo {
            url: r.stream_url,
            ext: r.ext,
            acodec: r.acodec,
            abr_kbps: r.abr_kbps,
        })
    }

    fn related(&self, key: &str, count: usize) -> Result<Vec<TrackInfo>, SourceError> {
        let id = video_id(key)?;
        let target = format!("https://www.youtube.com/watch?v={id}&list=RD{id}");
        let out = self.run(&flat_args(target, count + 1))?;
        Ok(parse_entries(&out)?
            .into_iter()
            .filter_map(|i| {
                if let SearchItem::Track(t) = i {
                    Some(t)
                } else {
                    None
                }
            })
            .filter(|t| t.key != key)
            .take(count)
            .collect())
    }

    fn suggest(&self, prefix: &str) -> Result<Vec<String>, SourceError> {
        suggest::fetch(prefix)
    }

    fn supported_filters(&self) -> Vec<SearchFilter> {
        use SearchFilter::{All, Artists, Playlists, Songs, Videos};
        vec![All, Songs, Videos, Artists, Playlists]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: &str = r#"{"_type":"url","ie_key":"Youtube","id":"abc12345678","url":"https://www.youtube.com/watch?v=abc12345678","title":"Neon Harbor","duration":213.5,"channel":"Lowtide - Topic","uploader":"Lowtide - Topic","thumbnails":[{"url":"https://i.ytimg.com/vi/abc12345678/small.jpg","width":120},{"url":"https://i.ytimg.com/vi/abc12345678/big.jpg","width":720}]}"#;
    const PLAYLIST: &str = r#"{"_type":"url","ie_key":"YoutubeTab","id":"PL123","url":"https://www.youtube.com/playlist?list=PL123","title":"Late Night Mix","channel":"Some User"}"#;
    const CHANNEL: &str = r#"{"_type":"url","ie_key":"YoutubeTab","id":"UC999","url":"https://www.youtube.com/channel/UC999","title":"Lowtide","thumbnails":[{"url":"https://yt3.example/a.jpg","width":88}]}"#;

    #[test]
    fn a_track_entry_becomes_a_track_with_clean_artist_and_best_thumbnail() {
        let items = parse_entries(TRACK).unwrap();
        let SearchItem::Track(t) = &items[0] else {
            panic!("{items:?}")
        };
        assert_eq!(t.key, "yt:abc12345678");
        assert_eq!(t.title, "Neon Harbor");
        assert_eq!(t.artist, "Lowtide", "the ' - Topic' suffix is dropped");
        assert_eq!(t.duration_ms, Some(213_500));
        assert!(t.thumbnail.as_deref().unwrap().ends_with("big.jpg"));
    }

    #[test]
    fn missing_thumbnails_fall_back_to_the_standard_video_image() {
        let v: Value = serde_json::from_str(
            r#"{"id":"zzz","url":"https://www.youtube.com/watch?v=zzz","title":"T"}"#,
        )
        .unwrap();
        let Some(SearchItem::Track(t)) = item_from_entry(&v) else {
            panic!()
        };
        assert_eq!(
            t.thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/zzz/hqdefault.jpg")
        );
        assert_eq!(t.artist, "Unknown artist");
        assert_eq!(t.duration_ms, None);
    }

    #[test]
    fn playlists_and_channels_are_told_apart_from_videos() {
        let items = parse_entries(&format!("{TRACK}\n{PLAYLIST}\n{CHANNEL}\n")).unwrap();
        assert!(matches!(&items[0], SearchItem::Track(_)));
        assert!(
            matches!(&items[1], SearchItem::Playlist { key, uploader, .. } if key == "yt-playlist:PL123" && uploader == "Some User")
        );
        assert!(
            matches!(&items[2], SearchItem::Artist { key, name, .. } if key == "yt-channel:UC999" && name == "Lowtide")
        );
    }

    #[test]
    fn noise_lines_are_ignored_but_a_wholly_unreadable_result_is_an_error() {
        assert_eq!(
            parse_entries(&format!("WARNING: x\n{TRACK}\n"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(parse_entries("").unwrap(), vec![]);
        assert!(matches!(
            parse_entries(r#"{"nothing":"useful"}"#),
            Err(SourceError::Parse(_))
        ));
    }

    #[test]
    fn each_filter_maps_to_the_right_search() {
        assert_eq!(
            search_target(" lowtide ", SearchFilter::All).unwrap(),
            "ytsearch20:lowtide"
        );
        assert_eq!(
            search_target("lowtide", SearchFilter::Videos).unwrap(),
            "ytsearch20:lowtide"
        );
        assert_eq!(
            search_target("low tide", SearchFilter::Songs).unwrap(),
            "https://music.youtube.com/search?q=low%20tide"
        );
        assert!(search_target("x", SearchFilter::Playlists)
            .unwrap()
            .ends_with("sp=EgIQAw%253D%253D"));
        assert!(search_target("x", SearchFilter::Artists)
            .unwrap()
            .ends_with("sp=EgIQAg%253D%253D"));
        assert!(matches!(
            search_target("x", SearchFilter::Albums),
            Err(SourceError::Unsupported(_))
        ));
    }

    #[test]
    fn stream_dumps_parse_and_report_what_is_missing() {
        let r = parse_dump(r#"{"id":"a","title":"S","channel":"C","duration":10.0,"url":"https://x/y","ext":"webm","acodec":"opus","abr":139.5}"#).unwrap();
        assert_eq!(
            (r.ext.as_str(), r.acodec.as_deref(), r.uploader.as_deref()),
            ("webm", Some("opus"), Some("C"))
        );
        assert!(parse_dump(r#"{"id":"a"}"#)
            .unwrap_err()
            .to_string()
            .contains("no stream url"));
        assert!(matches!(parse_dump("not json"), Err(SourceError::Parse(_))));
    }

    #[test]
    fn errors_are_classified_for_the_ui() {
        let c = |s: &str| classify_stderr("yt-dlp", s);
        assert_eq!(
            c("ERROR: unable to download: HTTP Error 429: Too Many Requests"),
            SourceError::RateLimited
        );
        assert_eq!(
            c("ERROR: Sign in to confirm you're not a bot"),
            SourceError::RateLimited
        );
        assert_eq!(c("ERROR: [youtube] x: Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>"), SourceError::Offline);
        assert!(matches!(
            c("ERROR: [youtube] x: Video unavailable. This video has been removed"),
            SourceError::Unavailable(_)
        ));
        assert!(
            matches!(c("ERROR: something odd"), SourceError::Failed(m) if m.contains("something odd"))
        );
    }

    #[test]
    fn empty_queries_search_nothing_and_non_youtube_keys_are_refused() {
        let s = YtDlpSource::new("definitely-not-installed");
        assert_eq!(s.search("   ", SearchFilter::All).unwrap(), vec![]);
        assert!(s.resolve_stream("file:/x.mp3").is_err());
        assert!(matches!(
            s.search("x", SearchFilter::All),
            Err(SourceError::ToolMissing(_))
        ));
    }

    #[cfg(unix)]
    mod fake_tool {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        /// A stand-in yt-dlp: logs its arguments and prints canned output by request type.
        fn fake(name: &str, body: &str) -> (PathBuf, PathBuf) {
            let dir =
                std::env::temp_dir().join(format!("pyxee-fake-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let log = dir.join("args.log");
            let script = dir.join("yt-dlp");
            std::fs::write(
                &script,
                format!("#!/bin/sh\necho \"$@\" >> '{}'\n{body}\n", log.display()),
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            (script, log)
        }

        fn canned() -> String {
            let dump = r#"{"id":"abc12345678","title":"Neon Harbor","uploader":"Lowtide","duration":200.0,"url":"https://stream.example/a","ext":"webm","acodec":"opus","abr":139.0}"#;
            format!(
                r#"case "$*" in
  *list=RD*) printf '%s\n' '{TRACK}' '{second}' ;;
  *--flat-playlist*) printf '%s\n' '{TRACK}' '{PLAYLIST}' ;;
  *watch?v=*) printf '%s\n' '{dump}' ;;
esac"#,
                second = TRACK
                    .replace("abc12345678", "def12345678")
                    .replace("Neon Harbor", "Paper Planes"),
            )
        }

        #[test]
        fn search_resolve_and_related_drive_the_tool_with_the_right_arguments() {
            let (script, log) = fake("ok", &canned());
            let s = YtDlpSource::new(&script);

            let found = s.search("lowtide", SearchFilter::All).unwrap();
            assert_eq!(found.len(), 2);
            let stream = s.resolve_stream("yt:abc12345678").unwrap();
            assert_eq!(
                (stream.url.as_str(), stream.ext.as_str()),
                ("https://stream.example/a", "webm")
            );
            let related = s.related("yt:abc12345678", 5).unwrap();
            assert_eq!(related.len(), 1, "the seed itself is excluded");
            assert_eq!(related[0].title, "Paper Planes");

            let args = std::fs::read_to_string(log).unwrap();
            assert!(
                args.contains(
                    "--flat-playlist -j --no-warnings --playlist-end 20 ytsearch20:lowtide"
                ),
                "{args}"
            );
            assert!(args.contains("-f bestaudio[acodec=opus]/bestaudio[ext=m4a]/bestaudio -j https://www.youtube.com/watch?v=abc12345678"), "{args}");
            assert!(args.contains("list=RDabc12345678"), "{args}");
        }

        #[test]
        fn tool_failures_become_clear_errors() {
            let (limited, _) = fake(
                "429",
                "echo 'ERROR: HTTP Error 429: Too Many Requests' >&2; exit 1",
            );
            assert_eq!(
                YtDlpSource::new(&limited).search("x", SearchFilter::All),
                Err(SourceError::RateLimited)
            );
            let (gone, _) = fake("gone", "echo 'ERROR: Video unavailable' >&2; exit 1");
            assert!(matches!(
                YtDlpSource::new(&gone).resolve_stream("yt:abc"),
                Err(SourceError::Unavailable(_))
            ));
        }

        #[test]
        fn a_hung_tool_is_stopped() {
            let (slow, _) = fake("slow", "sleep 10");
            let s = YtDlpSource::new(&slow).with_timeout(Duration::from_millis(300));
            let t = Instant::now();
            assert!(
                matches!(s.search("x", SearchFilter::All), Err(SourceError::Failed(m)) if m.contains("too long"))
            );
            assert!(t.elapsed() < Duration::from_secs(3));
        }

        #[test]
        fn partial_playlists_still_return_what_was_found() {
            let (partial, _) = fake(
                "partial",
                &format!("printf '%s\\n' '{TRACK}'; echo 'ERROR: Video unavailable' >&2; exit 1"),
            );
            assert_eq!(
                YtDlpSource::new(&partial)
                    .search("x", SearchFilter::All)
                    .unwrap()
                    .len(),
                1
            );
        }
    }
}
