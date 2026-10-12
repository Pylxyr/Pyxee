//! The real engine, pipeline and player wired together, with local fixtures standing in for the network
//! and a test "device" that pulls audio. Proves what the user would hear: order, gaplessness, skipping,
//! seeking, failures, radio, and recovery from an expired stream URL.

use crate::engine::{Engine, Host};
use pyxee_audio::gap::find_gaps;
use pyxee_audio::pipeline::{Consumer, Volume};
use pyxee_core::{Command, Effect, EngineEvent, Origin, Player, Repeat, Track, Transport};
use pyxee_sources::local::LocalSource;
use pyxee_sources::{MusicSource, SearchFilter, SearchItem, SourceError, StreamInfo, TrackInfo};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../audio/tests/fixtures")
        .join(name)
}

fn file_track(name: &str) -> Track {
    let mut t = Track::new(
        &format!("file:{}", fixture(name).display()),
        name,
        "fixture",
    );
    t.duration_ms = Some(2000);
    t
}

struct Shared {
    notices: Mutex<Vec<String>>,
    player: Mutex<Player>,
    queue: Mutex<VecDeque<Effect>>,
}

struct TestHost(Arc<Shared>);

impl Host for TestHost {
    fn engine_event(&self, e: EngineEvent) {
        let fx = self.0.player.lock().unwrap().engine(e);
        self.0.queue.lock().unwrap().extend(fx);
    }
    fn command(&self, c: Command) {
        let fx = self.0.player.lock().unwrap().command(c);
        self.0.queue.lock().unwrap().extend(fx);
    }
    fn notice(&self, message: String) {
        self.0.notices.lock().unwrap().push(message);
    }
}

type Resolver = Box<dyn Fn(&str) -> Result<StreamInfo, SourceError> + Send + Sync>;

/// Local files for playback, a fixed "related" list for radio, and an overridable resolver.
struct TestSource {
    related: Vec<TrackInfo>,
    resolve: Resolver,
}

impl TestSource {
    fn local() -> Self {
        Self {
            related: Vec::new(),
            resolve: Box::new(|k| LocalSource.resolve_stream(k)),
        }
    }
}

impl MusicSource for TestSource {
    fn search(&self, _q: &str, _f: SearchFilter) -> Result<Vec<SearchItem>, SourceError> {
        Ok(Vec::new())
    }
    fn resolve_stream(&self, key: &str) -> Result<StreamInfo, SourceError> {
        (self.resolve)(key)
    }
    fn related(&self, _key: &str, count: usize) -> Result<Vec<TrackInfo>, SourceError> {
        Ok(self.related.iter().take(count).cloned().collect())
    }
    fn suggest(&self, _p: &str) -> Result<Vec<String>, SourceError> {
        Ok(Vec::new())
    }
    fn supported_filters(&self) -> Vec<SearchFilter> {
        Vec::new()
    }
}

/// The test device consumes audio this many times faster than real time. Pacing it (instead of
/// draining as fast as possible) keeps wall-clock behaviour, like position updates, meaningful.
const DEVICE_SPEED: f64 = 6.0;

struct Rig {
    volume: Volume,
    budget_frames: f64,
    last_step: Instant,
    shared: Arc<Shared>,
    engine: Engine,
    consumer: Consumer,
    samples: Vec<f32>,
    audible_log: Vec<Option<u64>>,
    /// How many samples the device had taken when each `audible_log` entry appeared.
    audible_at_samples: Vec<usize>,
    effects: Vec<Effect>,
}

impl Rig {
    fn new(source: TestSource) -> Self {
        let volume = Volume::default();
        let shared = Arc::new(Shared {
            notices: Mutex::new(Vec::new()),
            player: Mutex::new(Player::new(3)),
            queue: Mutex::new(VecDeque::new()),
        });
        let engine = Engine::new(
            Arc::new(TestHost(shared.clone())),
            Arc::new(source),
            PathBuf::from("ffmpeg"),
            48_000,
            volume.clone(),
        );
        let consumer = engine.consumer();
        Self {
            volume,
            budget_frames: 0.0,
            last_step: Instant::now(),
            shared,
            engine,
            consumer,
            samples: Vec::new(),
            audible_log: Vec::new(),
            audible_at_samples: Vec::new(),
            effects: Vec::new(),
        }
    }

