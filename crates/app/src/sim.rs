//! Simulated audio engine for development, until the real engine is wired in (milestone 4).
//! It obeys the same contract as the real one (see docs/core.md): load time before a track is
//! audible, gapless handover into a prefetched track, position events, radio answers. It is a
//! pure state machine driven by `tick`, so it is testable without threads or clocks.

use pyxee_core::{Effect, EngineEvent, EntryId, Track};

pub const TICK_MS: u64 = 250;
const LOAD_MS: u64 = 500;
const PREFETCH_MS: u64 = 300;
const RADIO_MS: u64 = 800;
const DEFAULT_DURATION_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq)]
pub enum SimOut {
    Engine(EngineEvent),
    /// Answer to `Effect::NeedRadio`.
    Radio(Vec<Track>),
}

struct Slot {
    id: EntryId,
    pos_ms: u64,
    dur_ms: u64,
    audible: bool,
    load_left_ms: u64,
}

impl Slot {
    fn new(id: EntryId, track: &Track, pos_ms: u64, load_left_ms: u64) -> Self {
        Self {
            id,
            pos_ms,
            dur_ms: track.duration_ms.unwrap_or(DEFAULT_DURATION_MS),
            audible: false,
            load_left_ms,
        }
    }
}

struct Prefetch {
    id: EntryId,
    track: Track,
    left_ms: u64,
    ready: bool,
}

struct RadioJob {
    left_ms: u64,
    seed_key: String,
    count: usize,
}

pub struct Sim {
    catalog: Vec<Track>,
    current: Option<Slot>,
    paused: bool,
    prefetch: Option<Prefetch>,
    radio: Option<RadioJob>,
}

impl Sim {
    pub fn new(catalog: Vec<Track>) -> Self {
        Self {
            catalog,
            current: None,
            paused: false,
            prefetch: None,
            radio: None,
        }
    }

    pub fn apply(&mut self, e: &Effect) {
        match e {
            Effect::StartTrack {
                id,
                track,
                start_ms,
            } => {
                // A prefetched track that is ready starts without the load delay.
                let ready = self
                    .prefetch
                    .as_ref()
                    .is_some_and(|p| p.id == *id && p.ready);
                self.prefetch = None;
                self.paused = false;
                self.current = Some(Slot::new(
                    *id,
                    track,
                    *start_ms,
                    if ready { 0 } else { LOAD_MS },
                ));
            }
            Effect::Prefetch { id, track } => {
                self.prefetch = Some(Prefetch {
                    id: *id,
                    track: track.clone(),
                    left_ms: PREFETCH_MS,
                    ready: false,
                });
            }
            Effect::CancelPrefetch => self.prefetch = None,
            Effect::Pause => self.paused = true,
            Effect::Resume => self.paused = false,
            Effect::Stop => {
                self.current = None;
                self.prefetch = None;
                self.paused = false;
            }
            Effect::Seek { ms } => {
                if let Some(c) = self.current.as_mut() {
                    c.pos_ms = (*ms).min(c.dur_ms);
                }
            }
            Effect::NeedRadio { seed, count } => {
                self.radio = Some(RadioJob {
                    left_ms: RADIO_MS,
                    seed_key: seed.key.clone(),
                    count: *count,
                });
            }
            Effect::Persist => {}
        }
    }

    pub fn tick(&mut self, ms: u64) -> Vec<SimOut> {
        let mut out = Vec::new();

        if let Some(p) = self.prefetch.as_mut() {
            if !p.ready {
                p.left_ms = p.left_ms.saturating_sub(ms);
                if p.left_ms == 0 {
                    p.ready = true;
                    out.push(SimOut::Engine(EngineEvent::Prefetched { id: p.id }));
                }
            }
            // The engine ran dry while this was preparing: start it as soon as it is ready.
            if p.ready && self.current.is_none() {
                if let Some(p) = self.prefetch.take() {
                    self.current = Some(Slot::new(p.id, &p.track, 0, 0));
                }
            }
        }

        if let Some(job) = self.radio.as_mut() {
            job.left_ms = job.left_ms.saturating_sub(ms);
            if job.left_ms == 0 {
                let job = self.radio.take().expect("checked above");
                let tracks = self
                    .catalog
                    .iter()
                    .filter(|t| t.key != job.seed_key)
                    .take(job.count)
                    .cloned()
                    .collect();
                out.push(SimOut::Radio(tracks));
            }
        }

        if !self.paused {
            self.tick_current(ms, &mut out);
        }
        out
    }

