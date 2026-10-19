//! yt-dlp sidecar: search, resolve and related, all through one process wrapper that has a timeout
//! and turns failures into errors the UI can explain.

use crate::gate::{Gate, GateConfig};
use crate::suggest::{self, encode};
use crate::{
    classify_stderr, MusicSource, SearchFilter, SearchItem, SourceError, StreamInfo, StreamMeta,
    TrackInfo, UNKNOWN_ARTIST,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// What the built-in decoder (libav, a minimal in-process build: there is no ffmpeg process) plays.
///
/// First choice is Opus in WebM, YouTube's best quality per bitrate (itag 251, ~130-160 kbps). The
/// fallback is AAC-LC in M4A (itag 140, ~128 kbps; 141, ~256 kbps, with Premium cookies), for videos
/// that offer no Opus. Each filter excludes something that would fail to play or to seek:
///
/// * `acodec=opus` + `ext=webm`: the Opus stream YouTube serves. `acodec^=mp4a.40.2` is AAC-LC only:
///   HE-AAC (`mp4a.40.5`, itag 139) is not in the build's allowlist.
/// * `audio_channels<=?2`: no 5.1 (itags 256/258, 338). The `?` keeps formats that do not report a
///   channel count. It must sit right after the operator: yt-dlp rejects `<=2?` with "Invalid filter
///   specification" and then plays nothing at all.
/// * `protocol=https`: one plain file the range reader can fetch, not an HLS playlist or DASH
///   manifest (`http_dash_segments`).
///
/// If neither matches, yt-dlp reports "Requested format is not available", which becomes a clear
/// "no audio stream this player can decode" error instead of falling back to a codec we cannot play.
pub const FORMAT_SELECTOR: &str =
    "bestaudio[ext=webm][acodec=opus][protocol=https][audio_channels<=?2]/bestaudio[ext=m4a][acodec^=mp4a.40.2][protocol=https][audio_channels<=?2]";
const RESULTS: usize = 20;
/// How long a search answer is reused for the same query and filter (no request at all in that time).
const SEARCH_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const SEARCH_CACHE_MAX: usize = 32;
/// After the dedicated songs page answers with bare titles, songs searches skip it for this long.
const SONGS_PAGE_SKIP: Duration = Duration::from_secs(30 * 60);

/// Optional login cookies for yt-dlp. YouTube is far less likely to answer "sign in to confirm
/// you're not a bot" / HTTP 429 to a logged-in session. Set one of (the file wins if both are set):
///
/// * `PYXEE_COOKIES`: path to a Netscape-format `cookies.txt` (yt-dlp may rewrite it to refresh the session).
/// * `PYXEE_COOKIES_FROM_BROWSER`: yt-dlp's `BROWSER[+KEYRING][:PROFILE][::CONTAINER]`, e.g. `firefox`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Cookies {
    #[default]
    None,
    File(PathBuf),
    Browser(String),
}

impl Cookies {
    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// Reads the settings through `get` (the environment in the app, a map in tests). Logs which
    /// kind is used, never the path or the browser profile.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let var = |k: &str| {
            get(k)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let browser = var("PYXEE_COOKIES_FROM_BROWSER");
        if let Some(file) = var("PYXEE_COOKIES").map(PathBuf::from) {
            if file.is_file() {
                if browser.is_some() {
                    log::info!(
                        "PYXEE_COOKIES and PYXEE_COOKIES_FROM_BROWSER are both set; using the file"
                    );
                }
                log::info!("yt-dlp: using the cookie file from PYXEE_COOKIES");
                return Self::File(file);
            }
            // yt-dlp would create an empty file there and carry on without cookies, silently.
            log::warn!("PYXEE_COOKIES does not point to an existing file; ignoring it");
        }
        if let Some(spec) = browser {
            let name = spec.split(['+', ':']).next().unwrap_or_default();
            log::info!(
                "yt-dlp: reading cookies from the {name} browser (PYXEE_COOKIES_FROM_BROWSER)"
            );
            return Self::Browser(spec);
        }
        Self::None
    }

    fn args(&self) -> Vec<String> {
        match self {
            Self::None => Vec::new(),
            Self::File(p) => vec!["--cookies".into(), p.display().to_string()],
            Self::Browser(b) => vec!["--cookies-from-browser".into(), b.clone()],
        }
    }
}

pub struct YtDlpSource {
    path: PathBuf,
    /// A copy kept up to date by the in-app updater. Used whenever the file exists, so an update applies to the
    /// next search without restarting the app.
    preferred: Option<PathBuf>,
    timeout: Duration,
    gate: Gate,
    cookies: Cookies,
    /// Recent search answers, keyed by lower-cased query and filter.
    search_cache: Mutex<HashMap<(String, String), (Instant, Vec<SearchItem>)>>,
    /// Set while the dedicated songs page is known to answer with bare titles (see `search_fresh`).
    songs_page_bare_until: Mutex<Option<Instant>>,
}

