//! The real engine: turns the player's `Effect`s into resolving, decoding and playing, and turns what
//! the pipeline reports back into `EngineEvent`s. It owns no thread: the caller drives it with
//! `handle` (an effect arrived) and `poll` (time passed), which also makes it testable step by step.
//!
//! Heavy work (resolve, open, prefill) runs on short-lived worker threads and comes back through a
//! channel tagged with a generation, so results for a track the user already skipped are dropped.

use pyxee_audio::decode::{FfmpegPipeSource, SampleSource, SymphoniaSource};
use pyxee_audio::gapless::{FailKind, PreparedTrack};
use pyxee_audio::http::HttpRangeSource;
use pyxee_audio::pipeline::{Consumer, PipeEvent, Pipeline, Volume};
use pyxee_audio::AudioError;
use pyxee_core::{Command, Effect, EngineEvent, EntryId, Failure, Heard, Track};
use pyxee_sources::{MusicSource, SourceError, StreamInfo, TrackInfo};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const POSITION_EVERY: Duration = Duration::from_millis(250);
const URL_TTL: Duration = Duration::from_secs(15 * 60);
const RECOVER_AFTER: Duration = Duration::from_millis(600);

/// Where the engine reports to. The app forwards these to the `Player`.
pub trait Host: Send + Sync + 'static {
    fn engine_event(&self, e: EngineEvent);
    fn command(&self, c: Command);
    /// Something the listener should be told that is not part of the player state (a song was skipped,
    /// a stream was cut off). Shown as a message in the UI.
    fn notice(&self, _message: String) {}
}

fn clock(ms: u64) -> String {
    format!("{}:{:02}", ms / 60_000, ms / 1000 % 60)
}

/// Why a song cannot be played, in words for the listener.
pub fn skip_reason(f: &Failure) -> String {
    match f {
        Failure::Unavailable => {
            "it isn't available (it may be private, removed or blocked in your region)".into()
        }
        Failure::Decode => "its audio could not be read".into(),
        Failure::Expired => "its stream expired and could not be refreshed".into(),
        Failure::Offline => "you appear to be offline".into(),
        Failure::RateLimited => "the music service is limiting requests".into(),
        Failure::Other(m) => m.clone(),
    }
}

/// `upcoming`: the song has not been reached yet (a prefetch failed), so it *will* be skipped.
pub fn skipped_notice(title: &str, f: &Failure, upcoming: bool) -> String {
    let when = if upcoming {
        "it will be skipped"
    } else {
        "it was skipped"
    };
    format!(
        "\u{201c}{title}\u{201d} can't be played, so {when}: {}. The log has details.",
        skip_reason(f)
    )
}

pub fn early_end_notice(title: &str, heard_ms: u64, expected_ms: u64) -> String {
    format!(
        "\u{201c}{title}\u{201d} stopped early, at {} of {}. The stream was cut off. The log has details.",
        clock(heard_ms),
        clock(expected_ms)
    )
}

/// A song that ends well before its known length (and not because of a seek) was cut off.
pub fn ended_early(heard_ms: u64, expected_ms: Option<u64>) -> bool {
    expected_ms.is_some_and(|e| e > 15_000 && heard_ms * 10 < e * 6 && e - heard_ms > 10_000)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Start,
    Prefetch,
}

enum Work {
    Prepared {
        generation: u64,
        token: u64,
        id: EntryId,
        purpose: Purpose,
        result: Result<Box<PreparedTrack>, Failure>,
    },
    Radio(Result<Vec<TrackInfo>, SourceError>),
}

type UrlCache = Arc<Mutex<HashMap<String, (StreamInfo, Instant)>>>;

pub fn failure_from_source(e: &SourceError) -> Failure {
    match e {
        SourceError::Offline => Failure::Offline,
        SourceError::RateLimited => Failure::RateLimited,
        SourceError::Unavailable(_) => Failure::Unavailable,
        other => Failure::Other(other.to_string()),
    }
}

pub fn failure_from_audio(e: &AudioError) -> Failure {
    match e {
        AudioError::UrlExpired(_) => Failure::Expired,
        AudioError::Network(_) => Failure::Offline,
        AudioError::Ffmpeg(m) => Failure::Other(m.clone()),
        _ => Failure::Decode,
    }
}