    fn cmd(&mut self, c: Command) {
        let fx = self.shared.player.lock().unwrap().command(c);
        self.shared.queue.lock().unwrap().extend(fx);
    }

    fn snapshot(&self) -> pyxee_core::Snapshot {
        self.shared.player.lock().unwrap().snapshot()
    }

    fn entry_ids(&self) -> Vec<u64> {
        self.shared
            .player
            .lock()
            .unwrap()
            .queue()
            .entries()
            .iter()
            .map(|e| e.id.0)
            .collect()
    }

    /// One tick of the world: deliver effects, let the engine work, and let the "device" take a buffer.
    fn step(&mut self) {
        loop {
            let next = self.shared.queue.lock().unwrap().pop_front();
            let Some(e) = next else { break };
            self.effects.push(e.clone());
            self.engine.handle(e);
        }
        self.engine.poll();
        let now = Instant::now();
        self.budget_frames +=
            now.duration_since(self.last_step).as_secs_f64() * 48_000.0 * DEVICE_SPEED;
        self.last_step = now;
        let frames = (self.budget_frames as usize).min(480);
        let mut buf = [0f32; 960];
        let n = if frames > 0 {
            self.consumer.pull(&mut buf[..frames * 2], 0)
        } else {
            0
        };
        self.budget_frames -= (n / 2) as f64;
        self.budget_frames = self.budget_frames.min(4_800.0); // no catching up after a stall
        self.samples.extend_from_slice(&buf[..n]);
        if n == 0 {
            sleep(Duration::from_millis(1));
        }
        let a = self.snapshot().audible.map(|i| i.0);
        if self.audible_log.last() != Some(&a) {
            self.audible_log.push(a);
            self.audible_at_samples.push(self.samples.len());
        }
    }

    fn run_until(&mut self, secs: u64, cond: impl Fn(&Rig) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            self.step();
            if cond(self) {
                return true;
            }
        }
        false
    }

    /// Lets real time pass (the engine throttles position updates by wall clock).
    fn run_for(&mut self, ms: u64) {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            self.step();
        }
    }

    fn start_tracks(&self) -> usize {
        self.effects
            .iter()
            .filter(|e| matches!(e, Effect::StartTrack { .. }))
            .count()
    }
}

#[test]
fn two_tracks_play_in_order_gaplessly_and_playback_ends() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav"), file_track("b_48k.wav")],
        start: 0,
    });
    assert_eq!(
        r.snapshot().audible,
        None,
        "nothing is audible before audio reaches the device"
    );
    assert!(
        r.run_until(15, |r| r.snapshot().transport == Transport::Idle
            && r.audible_log.len() >= 3)
    );

    let ids = r.entry_ids();
    assert_eq!(r.audible_log, vec![None, Some(ids[0]), Some(ids[1]), None]);
    assert_eq!(
        r.start_tracks(),
        1,
        "the second track continued gaplessly from its prefetch"
    );
    assert_eq!(
        r.samples.len(),
        2 * 2 * 96_000,
        "every frame of both tracks was played"
    );
    assert!(
        find_gaps(&r.samples, 2, 1e-3, 8).is_empty(),
        "silence between or inside tracks"
    );
}

#[test]
fn skipping_stops_the_old_audio_and_the_old_track_never_returns() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    r.cmd(Command::PlayTracks {
        tracks: vec![
            file_track("a_44k.wav"),
            file_track("b_48k.wav"),
            file_track("a_44k.flac"),
        ],
        start: 0,
    });
    assert!(r.run_until(10, |r| r.snapshot().audible.is_some()));
    for _ in 0..30 {
        r.step();
    }
    let before = r.samples.len();
    r.cmd(Command::Next);
    let ids = r.entry_ids();
    let second = Some(ids[1]);
    assert!(r.run_until(10, |r| r.snapshot().audible.map(|i| i.0) == second));
    // The first track was cut off for good: it never becomes audible again.
    assert_eq!(r.audible_log, vec![None, Some(ids[0]), None, Some(ids[1])]);
    assert!(r.samples.len() > before);
}