impl YtDlpSource {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            preferred: None,
            timeout: Duration::from_secs(40),
            gate: Gate::new(GateConfig::DEFAULT),
            cookies: Cookies::None,
            search_cache: Mutex::new(HashMap::new()),
            songs_page_bare_until: Mutex::new(None),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Replaces the default pacing (see [`GateConfig`]).
    pub fn with_gate(mut self, cfg: GateConfig) -> Self {
        self.gate = Gate::new(cfg);
        self
    }

    /// Prefers `path` over the located program whenever that file exists (see the field's note).
    pub fn with_preferred(mut self, path: impl Into<PathBuf>) -> Self {
        self.preferred = Some(path.into());
        self
    }

    /// The program the next run starts: the updater's copy if there is one, else the located one.
    pub fn current_path(&self) -> &Path {
        match &self.preferred {
            Some(p) if p.is_file() => p,
            _ => &self.path,
        }
    }

    pub fn with_cookies(mut self, cookies: Cookies) -> Self {
        self.cookies = cookies;
        self
    }

    fn cached_search(&self, key: &(String, String)) -> Option<Vec<SearchItem>> {
        let mut cache = self
            .search_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let fresh = cache
            .get(key)
            .filter(|(at, _)| at.elapsed() < SEARCH_CACHE_TTL)
            .map(|(_, items)| items.clone());
        if fresh.is_none() {
            cache.remove(key);
        }
        fresh
    }

    fn store_search(&self, key: (String, String), items: &[SearchItem]) {
        if items.is_empty() {
            return;
        }
        let mut cache = self
            .search_cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        cache.retain(|_, (at, _)| at.elapsed() < SEARCH_CACHE_TTL);
        if cache.len() >= SEARCH_CACHE_MAX {
            let oldest = cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                cache.remove(&k);
            }
        }
        cache.insert(key, (Instant::now(), items.to_vec()));
    }

    fn songs_page_is_bare(&self) -> bool {
        self.songs_page_bare_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|until| Instant::now() < until)
    }

    fn note_songs_page_bare(&self) {
        *self
            .songs_page_bare_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now() + SONGS_PAGE_SKIP);
    }

    /// A search that is not in the cache. The dedicated songs page answers with bare titles (no artist, no length),
    /// and then the ordinary video search is run as well: two runs. Once that has happened, songs searches go
    /// straight to the video search for a while: one run, so faster and half the requests.
    fn search_fresh(
        &self,
        query: &str,
        filter: SearchFilter,
    ) -> Result<Vec<SearchItem>, SourceError> {
        let skip_songs_page = filter == SearchFilter::Songs && self.songs_page_is_bare();
        let first = if skip_songs_page {
            SearchFilter::Videos
        } else {
            filter
        };
        let out = self.run("search", &flat_args(search_target(query, first)?, RESULTS))?;
        let items = parse_entries(&out)?;
        if filter == SearchFilter::Songs && !skip_songs_page && too_bare(&items) {
            self.note_songs_page_bare();
            log::info!(
                "songs search came back without artists or lengths; using the video search instead"
            );
            let out = self.run(
                "search",
                &flat_args(search_target(query, SearchFilter::Videos)?, RESULTS),
            )?;
            if let Ok(richer) = parse_entries(&out) {
                if !richer.is_empty() && !too_bare(&richer) {
                    return Ok(richer);
                }
            }
        }
        Ok(items)
    }

    /// One yt-dlp run, when the gate says it is this call's turn.
    fn run(&self, what: &str, args: &[String]) -> Result<String, SourceError> {
        let all: Vec<String> = self
            .cookies
            .args()
            .into_iter()
            .chain(args.iter().cloned())
            .collect();
        self.gate
            .run(what, || run_tool(self.current_path(), &all, self.timeout))
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
    let (Some(child_out), Some(child_err)) = (child.stdout.take(), child.stderr.take()) else {
        // Both pipes were requested above, so this should not happen; if it does, do not leave a
        // running child behind and report it instead of panicking a worker thread.
        let _ = child.kill();
        let _ = child.wait();
        return Err(SourceError::Failed(format!(
            "could not read the output of {name}"
        )));
    };
    let out = read(Box::new(child_out));
    let err = read(Box::new(child_err));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                log::warn!("{name} was stopped after {timeout:?}: {args:?}");
                // A stall means the service cannot be reached right now: transient, so the player
                // stays on the song with a Retry instead of skipping it (the song is not the problem).
                return Err(SourceError::Offline);
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

/// The largest thumbnail yt-dlp lists (by area, then by its own preference), else the top-level
/// `thumbnail`, else YouTube's standard still for `id`. Plain `http` is upgraded to `https`.
fn thumbnail(v: &Value, id: Option<&str>) -> Option<String> {
    let list = v.get("thumbnails").and_then(Value::as_array);
    let best = list.and_then(|l| {
        l.iter()
            .filter_map(|t| {
                let url = t.get("url")?.as_str().filter(|u| !u.is_empty())?;
                let w = t.get("width").and_then(Value::as_u64).unwrap_or(0);
                let h = t.get("height").and_then(Value::as_u64).unwrap_or(w);
                let pref = t.get("preference").and_then(Value::as_i64).unwrap_or(0);
                Some((url, (w * h, pref)))
            })
            .max_by_key(|(_, rank)| *rank)
            .map(|(u, _)| u.to_string())
    });
    best.or_else(|| text(v, "thumbnail"))
        .or_else(|| id.map(|i| format!("https://i.ytimg.com/vi/{i}/hqdefault.jpg")))
        .map(|u| u.replacen("http://", "https://", 1))
}

/// The performer: music metadata when yt-dlp has it (`artists`, `artist`), else the uploader or
/// channel, with YouTube's auto-generated " - Topic" suffix removed.
fn artist_of(v: &Value) -> Option<String> {
    let listed = v.get("artists").and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    });
    let raw = listed
        .filter(|s| !s.is_empty())
        .or_else(|| text(v, "artist"))
        .or_else(|| text(v, "creator"))
        .or_else(|| text(v, "album_artist"))
        .or_else(|| text(v, "uploader"))
        .or_else(|| text(v, "channel"))?;
    let name = raw.strip_suffix(" - Topic").unwrap_or(&raw).trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn artist_name(v: &Value) -> String {
    artist_of(v).unwrap_or_else(|| UNKNOWN_ARTIST.into())
}

