//! Headless live check of the real production path: `YtDlpSource` (gate, cookies, format selector)
//! -> `HttpRangeSource` -> `SymphoniaSource` -> `GaplessSource`. No audio device is needed.
//! Driven by `scripts/check-windows.ps1`; usable by hand:
//!
//!   livecheck <formats|search|stream|stress|fake429> [--ytdlp PATH] [--q1 QUERY] [--q2 QUERY]
//!
//! Output is one line per result, `CHECK <PASS|FAIL|WARN|INFO> <name> :: <detail>`, plus `LOG ...`
//! lines from the app's own logging (yt-dlp gate waits, cookie use). Stream URLs are never printed:
//! every line is scrubbed down to the host. Exit code 1 if any check failed.

use anyhow::{anyhow, bail, Result};
use pyxee_audio::decode::{SampleSource, SymphoniaSource};
use pyxee_audio::gap::find_gaps;
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};
use pyxee_audio::http::HttpRangeSource;
use pyxee_audio::AudioError;
use pyxee_sources::gate::GateConfig;
use pyxee_sources::ytdlp::{Cookies, YtDlpSource, FORMAT_SELECTOR};
use pyxee_sources::{MusicSource, SearchFilter, SearchItem, SourceError, StreamInfo, TrackInfo};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static FAILS: AtomicUsize = AtomicUsize::new(0);

/// Replaces every URL with `<url host=...>`: stream URLs carry signatures and must not be logged.
fn scrub(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("http") {
        let (head, tail) = rest.split_at(i);
        out.push_str(head);
        if tail.starts_with("http://") || tail.starts_with("https://") {
            let end = tail
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | '>' | ','))
                .unwrap_or(tail.len());
            let host = tail[..end]
                .split("://")
                .nth(1)
                .unwrap_or("")
                .split(['/', '?'])
                .next()
                .unwrap_or("");
            out.push_str(&format!("<url host={host}>"));
            rest = &tail[end..];
        } else {
            out.push_str("http");
            rest = &tail[4..];
        }
    }
    out.push_str(rest);
    out
}

fn emit(status: &str, name: &str, detail: impl AsRef<str>) {
    if status == "FAIL" {
        FAILS.fetch_add(1, Ordering::SeqCst);
    }
    let d = scrub(detail.as_ref()).replace(['\r', '\n'], " | ");
    println!("CHECK {status} {name} :: {d}");
    let _ = std::io::stdout().flush();
}

fn verdict(ok: bool, name: &str, detail: impl AsRef<str>) {
    emit(if ok { "PASS" } else { "FAIL" }, name, detail);
}

struct Stdout;
impl log::Log for Stdout {
    fn enabled(&self, m: &log::Metadata) -> bool {
        // Only Pyxee's own decisions (gate waits, cookies, tool failures), not Symphonia's chatter.
        m.level() <= log::Level::Info && m.target().starts_with("pyxee")
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            println!("LOG {} {}", r.level(), scrub(&r.args().to_string()));
            let _ = std::io::stdout().flush();
        }
    }
    fn flush(&self) {}
}

struct Opts {
    ytdlp: PathBuf,
    q1: String,
    q2: String,
}

fn source(o: &Opts) -> YtDlpSource {
    YtDlpSource::new(&o.ytdlp).with_cookies(Cookies::from_env())
}

fn cookie_args() -> Vec<String> {
    match Cookies::from_env() {
        Cookies::None => Vec::new(),
        Cookies::File(p) => vec!["--cookies".into(), p.display().to_string()],
        Cookies::Browser(b) => vec!["--cookies-from-browser".into(), b],
    }
}

fn first_track(src: &dyn MusicSource, q: &str) -> Result<TrackInfo> {
    for filter in [SearchFilter::Songs, SearchFilter::All] {
        for item in src.search(q, filter).map_err(|e| anyhow!("search: {e}"))? {
            if let SearchItem::Track(t) = item {
                return Ok(t);
            }
        }
    }
    bail!("no track found for {q:?}")
}

fn secs(d: Duration) -> String {
    format!("{:.2}s", d.as_secs_f64())
}

// ---------------------------------------------------------------- formats