#[test]
fn pause_freezes_the_position_and_resume_continues() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav")],
        start: 0,
    });
    assert!(r.run_until(10, |r| r.snapshot().position_ms >= 250));
    r.cmd(Command::Pause);
    r.step();
    // The last position update before the pause can be up to 250 ms old; let it settle first.
    r.run_for(400);
    let (at, played) = (r.snapshot().position_ms, r.samples.len());
    r.run_for(500);
    assert_eq!(
        r.snapshot().position_ms,
        at,
        "the position does not move while paused"
    );
    assert_eq!(
        r.samples.len(),
        played,
        "no audio reaches the device while paused"
    );
    r.cmd(Command::Resume);
    assert!(r.run_until(10, |r| r.snapshot().position_ms > at + 200));
}

#[test]
fn seeking_restarts_at_the_offset_and_the_position_follows() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav")],
        start: 0,
    });
    assert!(r.run_until(10, |r| r.snapshot().position_ms >= 250));
    let id = r.entry_ids()[0];
    r.cmd(Command::Seek(1500));
    let before = r.samples.len();
    assert!(r.run_until(10, |r| r.snapshot().transport == Transport::Idle));
    let after_seek = (r.samples.len() - before) / 2;
    assert!(
        (0.45..0.6).contains(&(after_seek as f64 / 48_000.0)),
        "about 0.5 s of audio should remain after seeking to 1.5 s, got {} frames",
        after_seek
    );
    assert_eq!(
        r.audible_log.iter().filter(|a| **a == Some(id)).count(),
        1,
        "a seek is not a new track"
    );
}

#[test]
fn an_unplayable_track_is_skipped_and_the_next_one_plays() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    let missing = Track::new("file:/definitely/missing.mp3", "Missing", "x");
    r.cmd(Command::PlayTracks {
        tracks: vec![missing, file_track("b_48k.wav")],
        start: 0,
    });
    assert!(r.run_until(10, |r| r.snapshot().audible.is_some()));
    let ids = r.entry_ids();
    assert_eq!(r.snapshot().audible.map(|i| i.0), Some(ids[1]));
}

#[test]
fn a_service_that_is_offline_stops_on_the_track_with_a_retry() {
    let src = TestSource {
        related: Vec::new(),
        resolve: Box::new(|_| Err(SourceError::Offline)),
    };
    let mut r = Rig::new(src);
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav"), file_track("b_48k.wav")],
        start: 0,
    });
    assert!(r.run_until(5, |r| matches!(
        r.snapshot().transport,
        Transport::Failed { .. }
    )));
    let ids = r.entry_ids();
    assert!(
        matches!(r.snapshot().transport, Transport::Failed { id, .. } if id.0 == ids[0]),
        "does not burn through the queue"
    );
}

#[test]
fn autoplay_radio_keeps_music_going_after_the_queue_runs_out() {
    let related = vec![
        TrackInfo {
            key: format!("file:{}", fixture("b_48k.wav").display()),
            title: "R1".into(),
            artist: "radio".into(),
            duration_ms: Some(2000),
            thumbnail: None,
        },
        TrackInfo {
            key: format!("file:{}", fixture("b_48k.mp3").display()),
            title: "R2".into(),
            artist: "radio".into(),
            duration_ms: Some(2000),
            thumbnail: None,
        },
    ];
    let mut r = Rig::new(TestSource {
        related,
        ..TestSource::local()
    });
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav")],
        start: 0,
    });
    assert!(r.run_until(20, |r| r.audible_log.iter().flatten().count() >= 3));
    let radio: Vec<_> = {
        let p = r.shared.player.lock().unwrap();
        p.queue()
            .entries()
            .iter()
            .filter(|e| e.origin == Origin::Radio)
            .map(|e| e.id.0)
            .collect()
    };
    assert_eq!(radio.len(), 2);
    let played: Vec<u64> = r.audible_log.iter().flatten().copied().collect();
    assert!(
        played.contains(&radio[0]),
        "the first radio track played: {played:?}"
    );
    assert!(find_gaps(&r.samples, 2, 1e-3, 8).is_empty());
}

#[test]
fn repeat_one_loops_a_track_gaplessly() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav")],
        start: 0,
    });
    r.cmd(Command::SetRepeat(Repeat::One));
    assert!(r.run_until(20, |r| r.samples.len() >= 2 * 2 * 96_000 * 2 + 4800));
    assert_eq!(r.start_tracks(), 1);
    assert!(
        find_gaps(&r.samples, 2, 1e-3, 8).is_empty(),
        "the loop point is seamless"
    );
}

// ---- expired stream URL, mid-play -------------------------------------------------------------