    fn tick_current(&mut self, ms: u64, out: &mut Vec<SimOut>) {
        let Some(c) = self.current.as_mut() else {
            return;
        };
        if !c.audible {
            c.load_left_ms = c.load_left_ms.saturating_sub(ms);
            if c.load_left_ms == 0 {
                c.audible = true;
                out.push(SimOut::Engine(EngineEvent::Audible { id: c.id }));
            }
            return;
        }
        c.pos_ms = (c.pos_ms + ms).min(c.dur_ms);
        out.push(SimOut::Engine(EngineEvent::Position {
            id: c.id,
            ms: c.pos_ms,
        }));
        if c.pos_ms >= c.dur_ms {
            let id = c.id;
            out.push(SimOut::Engine(EngineEvent::Ended { id }));
            self.current = None;
            // Gapless: continue straight into a ready prefetched track.
            if self.prefetch.as_ref().is_some_and(|p| p.ready) {
                if let Some(p) = self.prefetch.take() {
                    let mut next = Slot::new(p.id, &p.track, 0, 0);
                    next.audible = true;
                    out.push(SimOut::Engine(EngineEvent::Audible { id: p.id }));
                    self.current = Some(next);
                }
            }
        }
    }
}

/// Demo tracks. Keep in sync with `ui/src/lib/mock.ts`.
pub fn catalog() -> Vec<Track> {
    [
        ("Neon Harbor", "Lowtide", 38),
        ("Paper Planes", "Alma Vey", 31),
        ("Slow Orbit", "Kestrel Hall", 36),
        ("Golden Hour Static", "Marlow", 29),
        ("Glass Rooms", "Nova Fen", 34),
        ("Afterimage", "Dune Choir", 40),
        ("Low Light Parade", "Wren & Co.", 27),
        ("Salt and Signal", "Harbor Lines", 33),
    ]
    .iter()
    .enumerate()
    .map(|(i, (title, artist, secs))| Track {
        key: format!("demo:{i}"),
        title: (*title).into(),
        artist: (*artist).into(),
        duration_ms: Some(secs * 1000),
        thumbnail: None,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyxee_core::{Command, Player, Repeat, Transport};

    fn short(n: usize, secs: u64) -> Vec<Track> {
        (0..n)
            .map(|i| {
                let mut t = Track::new(&format!("t{i}"), &format!("T{i}"), "x");
                t.duration_ms = Some(secs * 1000);
                t
            })
            .collect()
    }

    /// Connects a Player to a Sim like the real app does, advancing simulated time.
    struct Rig {
        p: Player,
        sim: Sim,
        audible_log: Vec<Option<u64>>,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                p: Player::new(9),
                sim: Sim::new(catalog()),
                audible_log: Vec::new(),
            }
        }
        fn cmd(&mut self, c: Command) {
            for e in self.p.command(c) {
                self.sim.apply(&e);
            }
        }
        fn run(&mut self, total_ms: u64) {
            for _ in 0..total_ms / TICK_MS {
                for o in self.sim.tick(TICK_MS) {
                    let fx = match o {
                        SimOut::Engine(e) => self.p.engine(e),
                        SimOut::Radio(t) => self.p.command(Command::AppendRadio(t)),
                    };
                    for e in fx {
                        self.sim.apply(&e);
                    }
                }
                let a = self.p.snapshot().audible.map(|i| i.0);
                if self.audible_log.last() != Some(&a) {
                    self.audible_log.push(a);
                }
            }
        }
    }

    #[test]
    fn a_queue_plays_through_gaplessly_in_order_then_stops() {
        let mut r = Rig::new();
        r.cmd(Command::SetAutoplay(false));
        r.cmd(Command::PlayTracks {
            tracks: short(3, 2),
            start: 0,
        });
        assert_eq!(
            r.p.snapshot().audible,
            None,
            "nothing audible before the engine says so"
        );
        r.run(10_000);
        // None (loading) -> a -> b -> c -> None (ended); the next track never appears early or with a gap.
        let ids: Vec<u64> = r.p.queue().entries().iter().map(|e| e.id.0).collect();
        assert_eq!(
            r.audible_log,
            vec![None, Some(ids[0]), Some(ids[1]), Some(ids[2]), None]
        );
        assert_eq!(r.p.snapshot().transport, Transport::Idle);
    }

    #[test]
    fn skipping_is_instant_when_the_next_track_was_prefetched_and_loads_otherwise() {
        let mut r = Rig::new();
        r.cmd(Command::SetAutoplay(false));
        r.cmd(Command::PlayTracks {
            tracks: short(3, 20),
            start: 0,
        });
        r.run(1_000);
        let first = r.p.snapshot().audible;
        assert!(first.is_some());

        // Track 2 was prefetched while track 1 played, so skipping to it is immediate.
        r.cmd(Command::Next);
        assert_eq!(
            r.p.snapshot().audible,
            None,
            "never audible before the engine reports it"
        );
        r.run(250);
        let second = r.p.snapshot().audible;
        assert!(second.is_some() && second != first);

        // Skip again before track 3's prefetch finished: it has to load first.
        r.cmd(Command::Next);
        r.run(250);
        assert_eq!(r.p.snapshot().audible, None, "still loading after 250 ms");
        r.run(750);
        assert!(r.p.snapshot().audible.is_some());
        assert_ne!(r.p.snapshot().audible, second);
    }

    #[test]
    fn pausing_freezes_the_position_and_resume_continues() {
        let mut r = Rig::new();
        r.cmd(Command::SetAutoplay(false));
        r.cmd(Command::PlayTracks {
            tracks: short(2, 20),
            start: 0,
        });
        r.run(2_000);
        r.cmd(Command::Pause);
        let at = r.p.snapshot().position_ms;
        r.run(3_000);
        assert_eq!(r.p.snapshot().position_ms, at);
        r.cmd(Command::Resume);
        r.run(1_000);
        assert!(r.p.snapshot().position_ms > at);
    }

    #[test]
    fn seek_moves_the_position() {
        let mut r = Rig::new();
        r.cmd(Command::SetAutoplay(false));
        r.cmd(Command::PlayTracks {
            tracks: short(1, 60),
            start: 0,
        });
        r.run(1_000);
        r.cmd(Command::Seek(30_000));
        r.run(500);
        let pos = r.p.snapshot().position_ms;
        assert!((30_000..31_000).contains(&pos), "position {pos}");
    }

    #[test]
    fn autoplay_fetches_radio_and_keeps_playing_after_the_queue_runs_out() {
        let mut r = Rig::new();
        r.cmd(Command::PlayTracks {
            tracks: short(1, 2),
            start: 0,
        });
        r.run(12_000);
        assert!(r
            .p
            .queue()
            .entries()
            .iter()
            .any(|e| e.origin == pyxee_core::Origin::Radio));
        let played: Vec<_> = r.audible_log.iter().flatten().collect();
        assert!(
            played.len() >= 2,
            "radio tracks followed: {:?}",
            r.audible_log
        );
    }

    #[test]
    fn repeat_one_keeps_replaying_the_same_track() {
        let mut r = Rig::new();
        r.cmd(Command::SetAutoplay(false));
        r.cmd(Command::PlayTracks {
            tracks: short(2, 2),
            start: 0,
        });
        r.cmd(Command::SetRepeat(Repeat::One));
        r.run(9_000);
        let first = r.p.queue().entries()[0].id.0;
        assert!(
            r.audible_log.iter().flatten().all(|&id| id == first),
            "{:?}",
            r.audible_log
        );
        assert!(r.p.snapshot().audible.is_some());
    }
}