fn formats(o: &Opts) -> Result<()> {
    let ver = Command::new(&o.ytdlp).arg("--version").output()?;
    emit(
        "INFO",
        "yt-dlp-version",
        String::from_utf8_lossy(&ver.stdout).trim(),
    );
    emit("INFO", "format-selector", FORMAT_SELECTOR);
    let src = source(o);
    let t = first_track(&src, &o.q1)?;
    let id = t.key.trim_start_matches("yt:").to_string();
    let mut args = cookie_args();
    args.extend(["-F".into(), "--no-playlist".into()]);
    args.push(format!("https://www.youtube.com/watch?v={id}"));
    let out = Command::new(&o.ytdlp).args(&args).output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        let last = stderr.lines().last().unwrap_or("(no output)");
        emit("FAIL", "format-list", format!("yt-dlp -F failed: {last}"));
        return Ok(());
    }
    let (mut decodable, mut audio_rows) = (0, 0);
    for line in stdout.lines().filter(|l| l.contains("audio only")) {
        audio_rows += 1;
        let row = line.split_whitespace().collect::<Vec<_>>().join(" ");
        let ok = row.contains(" m4a ") && row.contains("mp4a.40.2") && row.contains("https");
        decodable += usize::from(ok);
        emit(
            "INFO",
            "audio-format",
            format!(
                "{row}  [{}]",
                if ok { "playable" } else { "not playable here" }
            ),
        );
    }
    let mut seen = std::collections::HashSet::new();
    for line in stderr.lines().filter(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("warning")
            || l.contains("javascript")
            || l.contains("deno")
            || l.contains("po token")
            || l.contains("challenge")
    }) {
        if seen.insert(line.to_string()) && seen.len() <= 8 {
            emit("WARN", "yt-dlp-warning", line);
        }
    }
    verdict(
        decodable > 0,
        "playable-format-offered",
        format!("{decodable} of {audio_rows} audio-only formats are AAC-LC/M4A over https"),
    );
    Ok(())
}

// ---------------------------------------------------------------- search