fn make_wav(secs: usize) -> Vec<u8> {
    let frames = 48_000 * secs;
    let data_len = (frames * 4) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&192_000u32.to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let v =
            ((i as f32 / 48_000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5 * 32767.0) as i16;
        b.extend_from_slice(&v.to_le_bytes());
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// Serves `body` with range requests, each answer taking `delay`. Until `healthy` is set, only the first `allow` requests succeed;
/// the rest get 403, like a stream URL that expired part-way through a song.
fn serve(body: Vec<u8>, allow: usize, healthy: Arc<AtomicBool>, delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut req = Vec::new();
            let mut b = [0u8; 1];
            while !req.ends_with(b"\r\n\r\n") {
                if s.read(&mut b).unwrap_or(0) == 0 {
                    break;
                }
                req.push(b[0]);
            }
            sleep(delay); // a slow network
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            if !healthy.load(Ordering::SeqCst) && n > allow {
                let _ = s.write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                continue;
            }
            let text = String::from_utf8_lossy(&req).to_string();
            let range = text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("range: bytes=")
                        .map(str::to_string)
                })
                .unwrap_or_default();
            let (a, e) = range.split_once('-').unwrap_or(("0", "0"));
            let from: usize = a.trim().parse().unwrap_or(0);
            let to: usize = e
                .trim()
                .parse::<usize>()
                .unwrap_or(body.len() - 1)
                .min(body.len() - 1);
            let part = &body[from..=to];
            let head = format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{to}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len(), part.len());
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(part);
        }
    });
    format!("http://{addr}/track.wav")
}

