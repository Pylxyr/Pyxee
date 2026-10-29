//! The playback state machine. Sans-IO: feed it `Command`s (user) and `EngineEvent`s (audio
//! engine), get back `Effect`s to carry out. Nothing here touches audio, network or time.
//!
//! Key rule: `Transport` only reports a track as playing when the engine says it is *audible*
//! (see `HeardTracker`), never merely because it was requested or decoded.

use crate::queue::{EntryId, Origin, Queue, Repeat, UndoKind};
use crate::track::{Track, TrackDetails};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// "Previous" within this many ms of the start goes to the previous track; later it restarts.
pub const RESTART_THRESHOLD_MS: u64 = 3_000;
const MAX_AUTO_SKIPS: u8 = 3;
const MAX_URL_RETRIES: u8 = 2;
/// A re-resolved URL that then played this long worked: the next expiry (a long stream outliving
/// its URL) gets a fresh budget. Without this, every re-resolve that reaches "audible" would reset
/// the count, and a URL the server rejects shortly after each resume would loop forever.
const URL_RETRY_RESET_MS: u64 = 60_000;
const RADIO_THRESHOLD: usize = 1;
const RADIO_BATCH: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Offline,
    RateLimited,
    /// Removed, region-locked, private.
    Unavailable,
    Decode,
    /// The server refused the stream URL (401/403/410) and re-resolving did not help. Carries the
    /// HTTP status when it is known.
    Expired(Option<u16>),
    /// The tool that finds songs is missing or broken (not this song's fault): it will fail the same
    /// way for the next song, so the queue must not be skipped through. Carries the explanation.
    Tool(String),
    Other(String),
}

impl Failure {
    /// Retrying later may work, so skipping through the queue would only burn tracks.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Failure::Offline | Failure::RateLimited | Failure::Tool(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Idle,
    /// Requested; nothing of it is audible yet (and whatever played before has stopped).
    Loading {
        id: EntryId,
    },
    Playing {
        id: EntryId,
    },
    /// `loaded: false` after a restart: the track is selected but the engine has nothing loaded.
    Paused {
        id: EntryId,
        loaded: bool,
    },
    /// Audible track is waiting for data (underrun / network).
    Stalled {
        id: EntryId,
    },
    Failed {
        id: EntryId,
        failure: Failure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    PlayEntry(EntryId),
    PlayTracks {
        tracks: Vec<Track>,
        start: usize,
    },
    Toggle,
    Pause,
    Resume,
    Next,
    Previous,
    Seek(u64),
    Retry,
    SetRepeat(Repeat),
    SetShuffle(bool),
    SetAutoplay(bool),
    AddToQueue(Track),
    PlayNext(Track),
    Remove(EntryId),
    Move {
        id: EntryId,
        to: usize,
    },
    /// Reorders the "Up next" list: `to` is the index among the upcoming songs (0 = play next).
    MoveUpcoming {
        id: EntryId,
        to: usize,
    },
    /// While on, the song that is playing is the last: nothing is prefetched, and when it ends the player
    /// parks (paused) on the song that would have followed. The sleep timer uses it. Not saved with the session.
    StopAfterCurrent(bool),
    Clear,
    Undo,
    /// Result of a `NeedRadio` effect.
    AppendRadio(Vec<Track>),
    /// A `NeedRadio` effect failed (as opposed to finding nothing).
    RadioFailed(Failure),
}

/// What the audio engine reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    /// First sample of the track reached the output.
    Audible {
        id: EntryId,
    },
    /// Last sample of the track was heard.
    Ended {
        id: EntryId,
    },
    Position {
        id: EntryId,
        ms: u64,
    },
    Prefetched {
        id: EntryId,
    },
    PrefetchFailed {
        id: EntryId,
        failure: Failure,
    },
    LoadFailed {
        id: EntryId,
        failure: Failure,
    },
    /// The stream URL was rejected mid-play; resolve again and continue.
    UrlExpired {
        id: EntryId,
    },
    Stalled {
        id: EntryId,
    },
    Recovered {
        id: EntryId,
    },
    /// The stream's own metadata: fills in an unknown artist, a missing length or a better cover.
    TrackDetails {
        id: EntryId,
        details: TrackDetails,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Stop everything now and play this track from `start_ms` (resolve, open, play).
    /// The engine may reuse a track it already prepared. Replaces any prefetched tracks.
    StartTrack {
        id: EntryId,
        track: Track,
        start_ms: u64,
    },
    /// Resolve and prepare `track` to follow the current one gaplessly. If the engine has run
    /// dry by the time it is ready, it starts it immediately.
    Prefetch {
        id: EntryId,
        track: Track,
    },
    CancelPrefetch,
    Pause,
    Resume,
    Stop,
    Seek {
        ms: u64,
    },
    /// Fetch similar songs for autoplay; answer with `Command::AppendRadio`.
    NeedRadio {
        seed: Track,
        count: usize,
    },
    /// Queue or settings changed: save them.
    Persist,
}

/// Everything needed to resume after a restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persisted {
    pub queue: Queue,
    pub position_ms: u64,
    pub autoplay: bool,
}