pub struct Engine {
    host: Arc<dyn Host>,
    source: Arc<dyn MusicSource>,
    ffmpeg: PathBuf,
    rate: u32,
    pipeline: Pipeline,
    pipe_rx: Receiver<PipeEvent>,
    work_tx: Sender<Work>,
    work_rx: Receiver<Work>,
    cache: UrlCache,
    /// Bumped by anything that flushes the pipeline; older worker results are stale.
    generation: u64,
    /// Bumped when the wanted prefetch changes or is cancelled.
    prefetch_token: u64,
    start_id: Option<EntryId>,
    /// The track asked for by the latest StartTrack/Seek is in the pipeline (nothing may be queued before it).
    start_queued: bool,
    /// A prefetched track that finished before the start track did; queued right after it.
    held_prefetch: Option<Box<PreparedTrack>>,
    /// The song the listener is (logically) on: updated when a track becomes audible, so a seek always
    /// re-opens *this* song, not the one the session originally started with.
    current: Option<(EntryId, Track)>,
    /// Every track the engine was told about in this session (for titles and lengths in logs and notices).
    known: HashMap<EntryId, Track>,
    /// Last heard position of each track, in ms from its start (including a seek offset).
    last_heard_ms: HashMap<EntryId, u64>,
    prefetch_req: Option<(EntryId, String)>,
    offsets: HashMap<EntryId, u64>,
    audible: HashSet<EntryId>,
    /// Tracks that died after being heard: (why, frames played before it died).
    failed_later: HashMap<EntryId, (FailKind, u64)>,
    suppress_heard: bool,
    skip_audible_for: Option<EntryId>,
    paused: bool,
    last_position: Instant,
    seen_underruns: u64,
    stalled_since: Option<(EntryId, Instant)>,
}

impl Engine {
    pub fn new(
        host: Arc<dyn Host>,
        source: Arc<dyn MusicSource>,
        ffmpeg: PathBuf,
        rate: u32,
        volume: Volume,
    ) -> Self {
        let (pipe_tx, pipe_rx) = channel();
        let (work_tx, work_rx) = channel();
        Self {
            host,
            source,
            ffmpeg,
            rate,
            pipeline: Pipeline::new(rate, pipe_tx, volume),
            pipe_rx,
            work_tx,
            work_rx,
            cache: Arc::new(Mutex::new(HashMap::new())),
            generation: 0,
            prefetch_token: 0,
            start_id: None,
            start_queued: false,
            held_prefetch: None,
            current: None,
            known: HashMap::new(),
            last_heard_ms: HashMap::new(),
            prefetch_req: None,
            offsets: HashMap::new(),
            audible: HashSet::new(),
            failed_later: HashMap::new(),
            suppress_heard: false,
            skip_audible_for: None,
            paused: false,
            last_position: Instant::now(),
            seen_underruns: 0,
            stalled_since: None,
        }
    }

    /// Hand this to the output device.
    pub fn consumer(&self) -> Consumer {
        self.pipeline.consumer()
    }

    pub fn handle(&mut self, effect: Effect) {
        match effect {
            Effect::StartTrack {
                id,
                track,
                start_ms,
            } => {
                log::info!(
                    "start: \"{}\" ({}) at {} ms",
                    track.title,
                    track.key,
                    start_ms
                );
                self.known.clear();
                self.last_heard_ms.clear();
                self.start(id, &track, start_ms, false);
            }
            Effect::Prefetch { id, track } => {
                log::info!("prefetch: \"{}\" ({})", track.title, track.key);
                self.prefetch_req = Some((id, track.key.clone()));
                self.known.insert(id, track.clone());
                self.spawn_prefetch(id, track.key);
            }
            Effect::CancelPrefetch => {
                self.prefetch_req = None;
                self.prefetch_token += 1;
                self.held_prefetch = None;
                self.pipeline.clear_queued();
            }
            Effect::Pause => {
                self.paused = true;
                self.pipeline.set_paused(true);
            }
            Effect::Resume => {
                self.paused = false;
                self.pipeline.set_paused(false);
            }
            Effect::Stop => {
                log::info!("stop");
                self.generation += 1;
                self.current = None;
                self.known.clear();
                self.prefetch_req = None;
                self.start_id = None;
                self.held_prefetch = None;
                self.pipeline.flush();
                self.pipeline.set_paused(false);
                self.paused = false;
            }
            Effect::Seek { ms } => {
                // Re-open the song that is playing now at the offset. Its "audible again" is not a new track.
                if let Some((id, track)) = self.current.clone() {
                    log::info!("seek: \"{}\" ({}) to {} ms", track.title, track.key, ms);
                    self.start(id, &track, ms, true);
                    if let Some((pid, pkey)) = self.prefetch_req.clone() {
                        self.spawn_prefetch(pid, pkey);
                    }
                } else {
                    log::warn!("seek to {ms} ms ignored: no song is playing");
                }
            }
            Effect::NeedRadio { seed, count } => {
                let (source, tx) = (Arc::clone(&self.source), self.work_tx.clone());
                thread::spawn(move || {
                    let _ = tx.send(Work::Radio(source.related(&seed.key, count)));
                });
            }
            Effect::Persist => {}
        }
    }