#[test]
fn an_expired_stream_url_mid_song_is_re_resolved_and_playback_continues_where_it_stopped() {
    let healthy = Arc::new(AtomicBool::new(false));
    let url = serve(make_wav(10), 3, healthy.clone(), Duration::ZERO);
    let resolves = Arc::new(AtomicUsize::new(0));
    let (r2, h2) = (resolves.clone(), healthy.clone());
    let src = TestSource {
        related: Vec::new(),
        resolve: Box::new(move |_| {
            if r2.fetch_add(1, Ordering::SeqCst) >= 1 {
                h2.store(true, Ordering::SeqCst); // the second resolve returns a fresh, working URL
            }
            Ok(StreamInfo {
                url: url.clone(),
                ext: "wav".into(),
                acodec: None,
                abr_kbps: None,
                ..Default::default()
            })
        }),
    };
    let mut r = Rig::new(src);
    r.cmd(Command::SetAutoplay(false));
    let mut t = Track::new("yt:expiring", "Long song", "x");
    t.duration_ms = Some(10_000);
    r.cmd(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });

    assert!(
        r.run_until(30, |r| r.snapshot().transport == Transport::Idle
            && r.audible_log.len() >= 3),
        "{:?}",
        r.snapshot()
    );

    assert_eq!(
        resolves.load(Ordering::SeqCst),
        2,
        "resolved again after the URL was rejected"
    );
    let starts: Vec<u64> = r
        .effects
        .iter()
        .filter_map(|e| {
            if let Effect::StartTrack { start_ms, .. } = e {
                Some(*start_ms)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(starts.len(), 2);
    assert!(
        (3_000..8_000).contains(&starts[1]),
        "resumed at the heard position, not from the start: {starts:?}"
    );
    let secs = r.samples.len() as f64 / 2.0 / 48_000.0;
    assert!(
        (9.5..10.6).contains(&secs),
        "the whole song was heard once, {secs:.2} s"
    );
    assert!(find_gaps(&r.samples, 2, 1e-3, 8).is_empty());
}

/// A source whose n-th resolve returns `urls[min(n, last)]`; counts the resolves.
fn resolving_in_turn(urls: Vec<String>, resolves: Arc<AtomicUsize>) -> TestSource {
    TestSource {
        related: Vec::new(),
        resolve: Box::new(move |_| {
            let n = resolves.fetch_add(1, Ordering::SeqCst);
            Ok(StreamInfo {
                url: urls[n.min(urls.len() - 1)].clone(),
                ext: "wav".into(),
                ..Default::default()
            })
        }),
    }
}

fn refusing_server() -> String {
    serve(
        make_wav(2),
        0,
        Arc::new(AtomicBool::new(false)),
        Duration::ZERO,
    )
}

fn working_server() -> String {
    serve(
        make_wav(2),
        usize::MAX,
        Arc::new(AtomicBool::new(true)),
        Duration::ZERO,
    )
}

fn play_one(r: &mut Rig, key: &str) {
    let mut t = Track::new(key, "Song", "x");
    t.duration_ms = Some(2_000);
    r.cmd(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });
}

#[test]
fn a_url_refused_at_once_is_re_resolved_and_only_the_working_url_is_remembered() {
    let resolves = Arc::new(AtomicUsize::new(0));
    let src = resolving_in_turn(vec![refusing_server(), working_server()], resolves.clone());
    let mut r = Rig::new(src);
    r.cmd(Command::SetAutoplay(false));
    play_one(&mut r, "yt:picky");
    assert!(
        r.run_until(20, |r| r.snapshot().transport == Transport::Idle
            && r.samples.len() >= 180_000),
        "{:?}",
        r.snapshot()
    );
    assert_eq!(
        resolves.load(Ordering::SeqCst),
        2,
        "the refused URL was replaced by one fresh resolve"
    );

    // Playing it again must use the remembered (working) URL: no third resolve.
    let heard = r.samples.len();
    play_one(&mut r, "yt:picky");
    assert!(
        r.run_until(20, |r| r.snapshot().transport == Transport::Idle
            && r.samples.len() >= heard + 180_000),
        "{:?}",
        r.snapshot()
    );
    assert_eq!(resolves.load(Ordering::SeqCst), 2, "served from the cache");
}

#[test]
fn when_every_url_is_refused_the_song_fails_with_the_status_and_a_retry_resolves_afresh() {
    let resolves = Arc::new(AtomicUsize::new(0));
    // Resolves 1 and 2 are refused (the first attempt and its single re-resolve); the third works.
    let src = resolving_in_turn(
        vec![refusing_server(), refusing_server(), working_server()],
        resolves.clone(),
    );
    let mut r = Rig::new(src);
    r.cmd(Command::SetAutoplay(false));
    play_one(&mut r, "yt:picky");
    assert!(
        r.run_until(20, |r| matches!(
            r.snapshot().transport,
            Transport::Failed { .. }
        )),
        "{:?}",
        r.snapshot()
    );
    assert_eq!(
        resolves.load(Ordering::SeqCst),
        2,
        "one re-resolve, no loop"
    );
    assert!(
        matches!(
            r.snapshot().transport,
            Transport::Failed {
                failure: pyxee_core::Failure::Expired(Some(403)),
                ..
            }
        ),
        "{:?}",
        r.snapshot().transport
    );
    assert!(
        r.notices()
            .iter()
            .any(|n| n.contains("YouTube refused to send it (HTTP 403)")),
        "{:?}",
        r.notices()
    );

    // Retry must not be handed the refused URL from the cache.
    r.cmd(Command::Retry);
    assert!(
        r.run_until(20, |r| r.snapshot().transport == Transport::Idle
            && r.samples.len() >= 180_000),
        "{:?}",
        r.snapshot()
    );
    assert_eq!(resolves.load(Ordering::SeqCst), 3);
}

#[test]
fn the_streams_own_metadata_fills_in_artist_length_and_cover_in_the_queue() {
    let url = working_server();
    let src = TestSource {
        related: Vec::new(),
        resolve: Box::new(move |_| {
            Ok(StreamInfo {
                url: url.clone(),
                ext: "wav".into(),
                meta: pyxee_sources::StreamMeta {
                    title: None,
                    artist: Some("Lowtide".into()),
                    duration_secs: Some(2.0),
                    thumbnail: Some("https://big/cover.jpg".into()),
                },
                ..Default::default()
            })
        }),
    };
    let mut r = Rig::new(src);
    r.cmd(Command::SetAutoplay(false));
    // What a songs-only search gives: no artist, no length, a small cover.
    let mut t = Track::new("yt:bare", "Neon Harbor", pyxee_core::UNKNOWN_ARTIST);
    t.thumbnail = Some("https://i.ytimg.com/vi/bare/hqdefault.jpg".into());
    r.cmd(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });
    assert!(
        r.run_until(20, |r| r.snapshot().transport == Transport::Idle
            && r.samples.len() >= 180_000)
    );
    let player = r.shared.player.lock().unwrap();
    let entry = &player.queue().entries()[0].track;
    assert_eq!(entry.artist, "Lowtide");
    assert_eq!(entry.duration_ms, Some(2_000));
    assert_eq!(entry.thumbnail.as_deref(), Some("https://big/cover.jpg"));
}