/// A read-only view for the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub transport: Transport,
    /// What the user can hear right now.
    pub audible: Option<EntryId>,
    /// Requested but not yet audible.
    pub loading: Option<EntryId>,
    pub position_ms: u64,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub autoplay: bool,
    pub can_undo: bool,
}

pub struct Player {
    queue: Queue,
    transport: Transport,
    position_ms: u64,
    autoplay: bool,
    radio_requested: bool,
    radio_dry: bool,
    pending_advance: bool,
    /// The song that was playing when a radio request failed: no new request until another song plays.
    radio_failed_on: Option<EntryId>,
    /// Radio answers still to come for requests made before the queue was replaced or cleared; each is ignored.
    stale_radio: u8,
    /// (id, ready) of the track the engine is preparing to follow the current one.
    prefetch: Option<(EntryId, bool)>,
    no_prefetch_for: Option<EntryId>,
    stop_after_current: bool,
    skip: HashSet<EntryId>,
    auto_skips: u8,
    url_retries: u8,
    /// The song the retries were spent on, and where it was when the last one began.
    url_retry_id: Option<EntryId>,
    url_retry_at_ms: u64,
    dirty: bool,
    effects: Vec<Effect>,
}

impl Player {
    pub fn new(seed: u64) -> Self {
        Self::with_queue(Queue::new(seed), 0, true)
    }

    fn with_queue(queue: Queue, position_ms: u64, autoplay: bool) -> Self {
        let transport = match queue.current_id() {
            Some(id) => Transport::Paused { id, loaded: false },
            None => Transport::Idle,
        };
        Self {
            queue,
            transport,
            position_ms,
            autoplay,
            radio_requested: false,
            radio_dry: false,
            pending_advance: false,
            radio_failed_on: None,
            stale_radio: 0,
            prefetch: None,
            no_prefetch_for: None,
            stop_after_current: false,
            skip: HashSet::new(),
            auto_skips: 0,
            url_retries: 0,
            url_retry_id: None,
            url_retry_at_ms: 0,
            dirty: false,
            effects: Vec::new(),
        }
    }

    /// Restores a saved session: the track is selected and paused at its position, nothing plays
    /// until the user presses play.
    pub fn restore(p: Persisted) -> Self {
        Self::with_queue(p.queue, p.position_ms, p.autoplay)
    }