/// Seconds from `duration` (a number, or a number in a string), or from a text such as `3:45` / `1:02:03` in
/// `duration_string`, `duration_text` or `length` (the names different extractors use).
fn duration_secs(v: &Value) -> Option<f64> {
    if let Some(d) = v.get("duration").and_then(|d| {
        d.as_f64()
            .or_else(|| d.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
    }) {
        if d > 0.0 {
            return Some(d);
        }
    }
    for key in ["duration_string", "duration_text", "length"] {
        let Some(s) = text(v, key) else { continue };
        let mut total = 0.0;
        let mut ok = true;
        for part in s.split(':') {
            match part.trim().parse::<f64>() {
                Ok(n) => total = total * 60.0 + n,
                Err(_) => ok = false,
            }
        }
        if ok && total > 0.0 {
            return Some(total);
        }
    }
    None
}

/// More than half of the track rows have no artist or no length: not good enough to show.
fn too_bare(items: &[SearchItem]) -> bool {
    let tracks: Vec<&TrackInfo> = items
        .iter()
        .filter_map(|i| match i {
            SearchItem::Track(t) => Some(t),
            _ => None,
        })
        .collect();
    let bare = tracks
        .iter()
        .filter(|t| t.artist == UNKNOWN_ARTIST || t.duration_ms.is_none())
        .count();
    !tracks.is_empty() && bare * 2 > tracks.len()
}

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
    let artist = artist_of(v).unwrap_or_else(|| UNKNOWN_ARTIST.into());
    Some(SearchItem::Track(TrackInfo {
        key: format!("yt:{id}"),
        title,
        artist,
        duration_ms: duration_secs(v).map(|s| (s * 1000.0) as u64),
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
    http_headers: Option<std::collections::BTreeMap<String, String>>,
}

/// Parses `yt-dlp -j -f ...` output for a single video.
pub fn parse_dump(json: &str) -> Result<Resolved, SourceError> {
    let d: Dump = serde_json::from_str(json).map_err(|e| SourceError::Parse(e.to_string()))?;
    let v: Value = serde_json::from_str(json).map_err(|e| SourceError::Parse(e.to_string()))?;
    let url = d
        .url
        .ok_or_else(|| SourceError::Parse("no stream url in output".into()))?;
    Ok(Resolved {
        id: d.id.clone(),
        title: d.title.unwrap_or_else(|| "Unknown title".into()),
        uploader: d.uploader.or(d.channel),
        artist: artist_of(&v),
        duration_secs: duration_secs(&v).or(d.duration),
        thumbnail: thumbnail(&v, Some(&d.id)),
        stream_url: url,
        ext: d.ext.unwrap_or_default(),
        acodec: d.acodec,
        abr_kbps: d.abr,
        headers: d.http_headers.unwrap_or_default().into_iter().collect(),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    /// Best guess at the performer (music metadata, else uploader/channel without " - Topic").
    pub artist: Option<String>,
    pub duration_secs: Option<f64>,
    /// Largest thumbnail on offer.
    pub thumbnail: Option<String>,
    pub stream_url: String,
    pub ext: String,
    pub acodec: Option<String>,
    pub abr_kbps: Option<f64>,
    /// Headers yt-dlp used for this URL (sorted by name).
    pub headers: Vec<(String, String)>,
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
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let key = (q.to_lowercase(), format!("{filter:?}"));
        if let Some(hit) = self.cached_search(&key) {
            log::info!("search: answered from the session cache, no yt-dlp run");
            return Ok(hit);
        }
        let items = self.search_fresh(query, filter)?;
        self.store_search(key, &items);
        Ok(items)
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
        let out = self.run("resolve", &args)?;
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
            headers: r.headers,
            meta: StreamMeta {
                title: None,
                artist: r.artist,
                duration_secs: r.duration_secs,
                thumbnail: r.thumbnail,
            },
        })
    }

    fn related(&self, key: &str, count: usize) -> Result<Vec<TrackInfo>, SourceError> {
        let id = video_id(key)?;
        let target = format!("https://www.youtube.com/watch?v={id}&list=RD{id}");
        let out = self.run("related", &flat_args(target, count + 1))?;
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
    fn track_item(json: &str) -> SearchItem {
        item_from_entry(&serde_json::from_str(json).unwrap()).unwrap()
    }

    fn track_of(json: &str) -> TrackInfo {
        match item_from_entry(&serde_json::from_str(json).unwrap()) {
            Some(SearchItem::Track(t)) => t,
            other => panic!("not a track: {other:?}"),
        }
    }

    #[test]
    fn a_song_entry_with_music_metadata_gets_its_artists_joined_and_no_topic_suffix() {
        let t = track_of(
            r#"{"id":"s1","url":"https://music.youtube.com/watch?v=s1","title":"Duet","artists":["Ana","Ben"],"uploader":"Label - Topic","duration":181}"#,
        );
        assert_eq!(t.artist, "Ana, Ben");
        assert_eq!(t.duration_ms, Some(181_000));
    }

    #[test]
    fn durations_come_from_numbers_strings_and_the_other_names_extractors_use() {
        let secs = |j: &str| duration_secs(&serde_json::from_str::<Value>(j).unwrap());
        assert_eq!(secs(r#"{"duration":"213"}"#), Some(213.0));
        assert_eq!(
            secs(r#"{"duration":null,"duration_text":"4:05"}"#),
            Some(245.0)
        );
        assert_eq!(secs(r#"{"length":"1:00:01"}"#), Some(3_601.0));
        assert_eq!(secs(r#"{"duration":0,"duration_string":"abc"}"#), None);
        assert_eq!(secs(r#"{}"#), None);
    }

    #[test]
    fn a_title_is_never_split_into_artist_and_song_because_that_guess_is_often_wrong() {
        // Real titles look like "Song (Artist) - Description": splitting at " - " would swap song and artist.
        let t = track_of(
            r#"{"id":"v1","title":"Sneaky Snitch (Kevin MacLeod) - Background Music (HD)"}"#,
        );
        assert_eq!(t.artist, UNKNOWN_ARTIST);
        assert_eq!(
            t.title,
            "Sneaky Snitch (Kevin MacLeod) - Background Music (HD)"
        );
    }

    #[test]
    fn a_result_list_without_artists_or_lengths_is_recognised_as_too_bare() {
        let bare = |n: usize| -> Vec<SearchItem> {
            (0..n)
                .map(|i| track_item(&format!(r#"{{"id":"b{i:010}","title":"T{i}"}}"#)))
                .collect()
        };
        let rich = r#"{"id":"r0000000000","title":"T","uploader":"A","duration":100}"#;
        assert!(too_bare(&bare(11)));
        let mut mixed = bare(3);
        mixed.extend((0..8).map(|_| track_item(rich)));
        assert!(!too_bare(&mixed), "mostly complete rows are fine");
        assert!(!too_bare(&[]), "nothing to judge");
    }

    #[test]
    fn a_song_entry_with_only_a_duration_string_still_has_a_length() {
        let t = track_of(r#"{"id":"s2","title":"T","artist":"Solo","duration_string":"3:45"}"#);
        assert_eq!((t.artist.as_str(), t.duration_ms), ("Solo", Some(225_000)));
        let long = track_of(r#"{"id":"s3","title":"T","duration_string":"1:02:03"}"#);
        assert_eq!(long.duration_ms, Some(3_723_000));
    }

    #[test]
    fn a_song_entry_with_no_artist_information_is_marked_unknown_so_it_can_be_filled_in_later() {
        let t = track_of(r#"{"id":"s4","title":"T"}"#);
        assert_eq!(t.artist, UNKNOWN_ARTIST);
        assert_eq!(t.duration_ms, None);
    }

    #[test]
    fn the_largest_thumbnail_wins_over_the_smallest_and_over_a_made_up_url() {
        let t = track_of(
            r#"{"id":"s5","title":"T","thumbnails":[{"url":"http://a/small.jpg","width":120,"height":90},{"url":"https://a/max.jpg","width":1280,"height":720},{"url":"https://a/mid.jpg","width":480,"height":360}]}"#,
        );
        assert_eq!(t.thumbnail.as_deref(), Some("https://a/max.jpg"));
        // Without any list the plain `thumbnail` is used, and http becomes https.
        let t = track_of(r#"{"id":"s6","title":"T","thumbnail":"http://a/one.jpg"}"#);
        assert_eq!(t.thumbnail.as_deref(), Some("https://a/one.jpg"));
    }

    #[test]
    fn resolving_reports_artist_length_and_cover_so_the_ui_can_fill_what_search_left_out() {
        let r = parse_dump(
            r#"{"id":"abc","title":"S","channel":"Lowtide - Topic","duration":200.0,"url":"https://x/y","ext":"webm","acodec":"opus","abr":139.0,"http_headers":{"User-Agent":"UA/1","Accept-Language":"en-us"},"thumbnails":[{"url":"https://a/s.jpg","width":120},{"url":"https://a/max.jpg","width":1280,"height":720}]}"#,
        )
        .unwrap();
        assert_eq!(r.artist.as_deref(), Some("Lowtide"));
        assert_eq!(r.duration_secs, Some(200.0));
        assert_eq!(r.thumbnail.as_deref(), Some("https://a/max.jpg"));
        assert_eq!(
            r.headers,
            vec![
                ("Accept-Language".to_string(), "en-us".to_string()),
                ("User-Agent".to_string(), "UA/1".to_string())
            ]
        );
    }

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
    fn the_format_selector_asks_only_for_what_the_built_in_decoder_can_play() {
        // Opus first, AAC-LC second, nothing else: see FORMAT_SELECTOR.
        for banned in ["vorbis", "ba/", "mp4a.40.5"] {
            assert!(
                !FORMAT_SELECTOR.contains(banned),
                "{banned} in {FORMAT_SELECTOR}"
            );
        }
        let alternatives: Vec<&str> = FORMAT_SELECTOR.split('/').collect();
        assert_eq!(
            alternatives.len(),
            2,
            "exactly Opus then AAC-LC, no fallback to a format that cannot be played"
        );
        for alt in &alternatives {
            assert!(
                alt.starts_with("bestaudio["),
                "no bare `best`/`bestaudio` fallback that could pick an unplayable format: {alt}"
            );
        }
        for (alt, needed) in [
            (alternatives[0], ["ext=webm", "acodec=opus"]),
            (alternatives[1], ["ext=m4a", "acodec^=mp4a.40.2"]),
        ] {
            for n in needed
                .into_iter()
                .chain(["protocol=https", "audio_channels<=?2"])
            {
                assert!(alt.contains(n), "{n} missing from {alt}");
            }
        }
    }

    #[test]
    fn every_filter_in_the_format_selector_is_valid_yt_dlp_syntax() {
        // yt-dlp only accepts the "keep formats that do not report this field" marker right after the operator
        // (`audio_channels<=?2`). `<=2?` is a SyntaxError that fails every single play, before any network call.
        // scripts/tests/test_format_selector.py also checks the real string against yt-dlp itself.
        let filters: Vec<&str> = FORMAT_SELECTOR
            .split('/')
            .flat_map(|alt| alt.split('[').skip(1))
            .map(|f| f.strip_suffix(']').expect("every filter closes with ]"))
            .collect();
        assert!(filters.len() >= 8, "{FORMAT_SELECTOR}");
        for filter in filters {
            if let Some(q) = filter.find('?') {
                assert!(
                    q > 0 && matches!(filter.as_bytes()[q - 1], b'<' | b'>' | b'=' | b'~'),
                    "the ? must directly follow the operator: [{filter}]"
                );
                assert!(q + 1 < filter.len(), "nothing after the ? in [{filter}]");
            }
        }
    }

    #[test]
    fn no_playable_format_is_a_clear_error_not_a_generic_tool_failure() {
        let e = classify_stderr(
            "yt-dlp",
            "ERROR: [youtube] abc: Requested format is not available. Use --list-formats for a list of available formats",
        );
        assert!(
            matches!(&e, SourceError::Failed(m) if m.contains("no audio stream this player can decode")),
            "{e:?}"
        );
    }

    #[test]
    fn a_403_or_a_5xx_from_youtube_is_transient_and_a_missing_video_is_not() {
        let c = |s: &str| classify_stderr("yt-dlp", s);
        // Blocked client or IP: transient (stay on the song, cooldown), not "this song is bad".
        assert_eq!(
            c("ERROR: unable to download video data: HTTP Error 403: Forbidden"),
            SourceError::RateLimited
        );
        // YouTube's servers failing: transient.
        for e in [
            "ERROR: HTTP Error 503: Service Unavailable",
            "ERROR: HTTP Error 502: Bad Gateway",
        ] {
            assert_eq!(c(e), SourceError::Offline, "{e}");
        }
        // The bot check is transient (cookies or a wait help); the age gate is permanent for this player.
        assert_eq!(
            c("ERROR: [youtube] abc: Sign in to confirm you\u{2019}re not a bot"),
            SourceError::RateLimited
        );
        assert!(matches!(
            c("ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate"),
            SourceError::Unavailable(_)
        ));
        // Permanent: wins over a 403 in the same output, and never counts as transient.
        assert!(matches!(
            c("ERROR: [youtube] abc: Video unavailable. HTTP Error 403: Forbidden"),
            SourceError::Unavailable(_)
        ));
        // An out-of-date yt-dlp is named as such instead of a bare traceback line.
        assert!(matches!(
            c("ERROR: [youtube] abc: Unable to extract player"),
            SourceError::Failed(m) if m.contains("out of date")
        ));
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: std::collections::HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn cookie_settings_come_from_the_environment_and_a_bad_file_is_ignored() {
        assert_eq!(Cookies::from_lookup(env(&[])), Cookies::None);
        assert_eq!(
            Cookies::from_lookup(env(&[
                ("PYXEE_COOKIES", "  "),
                ("PYXEE_COOKIES_FROM_BROWSER", "")
            ])),
            Cookies::None,
            "blank settings count as unset"
        );
        let file = std::env::temp_dir().join(format!("pyxee-cookies-{}.txt", std::process::id()));
        std::fs::write(&file, "# Netscape HTTP Cookie File\n").unwrap();
        let path = file.display().to_string();
        assert_eq!(
            Cookies::from_lookup(env(&[("PYXEE_COOKIES", &path)])),
            Cookies::File(file.clone())
        );
        assert_eq!(
            Cookies::from_lookup(env(&[
                ("PYXEE_COOKIES", &path),
                ("PYXEE_COOKIES_FROM_BROWSER", "firefox")
            ])),
            Cookies::File(file),
            "the file wins when both are set"
        );
        assert_eq!(
            Cookies::from_lookup(env(&[("PYXEE_COOKIES", "/no/such/cookies.txt")])),
            Cookies::None,
            "a missing file must not be handed to yt-dlp (it would create one)"
        );
        assert_eq!(
            Cookies::from_lookup(env(&[
                ("PYXEE_COOKIES", "/no/such/cookies.txt"),
                ("PYXEE_COOKIES_FROM_BROWSER", "chrome:Profile 1")
            ])),
            Cookies::Browser("chrome:Profile 1".into()),
            "a bad file falls through to the browser setting"
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

        /// A source with no pacing, for tests that are not about pacing.
        fn quick(script: &Path) -> YtDlpSource {
            YtDlpSource::new(script).with_gate(GateConfig::NONE)
        }

        fn runs_logged(log: &Path) -> usize {
            std::fs::read_to_string(log).map_or(0, |t| t.lines().count())
        }

        fn canned() -> String {
            let dump = r#"{"id":"abc12345678","title":"Neon Harbor","uploader":"Lowtide","duration":200.0,"url":"https://stream.example/a","ext":"m4a","acodec":"mp4a.40.2","abr":129.0}"#;
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
        fn a_bare_songs_list_is_replaced_by_the_video_search_once_and_a_good_one_is_kept() {
            let bare = r#"{"id":"s0000000001","title":"Song"}"#;
            let rich = r#"{"id":"v0000000001","title":"Song","uploader":"Artist","duration":100}"#;
            let body = format!(
                "case \"$*\" in\n  *music.youtube.com*) printf '%s\\n' '{bare}' '{bare}' ;;\n  *) printf '%s\\n' '{rich}' ;;\nesac"
            );
            let (script, log) = fake("bare-songs", &body);
            let s = quick(&script);
            let found = s.search("song", SearchFilter::Songs).unwrap();
            assert_eq!(runs_logged(&log), 2, "one songs run, one video run");
            match &found[..] {
                [SearchItem::Track(t)] => assert_eq!(
                    (t.artist.as_str(), t.duration_ms),
                    ("Artist", Some(100_000))
                ),
                other => panic!("{other:?}"),
            }
            // A songs list that is complete is used as it is: no second run.
            let good = r#"{"id":"s0000000002","title":"Song","uploader":"Artist","duration":100}"#;
            let (script, log) = fake("good-songs", &format!("printf '%s\\n' '{good}'"));
            quick(&script).search("song", SearchFilter::Songs).unwrap();
            assert_eq!(runs_logged(&log), 1);
        }

        #[test]
        fn search_resolve_and_related_drive_the_tool_with_the_right_arguments() {
            let (script, log) = fake("ok", &canned());
            let s = quick(&script);

            let found = s.search("lowtide", SearchFilter::All).unwrap();
            assert_eq!(found.len(), 2);
            let stream = s.resolve_stream("yt:abc12345678").unwrap();
            assert_eq!(
                (stream.url.as_str(), stream.ext.as_str()),
                ("https://stream.example/a", "m4a")
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
            assert!(
                args.contains(&format!(
                    "-f {FORMAT_SELECTOR} -j https://www.youtube.com/watch?v=abc12345678"
                )),
                "{args}"
            );
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
        fn a_hung_tool_is_stopped_and_counts_as_unreachable_not_as_a_bad_song() {
            let (slow, _) = fake("slow", "sleep 10");
            let s = YtDlpSource::new(&slow).with_timeout(Duration::from_millis(300));
            let t = Instant::now();
            assert_eq!(
                s.search("x", SearchFilter::All),
                Err(SourceError::Offline),
                "transient: the player stays on the song"
            );
            assert!(t.elapsed() < Duration::from_secs(3));
        }

        #[test]
        fn after_a_rate_limit_later_calls_are_refused_without_starting_the_tool() {
            let (limited, log) = fake(
                "cooldown",
                "echo 'ERROR: HTTP Error 429: Too Many Requests' >&2; exit 1",
            );
            let gate = GateConfig {
                min_gap: Duration::ZERO,
                cooldown: Duration::from_secs(60),
            };
            let s = YtDlpSource::new(&limited).with_gate(gate);
            assert_eq!(
                s.search("a", SearchFilter::All),
                Err(SourceError::RateLimited)
            );
            assert_eq!(runs_logged(&log), 1);
            let t = Instant::now();
            // Every kind of call is refused, instantly, and none of them starts yt-dlp.
            assert_eq!(
                s.search("b", SearchFilter::Songs),
                Err(SourceError::RateLimited)
            );
            assert_eq!(s.resolve_stream("yt:abc"), Err(SourceError::RateLimited));
            assert_eq!(s.related("yt:abc", 5), Err(SourceError::RateLimited));
            assert!(t.elapsed() < Duration::from_secs(1));
            assert_eq!(runs_logged(&log), 1, "a retry storm reached the tool");
        }

        #[test]
        fn the_tool_is_used_again_once_the_cooldown_is_over() {
            // Rate limited on its first run (the wrapper has logged one line by then), fine after.
            let (flaky, log) = fake(
                "recover",
                &format!(
                    "if [ \"$(wc -l < \"$(dirname \"$0\")/args.log\")\" -le 1 ]; then\n  echo 'ERROR: HTTP Error 429: Too Many Requests' >&2; exit 1\nfi\nprintf '%s\\n' '{TRACK}'"
                ),
            );
            let gate = GateConfig {
                min_gap: Duration::ZERO,
                cooldown: Duration::from_millis(60),
            };
            let s = YtDlpSource::new(&flaky).with_gate(gate);
            assert_eq!(
                s.search("a", SearchFilter::All),
                Err(SourceError::RateLimited)
            );
            std::thread::sleep(Duration::from_millis(120));
            assert_eq!(s.search("a", SearchFilter::All).unwrap().len(), 1);
            assert_eq!(runs_logged(&log), 2);
        }

        #[test]
        fn simultaneous_calls_run_the_tool_one_at_a_time() {
            // The stand-in records when it starts and ends; overlapping runs would interleave.
            let (slow, log) = fake("serial", "");
            let order = log.with_file_name("order.log");
            let body = format!(
                "echo start >> '{o}'\nsleep 0.04\necho end >> '{o}'\nprintf '%s\\n' '{TRACK}'",
                o = order.display()
            );
            let script = std::fs::read_to_string(&slow).unwrap();
            std::fs::write(&slow, format!("{script}{body}\n")).unwrap();
            let _ = std::fs::remove_file(&order);
            let s = std::sync::Arc::new(quick(&slow));
            let handles: Vec<_> = (0..5)
                .map(|i| {
                    let s = s.clone();
                    std::thread::spawn(move || s.search(&format!("q{i}"), SearchFilter::All))
                })
                .collect();
            for h in handles {
                assert_eq!(h.join().unwrap().unwrap().len(), 1);
            }
            let seen: Vec<String> = std::fs::read_to_string(&order)
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect();
            let expected: Vec<String> = (0..5)
                .flat_map(|_| ["start".to_string(), "end".to_string()])
                .collect();
            assert_eq!(seen, expected, "yt-dlp runs overlapped");
        }

        #[test]
        fn cookies_reach_every_kind_of_call_and_are_absent_by_default() {
            let (script, log) = fake("cookies", &canned());
            let file = log.with_file_name("cookies.txt");
            std::fs::write(&file, "# Netscape HTTP Cookie File\n").unwrap();
            let s = quick(&script).with_cookies(Cookies::File(file.clone()));
            s.search("x", SearchFilter::All).unwrap();
            s.resolve_stream("yt:abc12345678").unwrap();
            s.related("yt:abc12345678", 3).unwrap();
            let lines: Vec<String> = std::fs::read_to_string(&log)
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect();
            assert_eq!(lines.len(), 3);
            let want = format!("--cookies {}", file.display());
            assert!(lines.iter().all(|l| l.starts_with(&want)), "{lines:#?}");

            let (script, log) = fake("nocookies", &canned());
            quick(&script).search("x", SearchFilter::All).unwrap();
            assert!(!std::fs::read_to_string(log).unwrap().contains("cookies"));

            let (script, log) = fake("browser", &canned());
            quick(&script)
                .with_cookies(Cookies::Browser("firefox:default".into()))
                .search("x", SearchFilter::All)
                .unwrap();
            assert!(std::fs::read_to_string(log)
                .unwrap()
                .starts_with("--cookies-from-browser firefox:default "));
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

    #[test]
    fn the_updaters_copy_is_used_only_while_it_exists() {
        let dir = std::env::temp_dir().join(format!("pyxee-ytdlp-pref-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let managed = dir.join("yt-dlp.exe");
        let src = YtDlpSource::new("installed/yt-dlp.exe").with_preferred(&managed);
        assert_eq!(src.current_path(), Path::new("installed/yt-dlp.exe"));
        std::fs::write(&managed, b"MZ").unwrap();
        assert_eq!(src.current_path(), managed.as_path());
        std::fs::remove_file(&managed).unwrap();
        assert_eq!(src.current_path(), Path::new("installed/yt-dlp.exe"));
        assert_eq!(
            YtDlpSource::new("a").current_path(),
            Path::new("a"),
            "no preferred copy configured"
        );
    }
}

#[cfg(test)]
mod speed_tests {
    use super::*;

    fn item(n: &str) -> SearchItem {
        SearchItem::Track(crate::TrackInfo {
            key: format!("yt:{n}"),
            title: n.to_string(),
            artist: "Artist".into(),
            duration_ms: Some(1000),
            thumbnail: None,
        })
    }

    #[test]
    fn the_same_search_is_answered_from_the_cache_and_an_empty_answer_is_not_kept() {
        let s = YtDlpSource::new("yt-dlp-that-is-never-started");
        let key = ("low tide".to_string(), "Songs".to_string());
        assert!(s.cached_search(&key).is_none());
        s.store_search(key.clone(), &[]);
        assert!(s.cached_search(&key).is_none(), "empty answers are not cached");
        s.store_search(key.clone(), &[item("a"), item("b")]);
        assert_eq!(s.cached_search(&key).map(|v| v.len()), Some(2));
        let other = ("low tide".to_string(), "Videos".to_string());
        assert!(s.cached_search(&other).is_none(), "another filter is another search");
    }

    #[test]
    fn the_cache_never_holds_more_than_its_limit() {
        let s = YtDlpSource::new("yt-dlp-that-is-never-started");
        for i in 0..(SEARCH_CACHE_MAX + 10) {
            s.store_search((format!("q{i}"), "All".into()), &[item("x")]);
        }
        let n = s.search_cache.lock().unwrap().len();
        assert_eq!(n, SEARCH_CACHE_MAX);
    }

    #[test]
    fn a_bare_songs_page_is_skipped_afterwards() {
        let s = YtDlpSource::new("yt-dlp-that-is-never-started");
        assert!(!s.songs_page_is_bare());
        s.note_songs_page_bare();
        assert!(s.songs_page_is_bare());
    }
}