    fn start(&mut self, id: EntryId, track: &Track, start_ms: u64, is_seek: bool) {
        self.generation += 1;
        self.pipeline.flush();
        self.pipeline.set_paused(self.paused && is_seek);
        self.suppress_heard = false;
        self.skip_audible_for = is_seek.then_some(id);
        self.offsets.clear();
        self.offsets.insert(id, start_ms);
        self.audible.clear();
        self.failed_later.clear();
        self.stalled_since = None;
        self.start_id = Some(id);
        self.start_queued = false;
        self.held_prefetch = None;
        self.known.insert(id, track.clone());
        self.current = Some((id, track.clone()));
        if !is_seek {
            self.prefetch_req = None;
        }
        self.spawn_prepare(id, track.key.clone(), start_ms, Purpose::Start);
    }

    fn spawn_prefetch(&mut self, id: EntryId, key: String) {
        self.prefetch_token += 1;
        self.offsets.insert(id, 0);
        self.spawn_prepare(id, key, 0, Purpose::Prefetch);
    }

    fn spawn_prepare(&self, id: EntryId, key: String, start_ms: u64, purpose: Purpose) {
        let (source, cache, ffmpeg, rate, tx) = (
            Arc::clone(&self.source),
            Arc::clone(&self.cache),
            self.ffmpeg.clone(),
            self.rate,
            self.work_tx.clone(),
        );
        let (generation, token) = (self.generation, self.prefetch_token);
        thread::spawn(move || {
            let result = prepare(&*source, &cache, &ffmpeg, rate, id, &key, start_ms).map(Box::new);
            let _ = tx.send(Work::Prepared {
                generation,
                token,
                id,
                purpose,
                result,
            });
        });
    }

    /// Call often (every ~20 ms): delivers finished work and pipeline events, and the position clock.
    pub fn poll(&mut self) {
        while let Ok(w) = self.work_rx.try_recv() {
            self.on_work(w);
        }
        while let Ok(e) = self.pipe_rx.try_recv() {
            self.on_pipe(e);
        }
        self.tick();
    }

    fn on_work(&mut self, w: Work) {
        match w {
            Work::Prepared {
                generation,
                token,
                id,
                purpose,
                result,
            } => {
                if generation != self.generation
                    || (purpose == Purpose::Prefetch && token != self.prefetch_token)
                {
                    return; // the user moved on
                }
                match (purpose, result) {
                    (Purpose::Start, Ok(t)) => {
                        log::info!("ready: {}", self.describe(id));
                        self.pipeline.push(*t);
                        self.start_queued = true;
                        // The next track may have been ready first (a seek re-opens the current track,
                        // which can be slow); it follows, never leads.
                        if let Some(next) = self.held_prefetch.take() {
                            self.pipeline.push(*next);
                        }
                    }
                    (Purpose::Prefetch, Ok(t)) => {
                        if self.start_queued || self.start_id.is_none() {
                            self.pipeline.push(*t);
                        } else {
                            self.held_prefetch = Some(t);
                        }
                        self.host.engine_event(EngineEvent::Prefetched { id });
                    }
                    (Purpose::Start, Err(failure)) => {
                        log::warn!("could not start {}: {failure:?}", self.describe(id));
                        self.held_prefetch = None;
                        self.notify_skip(id, &failure, false);
                        self.host
                            .engine_event(EngineEvent::LoadFailed { id, failure });
                    }
                    (Purpose::Prefetch, Err(failure)) => {
                        log::warn!("could not prefetch {}: {failure:?}", self.describe(id));
                        self.notify_skip(id, &failure, true);
                        self.host
                            .engine_event(EngineEvent::PrefetchFailed { id, failure });
                    }
                }
            }
            Work::Radio(result) => {
                let tracks = result
                    .unwrap_or_default()
                    .into_iter()
                    .map(|t| Track {
                        key: t.key,
                        title: t.title,
                        artist: t.artist,
                        duration_ms: t.duration_ms,
                        thumbnail: t.thumbnail,
                    })
                    .collect();
                self.host.command(Command::AppendRadio(tracks));
            }
        }
    }