fn search(o: &Opts) -> Result<()> {
    let src = source(o);
    for (name, filter) in [
        ("all", SearchFilter::All),
        ("songs", SearchFilter::Songs),
        ("videos", SearchFilter::Videos),
        ("artists", SearchFilter::Artists),
        ("playlists", SearchFilter::Playlists),
    ] {
        let t = Instant::now();
        match src.search(&o.q1, filter) {
            Ok(items) if !items.is_empty() => {
                let first = match &items[0] {
                    SearchItem::Track(t) => format!("track {:?}", t.title),
                    SearchItem::Playlist { title, .. } => format!("playlist {title:?}"),
                    _ => "other".to_string(),
                };
                emit(
                    "PASS",
                    &format!("search-{name}"),
                    format!(
                        "{} results in {}; first: {first}",
                        items.len(),
                        secs(t.elapsed())
                    ),
                );
            }
            Ok(_) => emit("WARN", &format!("search-{name}"), "0 results"),
            Err(e) => emit("FAIL", &format!("search-{name}"), e.to_string()),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- stream

#[derive(Default)]
struct Stats {
    frames: usize,
    sum_sq: f64,
    peak: f32,
    bad: usize,
}

impl Stats {
    fn rms(&self, channels: usize) -> f64 {
        (self.sum_sq / (self.frames.max(1) * channels) as f64).sqrt()
    }
}

fn decode_all(s: &mut SymphoniaSource) -> Result<Stats, AudioError> {
    let (mut st, mut buf) = (Stats::default(), Vec::new());
    let ch = s.channels();
    loop {
        buf.clear();
        if !s.read(&mut buf)? {
            return Ok(st);
        }
        st.frames += buf.len() / ch;
        for &x in &buf {
            if !x.is_finite() {
                st.bad += 1;
            } else {
                st.sum_sq += f64::from(x) * f64::from(x);
                st.peak = st.peak.max(x.abs());
            }
        }
    }
}

fn open(stream: &StreamInfo, url: &str) -> Result<SymphoniaSource, AudioError> {
    let http = HttpRangeSource::open_with(url, &stream.headers)?;
    SymphoniaSource::open(Box::new(http), Some(&stream.ext))
}

/// Corrupts the signature (or expiry) so the CDN must refuse the URL, like an expired one.
fn break_url(url: &str) -> Option<String> {
    for key in ["&sig=", "?sig=", "&lsig=", "&expire=", "?expire="] {
        if let Some(i) = url.find(key) {
            let at = i + key.len();
            let mut b = url.as_bytes().to_vec();
            if at < b.len() {
                b[at] = if b[at] == b'A' { b'B' } else { b'A' };
                return String::from_utf8(b).ok();
            }
        }
    }
    None
}

fn within(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn stream(o: &Opts) -> Result<()> {
    let src = source(o);
    let track = first_track(&src, &o.q1)?;
    emit(
        "INFO",
        "track",
        format!(
            "{:?} by {:?}, listed {:?} ms",
            track.title, track.artist, track.duration_ms
        ),
    );

    let t = Instant::now();
    let s = src
        .resolve_stream(&track.key)
        .map_err(|e| anyhow!("resolve: {e}"))?;
    let dur = s
        .meta
        .duration_secs
        .or(track.duration_ms.map(|m| m as f64 / 1000.0))
        .ok_or_else(|| anyhow!("no duration known"))?;
    let host = s
        .url
        .split("://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");
    emit(
        "INFO",
        "resolved",
        format!(
            "ext={} acodec={:?} abr={:?} headers={} host={host} duration={dur:.1}s in {}",
            s.ext,
            s.acodec,
            s.abr_kbps,
            s.headers.len(),
            secs(t.elapsed())
        ),
    );
    let playable = s.ext == "m4a"
        && s.acodec
            .as_deref()
            .is_some_and(|c| c.starts_with("mp4a.40.2"));
    verdict(
        playable,
        "format-is-aac-lc-m4a",
        format!("{} / {:?}", s.ext, s.acodec),
    );

    // Open + full decode.
    let t = Instant::now();
    let mut a = open(&s, &s.url).map_err(|e| anyhow!("open: {e}"))?;
    let (rate, ch) = (a.sample_rate(), a.channels());
    emit(
        "PASS",
        "open",
        format!("{rate} Hz, {ch} ch, opened in {}", secs(t.elapsed())),
    );
    let t = Instant::now();
    match decode_all(&mut a) {
        Ok(st) => {
            let len = st.frames as f64 / f64::from(rate);
            verdict(
                within(len, dur, 2.5) && st.bad == 0,
                "full-decode",
                format!(
                    "{len:.1}s decoded of {dur:.1}s expected, rms {:.4}, peak {:.2}, bad samples {}, in {}",
                    st.rms(ch),
                    st.peak,
                    st.bad,
                    secs(t.elapsed())
                ),
            );
            verdict(
                st.rms(ch) > 0.003,
                "full-decode-not-silent",
                format!("rms {:.4}", st.rms(ch)),
            );
        }
        Err(e) => emit("FAIL", "full-decode", e.to_string()),
    }

    // Seeks, from fresh sources on the same resolved URL.
    for (name, at) in [
        ("seek-middle", dur / 2.0),
        ("seek-near-end", (dur - 4.0).max(0.0)),
    ] {
        let t = Instant::now();
        let mut b = open(&s, &s.url).map_err(|e| anyhow!("reopen: {e}"))?;
        match b.seek_ms((at * 1000.0) as u64) {
            Ok(()) => {
                let seek_took = t.elapsed();
                match decode_all(&mut b) {
                    Ok(st) => {
                        let left = st.frames as f64 / f64::from(rate);
                        verdict(
                            within(left, dur - at, 2.5) && st.bad == 0,
                            name,
                            format!(
                                "sought to {at:.1}s, {left:.1}s left (expected {:.1}s), seek+open took {}",
                                dur - at,
                                secs(seek_took)
                            ),
                        );
                    }
                    Err(e) => emit("FAIL", name, format!("decode after seek: {e}")),
                }
            }
            Err(e) => emit("FAIL", name, format!("seek failed: {e}")),
        }
    }

    // The fallback used when a format cannot seek in place must land in the same place.
    let mut c = open(&s, &s.url)?;
    c.skip_ms((dur / 2.0 * 1000.0) as u64);
    match decode_all(&mut c) {
        Ok(st) => {
            let left = st.frames as f64 / f64::from(rate);
            verdict(
                within(left, dur / 2.0, 2.5),
                "skip-fallback-matches-seek",
                format!("{left:.1}s left after decoding through {:.1}s", dur / 2.0),
            );
        }
        Err(e) => emit("FAIL", "skip-fallback-matches-seek", e.to_string()),
    }

    // A rejected URL must be reported as such, and re-resolving must fix it.
    match break_url(&s.url) {
        None => emit(
            "WARN",
            "rejected-url",
            "URL has no signature/expiry to corrupt",
        ),
        Some(bad) => match open(&s, &bad) {
            Err(AudioError::UrlExpired(code)) => {
                emit(
                    "PASS",
                    "rejected-url-is-url-expired",
                    format!("HTTP {code} mapped to UrlExpired"),
                );
                let again = src
                    .resolve_stream(&track.key)
                    .map_err(|e| anyhow!("re-resolve: {e}"))?;
                match open(&again, &again.url) {
                    Ok(_) => emit("PASS", "re-resolve-after-rejection", "fresh URL opened"),
                    Err(e) => emit("FAIL", "re-resolve-after-rejection", e.to_string()),
                }
            }
            Err(e) => emit(
                "WARN",
                "rejected-url-is-url-expired",
                format!("got {e} instead of UrlExpired"),
            ),
            Ok(_) => emit(
                "WARN",
                "rejected-url-is-url-expired",
                "the corrupted URL still opened; cannot provoke a 403 this way",
            ),
        },
    }

    gapless(o, &src, &track, &s, dur)
}

fn gapless(
    o: &Opts,
    src: &YtDlpSource,
    first: &TrackInfo,
    s1: &StreamInfo,
    dur1: f64,
) -> Result<()> {
    const RATE: u32 = 48_000;
    let long = |t: &TrackInfo| t.duration_ms.is_some_and(|d| d > 8 * 60_000);
    let second = src
        .related(&first.key, 5)
        .ok()
        .and_then(|v| v.into_iter().find(|t| t.key != first.key && !long(t)))
        .map_or_else(|| first_track(src, &o.q2), Ok)?;
    let s2 = src
        .resolve_stream(&second.key)
        .map_err(|e| anyhow!("resolve second: {e}"))?;
    let dur2 = s2
        .meta
        .duration_secs
        .or(second.duration_ms.map(|m| m as f64 / 1000.0))
        .unwrap_or(0.0);
    emit(
        "INFO",
        "second-track",
        format!(
            "{:?}, {dur2:.1}s, {} / {:?}",
            second.title, s2.ext, s2.acodec
        ),
    );
    if dur1 > 480.0 || dur2 > 480.0 {
        emit(
            "WARN",
            "gapless",
            "a track is over 8 minutes; skipped to keep memory low",
        );
        return Ok(());
    }
    let mut g = GaplessSource::new();
    g.push(PreparedTrack::new(1, Box::new(open(s1, &s1.url)?), RATE)?);
    g.push(PreparedTrack::new(2, Box::new(open(&s2, &s2.url)?), RATE)?);
    let (mut all, mut buf, mut events) = (Vec::new(), vec![0f32; 8192], Vec::new());
    loop {
        let n = g.fill(&mut buf);
        all.extend_from_slice(&buf[..n]);
        events.extend(g.drain_events());
        if n == 0 {
            break;
        }
    }
    events.extend(g.drain_events());
    let failed: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::Failed { .. }))
        .collect();
    verdict(
        failed.is_empty(),
        "gapless-no-failures",
        format!("{} events, {} failed", events.len(), failed.len()),
    );
    let ends1 = events.iter().find_map(|e| match e {
        Event::Finished {
            id: 1,
            at_frame,
            frames_emitted,
        } => Some((*at_frame, *frames_emitted)),
        _ => None,
    });
    let starts2 = events.iter().find_map(|e| match e {
        Event::Started { id: 2, at_frame } => Some(*at_frame),
        _ => None,
    });
    match (ends1, starts2) {
        (Some((end, emitted)), Some(start)) => {
            verdict(
                end == start,
                "gapless-join-contiguous",
                format!("track 1 ends at frame {end}, track 2 starts at {start}"),
            );
            verdict(
                within(emitted as f64 / f64::from(RATE), dur1, 2.5),
                "gapless-track1-length",
                format!(
                    "{:.1}s emitted of {dur1:.1}s",
                    emitted as f64 / f64::from(RATE)
                ),
            );
            let near = find_gaps(&all, 2, 0.001, 48)
                .into_iter()
                .filter(|gap| {
                    gap.start_frame < end as usize + 4800
                        && gap.start_frame + gap.len_frames + 4800 > end as usize
                })
                .map(|gap| gap.len_frames)
                .max();
            emit(
                "INFO",
                "silence-at-join",
                match near {
                    None => "none: audio runs straight through the join".to_string(),
                    Some(n) => format!("{:.0} ms of digital silence around the join (usually the recordings' own fade-out/in, not the player)", n as f64 / 48.0),
                },
            );
        }
        _ => emit(
            "FAIL",
            "gapless-join-contiguous",
            format!("missing events: {events:?}"),
        ),
    }
    let total = all.len() as f64 / 2.0 / f64::from(RATE);
    verdict(
        within(total, dur1 + dur2, 5.0),
        "gapless-total-length",
        format!("{total:.1}s rendered of {:.1}s expected", dur1 + dur2),
    );
    Ok(())
}

// ---------------------------------------------------------------- stress

fn stress(o: &Opts) -> Result<()> {
    let src = source(o);
    let items = src
        .search(&o.q1, SearchFilter::Songs)
        .map_err(|e| anyhow!("setup search: {e}"))?;
    let keys: Vec<String> = items
        .iter()
        .filter_map(|i| match i {
            SearchItem::Track(t) => Some(t.key.clone()),
            _ => None,
        })
        .take(3)
        .collect();
    if keys.len() < 3 {
        bail!("need 3 tracks for the stress test, got {}", keys.len());
    }
    emit(
        "INFO",
        "stress",
        "9 simultaneous calls (4 search, 3 resolve, 2 related) through one source",
    );
    let t0 = Instant::now();
    let results: Vec<(String, Duration, Result<(), SourceError>)> = std::thread::scope(|sc| {
        let mut hs = Vec::new();
        let src = &src;
        for (i, (name, filter)) in [
            ("search-all", SearchFilter::All),
            ("search-songs", SearchFilter::Songs),
            ("search-videos", SearchFilter::Videos),
            ("search-playlists", SearchFilter::Playlists),
        ]
        .into_iter()
        .enumerate()
        {
            let q = if i % 2 == 0 {
                o.q1.clone()
            } else {
                o.q2.clone()
            };
            hs.push(sc.spawn(move || {
                let t = Instant::now();
                let r = src.search(&q, filter).map(|_| ());
                (name.to_string(), t.elapsed(), r)
            }));
        }
        for (i, k) in keys.iter().enumerate() {
            hs.push(sc.spawn(move || {
                let t = Instant::now();
                let r = src.resolve_stream(k).map(|_| ());
                (format!("resolve-{i}"), t.elapsed(), r)
            }));
        }
        for (i, k) in keys.iter().take(2).enumerate() {
            hs.push(sc.spawn(move || {
                let t = Instant::now();
                let r = src.related(k, 5).map(|_| ());
                (format!("related-{i}"), t.elapsed(), r)
            }));
        }
        hs.into_iter()
            .map(|h| h.join().expect("worker panicked"))
            .collect()
    });
    let wall = t0.elapsed();
    let mut limited = 0;
    for (name, took, r) in &results {
        match r {
            Ok(()) => emit("INFO", name, format!("ok after {}", secs(*took))),
            Err(e) => {
                limited += usize::from(*e == SourceError::RateLimited);
                emit("WARN", name, format!("{e} after {}", secs(*took)));
            }
        }
    }
    emit(
        "INFO",
        "stress-wall-time",
        format!("{} for {} calls", secs(wall), results.len()),
    );
    verdict(
        limited == 0,
        "stress-no-rate-limit",
        format!("{limited} of {} calls were rate limited", results.len()),
    );
    let failed = results.iter().filter(|(_, _, r)| r.is_err()).count();
    verdict(
        failed == 0,
        "stress-all-calls-succeeded",
        format!("{failed} failed"),
    );
    Ok(())
}

// ---------------------------------------------------------------- fake429

/// A stand-in yt-dlp that always answers HTTP 429, to prove the cooldown on this machine without
/// provoking the real service. (The same tests in the suite are unix-only, so they do not run on Windows.)
fn fake429(_o: &Opts) -> Result<()> {
    let dir = std::env::temp_dir().join(format!("pyxee-fake429-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let log = dir.join("calls.log");
    #[cfg(windows)]
    let script = {
        let p = dir.join("fake-yt-dlp.cmd");
        std::fs::write(
            &p,
            format!("@echo off\r\necho call>> \"{}\"\r\necho ERROR: HTTP Error 429: Too Many Requests 1>&2\r\nexit /b 1\r\n", log.display()),
        )?;
        p
    };
    #[cfg(not(windows))]
    let script = {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("fake-yt-dlp.sh");
        std::fs::write(&p, format!("#!/bin/sh\necho call >> '{}'\necho 'ERROR: HTTP Error 429: Too Many Requests' >&2\nexit 1\n", log.display()))?;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))?;
        p
    };
    let calls = || {
        std::fs::read_to_string(&log)
            .map_or(0, |t| t.lines().filter(|l| !l.trim().is_empty()).count())
    };

    let s = YtDlpSource::new(&script);
    let first = s.search("a", SearchFilter::All);
    verdict(
        first == Err(SourceError::RateLimited),
        "fake429-classified",
        format!("first call -> {first:?}"),
    );
    let t = Instant::now();
    let later = [
        s.search("b", SearchFilter::Songs).map(|_| ()),
        s.resolve_stream("yt:abcdefghijk").map(|_| ()),
        s.related("yt:abcdefghijk", 3).map(|_| ()),
    ];
    let all_limited = later.iter().all(|r| *r == Err(SourceError::RateLimited));
    verdict(
        all_limited && t.elapsed() < Duration::from_secs(1) && calls() == 1,
        "fake429-cooldown-refuses-fast",
        format!(
            "3 later calls -> {later:?} in {}; tool started {} time(s) in total",
            secs(t.elapsed()),
            calls()
        ),
    );

    let short = GateConfig {
        min_gap: Duration::ZERO,
        cooldown: Duration::from_millis(300),
    };
    let s2 = YtDlpSource::new(&script).with_gate(short);
    let before = calls();
    let _ = s2.search("a", SearchFilter::All);
    std::thread::sleep(Duration::from_millis(450));
    let _ = s2.search("a", SearchFilter::All);
    verdict(
        calls() == before + 2,
        "fake429-tool-used-again-after-cooldown",
        format!("{} starts for 2 calls 450 ms apart", calls() - before),
    );
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let mut o = Opts {
        ytdlp: "yt-dlp".into(),
        q1: "Kevin MacLeod Sneaky Snitch".into(),
        q2: "Kevin MacLeod Monkeys Spinning Monkeys".into(),
    };
    while let Some(a) = args.next() {
        let v = args.next().unwrap_or_default();
        match a.as_str() {
            "--ytdlp" => o.ytdlp = v.into(),
            "--q1" => o.q1 = v,
            "--q2" => o.q2 = v,
            _ => {}
        }
    }
    let _ = log::set_logger(&Stdout).map(|()| log::set_max_level(log::LevelFilter::Info));
    let r = match cmd.as_str() {
        "formats" => formats(&o),
        "search" => search(&o),
        "stream" => stream(&o),
        "stress" => stress(&o),
        "fake429" => fake429(&o),
        _ => Err(anyhow!("usage: livecheck <formats|search|stream|stress|fake429> [--ytdlp PATH] [--q1 Q] [--q2 Q]")),
    };
    if let Err(e) = r {
        emit("FAIL", &cmd, format!("{e:#}"));
    }
    std::process::exit(i32::from(FAILS.load(Ordering::SeqCst) > 0));
}