    pub fn persisted(&self) -> Persisted {
        Persisted {
            queue: self.queue.clone(),
            position_ms: self.position_ms,
            autoplay: self.autoplay,
        }
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn snapshot(&self) -> Snapshot {
        let (audible, loading) = match &self.transport {
            Transport::Playing { id } | Transport::Stalled { id } => (Some(*id), None),
            Transport::Paused { id, loaded: true } => (Some(*id), None),
            Transport::Loading { id } => (None, Some(*id)),
            _ => (None, None),
        };
        Snapshot {
            transport: self.transport.clone(),
            audible,
            loading,
            position_ms: self.position_ms,
            repeat: self.queue.repeat(),
            shuffle: self.queue.shuffle(),
            autoplay: self.autoplay,
            can_undo: self.queue.can_undo(),
        }
    }

    pub fn command(&mut self, c: Command) -> Vec<Effect> {
        self.run(|p| p.on_command(c))
    }

    pub fn engine(&mut self, e: EngineEvent) -> Vec<Effect> {
        self.run(|p| p.on_engine(e))
    }

    fn run(&mut self, f: impl FnOnce(&mut Self)) -> Vec<Effect> {
        f(self);
        self.plan_prefetch();
        self.plan_radio();
        if std::mem::take(&mut self.dirty) {
            self.effects.push(Effect::Persist);
        }
        std::mem::take(&mut self.effects)
    }

    // ---------- helpers ----------
    fn is_active(&self) -> bool {
        matches!(
            self.transport,
            Transport::Playing { .. }
                | Transport::Stalled { .. }
                | Transport::Paused { loaded: true, .. }
        )
    }

    fn begin_load(&mut self, id: EntryId, start_ms: u64) {
        let Some(entry) = self.queue.get(id) else {
            return;
        };
        let track = entry.track.clone();
        self.prefetch = None;
        self.position_ms = start_ms;
        self.transport = Transport::Loading { id };
        self.pending_advance = false;
        self.dirty = true;
        self.effects.push(Effect::StartTrack {
            id,
            track,
            start_ms,
        });
    }

    /// A load the listener asked for (not an automatic one): the retries spent on earlier expired URLs
    /// do not count against it.
    fn begin_load_fresh(&mut self, id: EntryId, start_ms: u64) {
        self.url_retries = 0;
        self.url_retry_id = None;
        self.begin_load(id, start_ms);
    }

    fn stop(&mut self) {
        self.effects.push(Effect::Stop);
        self.prefetch = None;
        self.transport = Transport::Idle;
        self.position_ms = 0;
        self.pending_advance = false;
        self.dirty = true;
    }

    fn play_from_idle(&mut self) {
        let target = self.queue.current_id().or_else(|| {
            self.queue
                .next_candidate(false, &|i| self.skip.contains(&i))
        });
        if let Some(id) = target {
            self.queue.jump_to(id);
            let start = if self.queue.current_id() == Some(id) {
                self.position_ms
            } else {
                0
            };
            self.begin_load(id, start);
        }
    }

    fn next_user(&mut self) {
        match self.queue.advance(false, &|i| self.skip.contains(&i)) {
            Some(id) => self.begin_load_fresh(id, 0),
            None => self.end_of_queue(),
        }
    }

    /// Nothing follows the current track. Autoplay asks for more; otherwise playback stops.
    fn end_of_queue(&mut self) {
        let seed = self.queue.current().map(|e| e.track.clone());
        if let (true, Some(seed), false) = (self.autoplay, seed, self.radio_dry) {
            self.pending_advance = true;
            if !self.radio_requested {
                self.radio_requested = true;
                self.effects.push(Effect::NeedRadio {
                    seed,
                    count: RADIO_BATCH,
                });
            }
            if !matches!(
                self.transport,
                Transport::Playing { .. } | Transport::Stalled { .. }
            ) {
                self.transport = Transport::Idle;
            }
        } else {
            self.stop();
        }
    }

    fn restart_current(&mut self) {
        if self.is_active() {
            self.effects.push(Effect::Seek { ms: 0 });
        }
        self.position_ms = 0;
    }

    // ---------- planning ----------
    fn plan_prefetch(&mut self) {
        if !self.is_active() {
            return;
        }
        let want = self
            .queue
            .next_candidate(true, &|i| self.skip.contains(&i))
            .filter(|w| Some(*w) != self.no_prefetch_for)
            .filter(|_| !self.stop_after_current);
        if let (Some((p, _)), Some(w)) = (self.prefetch, want) {
            if p == w {
                return;
            }
        }
        if self.prefetch.take().is_some() {
            self.effects.push(Effect::CancelPrefetch);
        }
        if let Some(w) = want {
            if let Some(e) = self.queue.get(w) {
                self.effects.push(Effect::Prefetch {
                    id: w,
                    track: e.track.clone(),
                });
                self.prefetch = Some((w, false));
            }
        }
    }

    fn plan_radio(&mut self) {
        if !self.autoplay
            || self.radio_requested
            || self.radio_dry
            || self.radio_failed_on.is_some() && self.radio_failed_on == self.queue.current_id()
            || self.queue.repeat() != Repeat::Off
            || !self.is_active()
            || self.queue.upcoming().len() > RADIO_THRESHOLD
        {
            return;
        }
        if let Some(seed) = self.queue.current().map(|e| e.track.clone()) {
            self.radio_requested = true;
            self.effects.push(Effect::NeedRadio {
                seed,
                count: RADIO_BATCH,
            });
        }
    }

    // ---------- commands ----------
    fn on_command(&mut self, c: Command) {
        match c {
            Command::PlayEntry(id) => {
                if self.queue.jump_to(id) {
                    self.skip.remove(&id);
                    self.begin_load_fresh(id, 0);
                }
            }
            Command::PlayTracks { tracks, start } => {
                if tracks.is_empty() {
                    return;
                }
                if let Some(id) = self.queue.replace(tracks, start) {
                    self.skip.clear();
                    if self.radio_requested {
                        self.stale_radio = self.stale_radio.saturating_add(1);
                    }
                    self.radio_requested = false;
                    self.radio_dry = false;
                    self.radio_failed_on = None;
                    self.auto_skips = 0;
                    self.no_prefetch_for = None;
                    self.begin_load_fresh(id, 0);
                }
            }
            Command::Toggle => match self.transport {
                Transport::Playing { .. }
                | Transport::Stalled { .. }
                | Transport::Loading { .. } => self.on_command(Command::Pause),
                _ => self.on_command(Command::Resume),
            },
            Command::Pause => match self.transport.clone() {
                Transport::Playing { id } | Transport::Stalled { id } => {
                    self.effects.push(Effect::Pause);
                    self.transport = Transport::Paused { id, loaded: true };
                }
                Transport::Loading { id } => {
                    self.effects.push(Effect::Stop);
                    self.prefetch = None;
                    self.pending_advance = false;
                    self.transport = Transport::Paused { id, loaded: false };
                }
                _ => {}
            },
            Command::Resume => match self.transport.clone() {
                Transport::Paused { id, loaded: true } => {
                    self.effects.push(Effect::Resume);
                    self.transport = Transport::Playing { id };
                }
                Transport::Paused { id, loaded: false } => {
                    self.begin_load_fresh(id, self.position_ms)
                }
                Transport::Idle => self.play_from_idle(),
                Transport::Failed { .. } => self.on_command(Command::Retry),
                _ => {}
            },
            Command::Retry => {
                if let Transport::Failed { id, .. } = self.transport.clone() {
                    self.no_prefetch_for = None;
                    self.auto_skips = 0;
                    self.skip.remove(&id);
                    self.begin_load_fresh(id, self.position_ms);
                }
            }
            Command::Next => self.next_user(),
            Command::Previous => {
                let can_restart = self.position_ms > RESTART_THRESHOLD_MS;
                if can_restart {
                    self.restart_current();
                } else if let Some(id) = self.queue.previous(&|i| self.skip.contains(&i)) {
                    self.begin_load_fresh(id, 0);
                } else {
                    self.restart_current();
                }
            }
            Command::Seek(ms) => {
                let ms = match self.queue.current().and_then(|e| e.track.duration_ms) {
                    Some(d) => ms.min(d),
                    None => ms,
                };
                match self.transport {
                    Transport::Playing { .. }
                    | Transport::Stalled { .. }
                    | Transport::Paused { loaded: true, .. } => {
                        self.effects.push(Effect::Seek { ms });
                        self.position_ms = ms;
                    }
                    Transport::Paused { loaded: false, .. } => {
                        self.position_ms = ms;
                        self.dirty = true;
                    }
                    _ => {}
                }
            }
            Command::SetRepeat(r) => {
                self.queue.set_repeat(r);
                self.dirty = true;
            }
            Command::SetShuffle(on) => {
                self.queue.set_shuffle(on);
                self.dirty = true;
            }
            Command::SetAutoplay(on) => {
                self.autoplay = on;
                self.radio_dry = false;
                self.dirty = true;
            }
            Command::AddToQueue(t) => {
                self.queue.add(t, Origin::User);
                self.radio_dry = false;
                self.dirty = true;
            }
            Command::PlayNext(t) => {
                self.queue.play_next(t);
                self.radio_dry = false;
                self.dirty = true;
            }
            Command::Remove(id) => self.remove(id),
            Command::Move { id, to } => {
                if self.queue.move_item(id, to) {
                    self.dirty = true;
                }
            }
            Command::MoveUpcoming { id, to } => {
                if self.queue.move_upcoming(id, to) {
                    self.dirty = true;
                }
            }
            Command::StopAfterCurrent(on) => self.stop_after_current = on,
            Command::Clear => {
                if !self.queue.is_empty() {
                    self.queue.clear();
                    if self.radio_requested {
                        self.stale_radio = self.stale_radio.saturating_add(1);
                        self.radio_requested = false;
                    }
                    self.stop();
                }
            }
            Command::Undo => {
                if let Some(kind) = self.queue.undo() {
                    self.dirty = true;
                    if kind == UndoKind::Clear {
                        if let Some(id) = self.queue.current_id() {
                            self.transport = Transport::Paused { id, loaded: false };
                        }
                    }
                }
            }
            Command::AppendRadio(tracks) => self.append_radio(tracks),
            Command::RadioFailed(_) => self.radio_failed(),
        }
    }

    fn remove(&mut self, id: EntryId) {
        let Some(was_current) = self.queue.remove(id) else {
            return;
        };
        self.dirty = true;
        if matches!(self.prefetch, Some((p, _)) if p == id) {
            self.prefetch = None;
            self.effects.push(Effect::CancelPrefetch);
        }
        if was_current {
            match self.queue.advance(false, &|i| self.skip.contains(&i)) {
                Some(n) => self.begin_load(n, 0),
                None => self.end_of_queue(),
            }
        }
    }

    fn append_radio(&mut self, tracks: Vec<Track>) {
        if self.stale_radio > 0 {
            // The answer to a request made before the queue was replaced or cleared.
            self.stale_radio -= 1;
            return;
        }
        self.radio_requested = false;
        let mut added = 0;
        for t in tracks {
            if !self.queue.entries().iter().any(|e| e.track.key == t.key) {
                self.queue.add(t, Origin::Radio);
                added += 1;
            }
        }
        self.dirty = true;
        if added == 0 {
            self.radio_dry = true;
        }
        if std::mem::take(&mut self.pending_advance) {
            if added > 0 {
                self.next_user();
            } else {
                self.stop();
            }
        }
    }

    /// The request for similar songs failed. Unlike an empty answer this does not mean there is nothing
    /// to find, so autoplay stays on and asks again once another song is playing.
    fn radio_failed(&mut self) {
        if self.stale_radio > 0 {
            self.stale_radio -= 1;
            return;
        }
        self.radio_requested = false;
        self.radio_failed_on = self.queue.current_id();
        if std::mem::take(&mut self.pending_advance) {
            self.stop();
        }
    }

    // ---------- engine events ----------
    fn on_engine(&mut self, e: EngineEvent) {
        match e {
            EngineEvent::TrackDetails { id, details } => {
                let mut changed = false;
                self.queue.update_track(id, |t| changed = t.fill(&details));
                if changed {
                    self.dirty = true; // the saved session should carry the better data
                }
            }
            EngineEvent::Audible { id } => {
                if !self.queue.contains(id) {
                    return;
                }
                // Nothing is meant to be playing (stopped, cleared, paused without a loaded song, failed):
                // sound from an old stream must not bring the player back to life.
                if matches!(
                    self.transport,
                    Transport::Idle
                        | Transport::Paused { loaded: false, .. }
                        | Transport::Failed { .. }
                ) {
                    return;
                }
                self.skip.remove(&id);
                // The user paused while this track was already on its way (a gapless hand-over whose report
                // arrives after the Pause): the audio is paused, so the player stays paused on the new song.
                if matches!(self.transport, Transport::Paused { loaded: true, .. }) {
                    if self.queue.current_id() != Some(id) {
                        self.queue.jump_to(id);
                        self.position_ms = 0;
                    }
                    if matches!(self.prefetch, Some((p, _)) if p == id) {
                        self.prefetch = None;
                    }
                    self.transport = Transport::Paused { id, loaded: true };
                    self.no_prefetch_for = None;
                    self.dirty = true;
                    return;
                }
                let resumed_here =
                    matches!(self.transport, Transport::Loading { id: l } if l == id);
                if self.queue.current_id() != Some(id) {
                    self.queue.jump_to(id);
                }
                if matches!(self.prefetch, Some((p, _)) if p == id) {
                    self.prefetch = None;
                }
                if !resumed_here {
                    self.position_ms = 0;
                }
                self.transport = Transport::Playing { id };
                self.auto_skips = 0;
                // Another song is audible: its retries start fresh. The same song coming back after a
                // re-resolve keeps its count (see URL_RETRY_RESET_MS).
                if self.url_retry_id != Some(id) {
                    self.url_retries = 0;
                    self.url_retry_id = None;
                }
                self.no_prefetch_for = None;
                self.dirty = true;
            }
            EngineEvent::Ended { id } => self.on_ended(id),
            EngineEvent::Position { id, ms } => {
                if self.queue.current_id() == Some(id) && self.is_active() {
                    self.position_ms = ms;
                }
            }
            EngineEvent::Prefetched { id } => {
                if self.prefetch == Some((id, false)) {
                    self.prefetch = Some((id, true));
                }
            }
            EngineEvent::PrefetchFailed { id, failure } => {
                if matches!(self.prefetch, Some((p, _)) if p == id) {
                    self.prefetch = None;
                }
                if failure.is_transient() {
                    self.no_prefetch_for = Some(id);
                } else {
                    self.skip.insert(id);
                }
                // The previous song already ended and this one was the one being waited for: the engine
                // has nothing to start, so this is a failed load.
                if matches!(self.transport, Transport::Loading { id: l } if l == id) {
                    self.queue.jump_to(id);
                    self.position_ms = 0;
                    self.on_load_failed(id, failure);
                }
            }
            EngineEvent::LoadFailed { id, failure } => self.on_load_failed(id, failure),
            EngineEvent::UrlExpired { id } => {
                if self.transport == (Transport::Paused { id, loaded: true }) {
                    // Paused on a stream that died: nothing to resume. Unload it and keep the position, so
                    // Resume loads a fresh address from here.
                    self.effects.push(Effect::Stop);
                    self.prefetch = None;
                    self.transport = Transport::Paused { id, loaded: false };
                    return;
                }
                let mine = matches!(
                    self.transport,
                    Transport::Playing { id: p } | Transport::Stalled { id: p } | Transport::Loading { id: p } if p == id
                );
                if !mine {
                    return;
                }
                let played_since = self.position_ms.saturating_sub(self.url_retry_at_ms);
                if self.url_retry_id == Some(id) && played_since >= URL_RETRY_RESET_MS {
                    self.url_retries = 0;
                }
                if self.url_retries < MAX_URL_RETRIES {
                    self.url_retries += 1;
                    self.url_retry_id = Some(id);
                    self.url_retry_at_ms = self.position_ms;
                    self.begin_load(id, self.position_ms);
                } else {
                    self.transport = Transport::Failed {
                        id,
                        failure: Failure::Expired(None),
                    };
                }
            }
            EngineEvent::Stalled { id } => {
                if self.transport == (Transport::Playing { id }) {
                    self.transport = Transport::Stalled { id };
                }
            }
            EngineEvent::Recovered { id } => {
                if self.transport == (Transport::Stalled { id }) {
                    self.transport = Transport::Playing { id };
                }
            }
        }
    }

    fn on_ended(&mut self, id: EntryId) {
        let mine = self.queue.current_id() == Some(id)
            && matches!(
                self.transport,
                Transport::Playing { .. } | Transport::Stalled { .. }
            );
        if !mine {
            return; // stale: we already moved on
        }
        if self.stop_after_current {
            // The last song of a sleep timer: park on the one that would have come next.
            self.stop_after_current = false;
            self.effects.push(Effect::Stop);
            self.prefetch = None;
            self.dirty = true;
            match self.queue.next_candidate(true, &|i| self.skip.contains(&i)) {
                Some(next) => {
                    self.queue.jump_to(next);
                    self.position_ms = 0;
                    self.transport = Transport::Paused {
                        id: next,
                        loaded: false,
                    };
                }
                None => self.stop(),
            }
            return;
        }
        match self.queue.next_candidate(true, &|i| self.skip.contains(&i)) {
            Some(next) => match self.prefetch {
                // Gapless: the engine continues into `next`; stay put until it is audible.
                Some((p, true)) if p == next => {}
                // Still preparing: nothing audible now; the engine starts it as soon as it is ready.
                Some((p, false)) if p == next => {
                    self.queue.jump_to(next);
                    self.transport = Transport::Loading { id: next };
                }
                // Nothing usable prepared: cold start.
                _ => {
                    self.queue.jump_to(next);
                    self.begin_load(next, 0);
                }
            },
            None => {
                self.end_of_queue();
                // Waiting for radio after the last song ended: nothing is audible, so do not show Playing.
                if self.pending_advance {
                    if let Some(id) = self.queue.current_id() {
                        self.transport = Transport::Loading { id };
                    }
                }
            }
        }
    }

    fn on_load_failed(&mut self, id: EntryId, failure: Failure) {
        if !matches!(self.transport, Transport::Loading { id: l } if l == id) {
            return;
        }
        if failure.is_transient() {
            self.transport = Transport::Failed { id, failure };
            return;
        }
        self.skip.insert(id);
        self.auto_skips += 1;
        if self.auto_skips > MAX_AUTO_SKIPS {
            self.transport = Transport::Failed { id, failure };
            return;
        }
        match self.queue.advance(false, &|i| self.skip.contains(&i)) {
            Some(n) => self.begin_load(n, 0),
            None => self.transport = Transport::Failed { id, failure },
        }
    }
}