    fn on_pipe(&mut self, e: PipeEvent) {
        match e {
            PipeEvent::Failed {
                id,
                kind,
                message,
                frames_emitted,
            } => {
                log::warn!(
                    "{} stopped producing audio after {} ms: {kind:?}: {message}",
                    self.describe(id),
                    frames_emitted * 1000 / u64::from(self.rate)
                );
                if self.audible.contains(&id) {
                    // Still playing what was buffered before the failure; report it when it is heard.
                    self.failed_later.insert(id, (kind, frames_emitted));
                } else if self.start_id == Some(id) {
                    let failure = failure_for(kind);
                    self.notify_skip(id, &failure, false);
                    self.host
                        .engine_event(EngineEvent::LoadFailed { id, failure });
                } else {
                    let failure = failure_for(kind);
                    self.notify_skip(id, &failure, true);
                    self.host
                        .engine_event(EngineEvent::PrefetchFailed { id, failure });
                }
            }
            PipeEvent::Heard(_) if self.suppress_heard => {}
            PipeEvent::Heard(Heard::Audible(id)) => {
                self.audible.insert(id);
                self.stalled_since = None;
                log::info!("audible: {}", self.describe(id));
                if let Some(t) = self.known.get(&id) {
                    self.current = Some((id, t.clone()));
                }
                if self.skip_audible_for == Some(id) {
                    self.skip_audible_for = None; // same track, resumed after a seek
                } else {
                    self.host.engine_event(EngineEvent::Audible { id });
                }
            }
            PipeEvent::Heard(Heard::Ended(id)) => match self.failed_later.remove(&id) {
                Some((FailKind::Expired(_), frames)) => {
                    // Resume exactly where it stopped, whatever the position clock last reported.
                    let ms = self.offsets.get(&id).copied().unwrap_or(0)
                        + frames * 1000 / u64::from(self.rate);
                    self.host.engine_event(EngineEvent::Position { id, ms });
                    // Tracks queued behind it must not be announced while it is re-resolved.
                    self.suppress_heard = true;
                    self.host.engine_event(EngineEvent::UrlExpired { id });
                }
                failed => {
                    let heard = self.last_heard_ms.get(&id).copied().unwrap_or(0);
                    let expected = self.known.get(&id).and_then(|t| t.duration_ms);
                    log::info!(
                        "ended: {} after {} ms (expected {:?} ms)",
                        self.describe(id),
                        heard,
                        expected
                    );
                    if let (Some((kind, _)), Some(t)) = (failed, self.known.get(&id)) {
                        self.host.notice(format!(
                            "\u{201c}{}\u{201d} stopped early because of a playback error ({kind:?}). The log has details.",
                            t.title
                        ));
                    } else if ended_early(heard, expected) {
                        let title = self.known.get(&id).map_or("A song", |t| t.title.as_str());
                        log::warn!(
                            "{} was cut off: heard {heard} ms of {expected:?} ms",
                            self.describe(id)
                        );
                        self.host
                            .notice(early_end_notice(title, heard, expected.unwrap_or(0)));
                    }
                    self.host.engine_event(EngineEvent::Ended { id });
                }
            },
        }
    }

    /// `"Title" (key)` for log lines.
    fn describe(&self, id: EntryId) -> String {
        match self.known.get(&id) {
            Some(t) => format!("\"{}\" ({})", t.title, t.key),
            None => format!("track {}", id.0),
        }
    }

    /// Tell the listener when a song is skipped for a reason that will not go away by itself.
    fn notify_skip(&self, id: EntryId, failure: &Failure, upcoming: bool) {
        if failure.is_transient() {
            return; // the player bar already shows "offline" / "rate limited" with a Retry
        }
        let title = self
            .known
            .get(&id)
            .map_or_else(|| "A song".to_string(), |t| t.title.clone());
        self.host.notice(skipped_notice(&title, failure, upcoming));
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let underruns = self.pipeline.underruns();
        if let Some((id, ms)) = self.pipeline.heard_position() {
            self.last_heard_ms
                .insert(id, self.offsets.get(&id).copied().unwrap_or(0) + ms);
            if !self.paused && underruns > self.seen_underruns && self.stalled_since.is_none() {
                self.stalled_since = Some((id, now));
                self.host.engine_event(EngineEvent::Stalled { id });
            }
            if let Some((sid, since)) = self.stalled_since {
                if underruns == self.seen_underruns && now.duration_since(since) > RECOVER_AFTER {
                    self.stalled_since = None;
                    self.host.engine_event(EngineEvent::Recovered { id: sid });
                }
            }
            if now.duration_since(self.last_position) >= POSITION_EVERY && !self.suppress_heard {
                self.last_position = now;
                let offset = self.offsets.get(&id).copied().unwrap_or(0);
                self.host.engine_event(EngineEvent::Position {
                    id,
                    ms: offset + ms,
                });
            }
        }
        if underruns != self.seen_underruns {
            self.seen_underruns = underruns;
            if let Some((id, _)) = self.stalled_since {
                self.stalled_since = Some((id, now)); // still starving: restart the recovery timer
            }
        }
    }
}