#[test]
fn the_unknown_artist_text_is_the_same_in_core_and_sources() {
    assert_eq!(pyxee_core::UNKNOWN_ARTIST, pyxee_sources::UNKNOWN_ARTIST);
}

#[test]
fn seeking_never_lets_the_next_track_play_first() {
    // Re-opening the current track at the seek offset is slow over a network, while the next track
    // (already known) is prepared quickly. The next track must still wait its turn.
    let url = serve(
        make_wav(10),
        usize::MAX,
        Arc::new(AtomicBool::new(true)),
        Duration::from_millis(250),
    );
    let src = TestSource {
        related: Vec::new(),
        resolve: Box::new(move |key| {
            if key == "yt:slow" {
                Ok(StreamInfo {
                    url: url.clone(),
                    ext: "wav".into(),
                    acodec: None,
                    abr_kbps: None,
                    ..Default::default()
                })
            } else {
                LocalSource.resolve_stream(key)
            }
        }),
    };
    let mut r = Rig::new(src);
    r.cmd(Command::SetAutoplay(false));
    let mut slow = Track::new("yt:slow", "Slow song", "x");
    slow.duration_ms = Some(10_000);
    r.cmd(Command::PlayTracks {
        tracks: vec![slow, file_track("b_48k.wav")],
        start: 0,
    });
    let ids = r.entry_ids();
    assert!(r.run_until(30, |r| r.snapshot().audible.map(|i| i.0) == Some(ids[0])));
    r.run_for(1500); // the next track is prefetched and queued behind it

    let at_seek = r.samples.len();
    r.cmd(Command::Seek(6000));
    r.run_for(4000); // plenty: the 4 s left of the song play at 6x speed, then the next song follows

    // The next song may only start after the rest of this one (4 s of audio) has been heard.
    let i = r
        .audible_log
        .iter()
        .position(|a| *a == Some(ids[1]))
        .expect("the next song follows gaplessly");
    let heard_before_next = (r.audible_at_samples[i] - at_seek) as f64 / 2.0 / 48_000.0;
    assert!(
        heard_before_next >= 3.5,
        "the next song cut in after only {heard_before_next:.2} s of the seeked song: {:?}",
        r.audible_log
    );
}

#[test]
fn the_volume_setting_changes_how_loud_the_music_is() {
    let peak = |volume: u8| {
        let mut r = Rig::new(TestSource::local());
        r.volume.set_percent(volume);
        r.cmd(Command::SetAutoplay(false));
        r.cmd(Command::PlayTracks {
            tracks: vec![file_track("a_44k.wav")],
            start: 0,
        });
        assert!(
            r.run_until(15, |r| r.snapshot().transport == Transport::Idle
                && r.audible_log.len() >= 3)
        );
        r.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    };
    let (full, low, off) = (peak(100), peak(20), peak(0));
    assert!(
        full > 0.03,
        "the fixture tone is quiet but audible, got {full}"
    );
    assert!(
        (low - full * 0.04).abs() < full * 0.01,
        "20% on the slider is a gain of 0.04: {low} vs {full}"
    );
    assert_eq!(off, 0.0);
}

/// Rising zero crossings per second of the left channel: 440 Hz and 660 Hz tones are easy to tell apart.
fn dominant_hz(stereo: &[f32]) -> f64 {
    let left: Vec<f32> = stereo.chunks(2).map(|f| f[0]).collect();
    let rising = left
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    rising as f64 / (left.len() as f64 / 48_000.0)
}

#[test]
fn seeking_after_a_gapless_advance_stays_on_the_song_that_is_playing() {
    // a_44k.wav is a 440 Hz tone, b_48k.wav a 660 Hz tone. After A hands over to B, "seek to the start"
    // must restart B, not re-open A (the track the session originally started with).
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav"), file_track("b_48k.wav")],
        start: 0,
    });
    let ids = r.entry_ids();
    assert!(r.run_until(20, |r| r.snapshot().audible.map(|i| i.0) == Some(ids[1])));
    r.run_for(300);
    assert!(
        (dominant_hz(&r.samples[r.samples.len() - 9600..]) - 660.0).abs() < 50.0,
        "B is playing before the seek"
    );

    let at = r.samples.len();
    r.cmd(Command::Seek(0));
    r.run_for(800);
    let hz = dominant_hz(&r.samples[at + 2 * 960..at + 2 * 960 + 9600]);
    assert!(
        (hz - 660.0).abs() < 50.0,
        "after seeking, the tone should still be B's 660 Hz, but it is {hz:.0} Hz"
    );
    assert_eq!(r.snapshot().audible.map(|i| i.0), Some(ids[1]));
}