fn failure_for(kind: FailKind) -> Failure {
    match kind {
        FailKind::Expired(_) => Failure::Expired,
        FailKind::Network => Failure::Offline,
        FailKind::Decode | FailKind::Empty => Failure::Decode,
    }
}

/// Where a stream lives, for logs (the full URL carries a token and is long).
fn host_of(url: &str) -> &str {
    if url.starts_with("http") {
        url.split('/').nth(2).unwrap_or("?")
    } else {
        "a local file"
    }
}

fn resolve_cached(
    source: &dyn MusicSource,
    cache: &UrlCache,
    key: &str,
    refresh: bool,
) -> Result<StreamInfo, SourceError> {
    if !refresh {
        if let Some((s, at)) = cache.lock().ok().and_then(|c| c.get(key).cloned()) {
            if at.elapsed() < URL_TTL {
                return Ok(s);
            }
        }
    }
    let s = source.resolve_stream(key)?;
    log::info!(
        "resolved {key}: ext={} acodec={:?} abr={:?} from {}",
        s.ext,
        s.acodec,
        s.abr_kbps,
        host_of(&s.url)
    );
    if let Ok(mut c) = cache.lock() {
        c.insert(key.to_string(), (s.clone(), Instant::now()));
    }
    Ok(s)
}

/// Opus goes through ffmpeg (symphonia has no Opus decoder); everything else through symphonia,
/// with ffmpeg as the fallback for formats it cannot read.
fn open_stream(
    s: &StreamInfo,
    start_ms: u64,
    ffmpeg: &std::path::Path,
) -> Result<Box<dyn SampleSource>, AudioError> {
    let remote = s.url.starts_with("http");
    let opus = s.acodec.as_deref().is_some_and(|c| c.contains("opus"))
        || matches!(s.ext.as_str(), "webm" | "opus");
    if !opus {
        let opened = if remote {
            HttpRangeSource::open(&s.url)
                .and_then(|h| SymphoniaSource::open(Box::new(h), Some(&s.ext)))
        } else {
            SymphoniaSource::open_path(std::path::Path::new(&s.url))
        };
        match opened {
            Ok(mut src) => {
                if start_ms == 0 {
                    return Ok(Box::new(src));
                }
                match src.seek_ms(start_ms) {
                    Ok(()) => return Ok(Box::new(src)),
                    Err(e @ (AudioError::UrlExpired(_) | AudioError::Network(_))) => return Err(e),
                    // The format cannot seek (no index): let ffmpeg do it rather than give up on the song.
                    Err(e) => {
                        log::warn!("seeking to {start_ms} ms failed ({e}); using ffmpeg instead")
                    }
                }
            }
            Err(e @ (AudioError::UrlExpired(_) | AudioError::Network(_))) => return Err(e),
            Err(e) => log::warn!(
                "the built-in decoder cannot read this stream ({e}); using ffmpeg instead"
            ),
        }
    } else {
        log::info!("opus audio: decoding with ffmpeg");
    }
    Ok(Box::new(FfmpegPipeSource::spawn_at(
        ffmpeg, &s.url, start_ms,
    )?))
}

fn prepare(
    source: &dyn MusicSource,
    cache: &UrlCache,
    ffmpeg: &std::path::Path,
    rate: u32,
    id: EntryId,
    key: &str,
    start_ms: u64,
) -> Result<PreparedTrack, Failure> {
    let build = |stream: &StreamInfo| -> Result<PreparedTrack, AudioError> {
        PreparedTrack::new(id.0, open_stream(stream, start_ms, ffmpeg)?, rate)
    };
    let stream = resolve_cached(source, cache, key, false).map_err(|e| failure_from_source(&e))?;
    match build(&stream) {
        Err(AudioError::UrlExpired(_)) => {
            // The cached URL went stale: resolve again once.
            let fresh =
                resolve_cached(source, cache, key, true).map_err(|e| failure_from_source(&e))?;
            build(&fresh).map_err(|e| failure_from_audio(&e))
        }
        other => other.map_err(|e| failure_from_audio(&e)),
    }
}