// ---- what the listener is told, and what the log records ---------------------------------------

impl Rig {
    fn notices(&self) -> Vec<String> {
        self.shared.notices.lock().unwrap().clone()
    }
}

#[test]
fn a_skipped_song_is_announced_with_its_title_and_the_reason() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    let missing = Track::new("file:/definitely/missing.mp3", "Gone Song", "x");
    r.cmd(Command::PlayTracks {
        tracks: vec![missing, file_track("b_48k.wav")],
        start: 0,
    });
    assert!(r.run_until(10, |r| r.snapshot().audible.is_some()));
    let notices = r.notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        notices[0].contains("\u{201c}Gone Song\u{201d}"),
        "{notices:?}"
    );
    assert!(
        notices[0].contains("it was skipped") && notices[0].contains("isn't available"),
        "{notices:?}"
    );
    assert!(notices[0].contains("The log has details"), "{notices:?}");
}

#[test]
fn an_upcoming_song_that_cannot_play_is_announced_before_it_is_reached() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    let missing = Track::new("file:/definitely/missing.mp3", "Later Song", "x");
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav"), missing],
        start: 0,
    });
    assert!(r.run_until(10, |r| !r.notices().is_empty()));
    assert!(
        r.notices()[0].contains("it will be skipped"),
        "{:?}",
        r.notices()
    );
}

#[test]
fn being_offline_is_not_announced_as_a_skip() {
    let src = TestSource {
        related: Vec::new(),
        resolve: Box::new(|_| Err(SourceError::Offline)),
    };
    let mut r = Rig::new(src);
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav")],
        start: 0,
    });
    assert!(r.run_until(5, |r| matches!(
        r.snapshot().transport,
        Transport::Failed { .. }
    )));
    assert!(
        r.notices().is_empty(),
        "the player bar already shows this; no skip message: {:?}",
        r.notices()
    );
}

#[test]
fn normal_playback_says_nothing() {
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    r.cmd(Command::PlayTracks {
        tracks: vec![file_track("a_44k.wav"), file_track("b_48k.wav")],
        start: 0,
    });
    assert!(
        r.run_until(15, |r| r.snapshot().transport == Transport::Idle
            && r.audible_log.len() >= 3)
    );
    assert!(r.notices().is_empty(), "{:?}", r.notices());
}

#[test]
fn a_stream_cut_off_long_before_its_known_length_is_reported() {
    // The server only has 4 s of audio, but the song is known to be 60 s long.
    let url = serve(
        make_wav(4),
        usize::MAX,
        Arc::new(AtomicBool::new(true)),
        Duration::ZERO,
    );
    let src = TestSource {
        related: Vec::new(),
        resolve: Box::new(move |_| {
            Ok(StreamInfo {
                url: url.clone(),
                ext: "wav".into(),
                acodec: None,
                abr_kbps: None,
                ..Default::default()
            })
        }),
    };
    let mut r = Rig::new(src);
    r.cmd(Command::SetAutoplay(false));
    let mut t = Track::new("yt:cutoff", "Cut Off Song", "x");
    t.duration_ms = Some(60_000);
    r.cmd(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });
    assert!(
        r.run_until(30, |r| r.snapshot().transport == Transport::Idle
            && r.audible_log.len() >= 3)
    );
    let notices = r.notices();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("Cut Off Song"), "{notices:?}");
    assert!(
        notices[0].contains("0:03 of 1:00") || notices[0].contains("0:04 of 1:00"),
        "says how far it got and how long it should have been: {notices:?}"
    );
}

#[test]
fn the_early_end_rule_ignores_normal_endings_and_unknown_lengths() {
    use crate::engine::ended_early;
    assert!(!ended_early(199_000, Some(200_000)), "ends about on time");
    assert!(
        !ended_early(130_000, Some(200_000)),
        "65%: not worth a warning"
    );
    assert!(ended_early(41_000, Some(200_000)));
    assert!(!ended_early(1_000, None), "unknown length");
    assert!(
        !ended_early(2_000, Some(10_000)),
        "short songs are never flagged"
    );
}

#[test]
fn the_wording_names_the_song_and_the_reason() {
    use crate::engine::{early_end_notice, skipped_notice};
    use pyxee_core::Failure;
    let n = skipped_notice("My Song", &Failure::Decode, false);
    assert_eq!(n, "\u{201c}My Song\u{201d} can't be played, so it was skipped: its audio could not be read. The log has details.");
    assert!(skipped_notice(
        "S",
        &Failure::Other("ffmpeg could not be started".into()),
        true
    )
    .contains("ffmpeg could not be started"));
    assert!(early_end_notice("S", 41_000, 200_000).contains("0:41 of 3:20"));
}

/// Captures log records so a test can check what was written.
struct Capture(Mutex<Vec<String>>);
impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        self.0
            .lock()
            .unwrap()
            .push(format!("{} {}", r.level(), r.args()));
    }
    fn flush(&self) {}
}
static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

#[test]
fn the_log_tells_the_story_of_a_song_including_why_it_failed() {
    let _ = log::set_logger(&CAPTURE);
    log::set_max_level(log::LevelFilter::Info);
    let mut r = Rig::new(TestSource::local());
    r.cmd(Command::SetAutoplay(false));
    let ok = {
        let mut t = file_track("a_44k.wav");
        t.title = "LogProbe Good".into();
        t
    };
    let bad = Track::new("file:/definitely/missing.mp3", "LogProbe Bad", "x");
    r.cmd(Command::PlayTracks {
        tracks: vec![bad, ok],
        start: 0,
    });
    assert!(
        r.run_until(15, |r| r.snapshot().transport == Transport::Idle
            && r.audible_log.len() >= 3)
    );
    let lines: Vec<String> = CAPTURE
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|l| l.contains("LogProbe"))
        .cloned()
        .collect();
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    assert!(
        has("INFO start: \"LogProbe Bad\" (file:/definitely/missing.mp3)"),
        "{lines:#?}"
    );
    assert!(
        has("WARN could not start \"LogProbe Bad\""),
        "failures are warnings with the reason: {lines:#?}"
    );
    assert!(has("INFO ready: \"LogProbe Good\""), "{lines:#?}");
    assert!(has("INFO audible: \"LogProbe Good\""), "{lines:#?}");
    assert!(has("INFO ended: \"LogProbe Good\""), "{lines:#?}");
}

// ---- a server without range support: the built-in reader cannot be used, ffmpeg can ----------

fn serve_without_range_support(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut req = Vec::new();
            let mut b = [0u8; 1];
            while !req.ends_with(b"\r\n\r\n") {
                if s.read(&mut b).unwrap_or(0) == 0 {
                    break;
                }
                req.push(b[0]);
            }
            // Ignores Range entirely, like a simple file server.
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&body);
        }
    });
    format!("http://{addr}/track.wav")
}

#[test]
fn a_server_that_ignores_range_requests_falls_back_to_ffmpeg_instead_of_failing_the_song() {
    if std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_err()
    {
        eprintln!("ffmpeg is not installed here; skipping");
        return;
    }
    let url = serve_without_range_support(make_wav(3));
    let src = TestSource {
        related: Vec::new(),
        resolve: Box::new(move |_| {
            Ok(StreamInfo {
                url: url.clone(),
                ext: "wav".into(),
                acodec: None,
                abr_kbps: None,
                ..Default::default()
            })
        }),
    };
    let mut r = Rig::new(src);
    r.cmd(Command::SetAutoplay(false));
    let mut t = Track::new("yt:norange", "No Range Song", "x");
    t.duration_ms = Some(3_000);
    r.cmd(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });
    assert!(
        r.run_until(30, |r| r.snapshot().transport == Transport::Idle
            && r.audible_log.len() >= 3),
        "{:?}",
        r.snapshot()
    );
    assert!(
        r.notices().is_empty(),
        "it played normally: {:?}",
        r.notices()
    );
    assert!(
        (2.8..3.3).contains(&(r.samples.len() as f64 / 2.0 / 48_000.0)),
        "all 3 s were heard"
    );
}
