//! Paces yt-dlp so normal use cannot trip YouTube's rate limit.
//!
//! Search, resolve, prefetch and radio each run on their own thread, so without this they all
//! start yt-dlp at once, and every yt-dlp run makes several requests of its own. The gate makes
//! them take turns instead:
//!
//! * **One run at a time.** Others wait in line.
//! * **A quiet gap** (`min_gap`) after each run ends before the next may start.
//! * **A cooldown** after a rate-limit answer: for `cooldown`, new runs are refused at once with
//!   `SourceError::RateLimited` (the player already treats that as transient: it shows "rate
//!   limited" with a Retry instead of skipping songs, and a failed prefetch is not retried). This
//!   is what stops a retry storm: queued callers are refused too, they do not each hit the server.
//! * **Newest waiter first.** The engine never cancels a worker for a song the listener has
//!   already skipped, it only discards the result. Serving the newest request first means the
//!   song they landed on is resolved before the stale ones, instead of behind all of them.
//!
//! Each `YtDlpSource` owns its gate (not a global), so tests cannot affect one another.

use crate::SourceError;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// Waits shorter than this are not worth a log line.
const LOG_WAITS_OVER: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateConfig {
    /// Quiet time after one yt-dlp run ends before the next may start.
    pub min_gap: Duration,
    /// How long new runs are refused after a rate-limit answer. Zero disables the cooldown.
    pub cooldown: Duration,
}

impl GateConfig {
    /// What the app uses: 1 s between runs, 2 minutes of refusals after a rate limit.
    pub const DEFAULT: Self = Self {
        min_gap: Duration::from_secs(1),
        cooldown: Duration::from_secs(120),
    };
    /// No pacing at all (still one run at a time). For tests that are not about pacing.
    pub const NONE: Self = Self {
        min_gap: Duration::ZERO,
        cooldown: Duration::ZERO,
    };
}

impl Default for GateConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How urgent a run is. Only the order of the line changes: the pacing (one run at a time, the gap after each, the
/// cooldown after a rate limit) is the same for both, so this never means more requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Something the person is waiting for: a search, the song they just picked.
    Interactive,
    /// Work nobody is waiting for yet: shelves, similar songs, the next song's address.
    Background,
}

thread_local! {
    static CURRENT: std::cell::Cell<Priority> = const { std::cell::Cell::new(Priority::Interactive) };
}

/// Runs `f`; every gated run it starts on this thread waits as [`Priority::Background`].
pub fn background<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(Priority);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(CURRENT.with(|c| c.replace(Priority::Background)));
    f()
}

fn current_priority() -> Priority {
    CURRENT.with(std::cell::Cell::get)
}

#[derive(Default)]
struct State {
    /// A run (or a caller about to start one) owns the slot.
    busy: bool,
    next_ticket: u64,
    /// Tickets of callers in line, oldest first. Interactive callers are served before background ones, and
    /// within each kind the newest is served first.
    waiting: Vec<(u64, Priority)>,
    /// The ticket the slot was handed to.
    granted: Option<u64>,
    /// Earliest start of the next run: the previous run's end plus `min_gap`.
    quiet_until: Option<Instant>,
    cooldown_until: Option<Instant>,
}

impl State {
    /// Time left on the cooldown, if one is on.
    fn cooling(&self, now: Instant) -> Option<Duration> {
        self.cooldown_until
            .and_then(|t| t.checked_duration_since(now))
            .filter(|d| !d.is_zero())
    }

    /// How long the caller that owns the slot must still wait before it may start.
    fn pause(&self, now: Instant) -> Duration {
        self.quiet_until
            .map_or(Duration::ZERO, |t| t.saturating_duration_since(now))
    }

    /// A run ended: record the gap (and the cooldown after a rate limit).
    fn finished(&mut self, now: Instant, cfg: GateConfig, rate_limited: bool) {
        self.quiet_until = Some(now + cfg.min_gap);
        if rate_limited && !cfg.cooldown.is_zero() {
            self.cooldown_until = Some(now + cfg.cooldown);
        }
    }

    /// Gives the slot to the newest interactive waiter (else the newest background one), or frees it. Returns whether
    /// someone was woken.
    fn pass_on(&mut self) -> bool {
        let pick = self
            .waiting
            .iter()
            .rposition(|(_, p)| *p == Priority::Interactive)
            .or_else(|| self.waiting.len().checked_sub(1));
        match pick {
            Some(i) => {
                let (next, _) = self.waiting.remove(i);
                self.granted = Some(next); // `busy` stays set: nobody can jump the line
                true
            }
            None => {
                self.busy = false;
                false
            }
        }
    }
}

pub struct Gate {
    cfg: GateConfig,
    state: Mutex<State>,
    turn: Condvar,
}

/// Held while a run is in progress; dropping it ends the run.
struct Permit<'a> {
    gate: &'a Gate,
    rate_limited: bool,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let mut st = self.gate.lock();
        st.finished(Instant::now(), self.gate.cfg, self.rate_limited);
        if self.rate_limited && !self.gate.cfg.cooldown.is_zero() {
            log::warn!(
                "yt-dlp was rate limited: new runs are refused for {:?}",
                self.gate.cfg.cooldown
            );
        }
        self.gate.release(st);
    }
}

impl Gate {
    pub fn new(cfg: GateConfig) -> Self {
        Self {
            cfg,
            state: Mutex::new(State::default()),
            turn: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn release(&self, mut st: MutexGuard<'_, State>) {
        let woke = st.pass_on();
        drop(st);
        if woke {
            self.turn.notify_all();
        }
    }

    fn refuse(what: &str, left: Duration) -> SourceError {
        log::info!("yt-dlp {what} refused: rate-limit cooldown, {left:?} left");
        SourceError::RateLimited
    }

    /// Runs `f` (one yt-dlp run) when it is this caller's turn. During a cooldown it returns
    /// `RateLimited` at once without calling `f`. If `f` itself returns `RateLimited`, the
    /// cooldown starts. `what` names the operation in logs (never put URLs or secrets in it).
    pub fn run<T>(
        &self,
        what: &str,
        f: impl FnOnce() -> Result<T, SourceError>,
    ) -> Result<T, SourceError> {
        let mut permit = self.enter(what)?;
        let out = f();
        permit.rate_limited = matches!(out, Err(SourceError::RateLimited));
        out
    }

    fn enter(&self, what: &str) -> Result<Permit<'_>, SourceError> {
        let queued_at = Instant::now();
        let mut st = self.lock();
        if let Some(left) = st.cooling(queued_at) {
            return Err(Self::refuse(what, left));
        }
        let mut ahead = 0;
        if st.busy {
            let me = st.next_ticket;
            st.next_ticket += 1;
            st.waiting.push((me, current_priority()));
            ahead = st.waiting.len();
            while st.granted != Some(me) {
                st = self.turn.wait(st).unwrap_or_else(PoisonError::into_inner);
            }
            st.granted = None;
        } else {
            st.busy = true;
        }
        // We own the slot. A rate limit may have been answered while we waited: do not start
        // another run into it, pass the slot on (the next waiter is refused the same way).
        if let Some(left) = st.cooling(Instant::now()) {
            self.release(st);
            return Err(Self::refuse(what, left));
        }
        let pause = st.pause(Instant::now());
        drop(st);
        if !pause.is_zero() {
            thread::sleep(pause);
        }
        let waited = queued_at.elapsed();
        if waited >= LOG_WAITS_OVER {
            log::info!(
                "yt-dlp {what} waited {waited:?} for its turn ({ahead} in line when it queued)"
            );
        }
        Ok(Permit {
            gate: self,
            rate_limited: false,
        })
    }

    /// Callers currently in line (for tests).
    #[cfg(test)]
    fn queued(&self) -> usize {
        self.lock().waiting.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn cfg(gap_ms: u64, cooldown_ms: u64) -> GateConfig {
        GateConfig {
            min_gap: Duration::from_millis(gap_ms),
            cooldown: Duration::from_millis(cooldown_ms),
        }
    }

    // ---- the pacing rules, with made-up clocks: no sleeping ---------------------------------

    #[test]
    fn a_rate_limit_starts_a_cooldown_that_ends_on_time() {
        let t0 = Instant::now();
        let c = cfg(1_000, 120_000);
        let mut st = State::default();
        assert_eq!(st.cooling(t0), None);
        st.finished(t0, c, true);
        assert_eq!(
            st.cooling(t0 + Duration::from_secs(30)),
            Some(Duration::from_secs(90))
        );
        assert_eq!(st.cooling(t0 + Duration::from_secs(120)), None);
        assert_eq!(st.cooling(t0 + Duration::from_secs(500)), None);
    }

    #[test]
    fn an_ordinary_run_leaves_only_the_short_gap() {
        let t0 = Instant::now();
        let mut st = State::default();
        st.finished(t0, cfg(1_000, 120_000), false);
        assert_eq!(st.cooling(t0), None, "no cooldown without a rate limit");
        assert_eq!(st.pause(t0), Duration::from_secs(1));
        assert_eq!(
            st.pause(t0 + Duration::from_millis(400)),
            Duration::from_millis(600)
        );
        assert_eq!(st.pause(t0 + Duration::from_secs(5)), Duration::ZERO);
    }

    #[test]
    fn a_zero_cooldown_means_no_cooldown() {
        let t0 = Instant::now();
        let mut st = State::default();
        st.finished(t0, GateConfig::NONE, true);
        assert_eq!(st.cooling(t0), None);
    }

    #[test]
    fn the_newest_waiter_gets_the_slot_first_and_the_slot_frees_when_nobody_waits() {
        let mut st = State {
            busy: true,
            waiting: vec![
                (4, Priority::Interactive),
                (7, Priority::Interactive),
                (9, Priority::Interactive),
            ],
            ..State::default()
        };
        assert!(st.pass_on());
        assert_eq!(st.granted, Some(9));
        assert!(st.busy, "the slot is handed over, never freed in between");
        st.granted = None;
        assert!(st.pass_on() && st.pass_on());
        assert_eq!(st.waiting, Vec::<(u64, Priority)>::new());
        assert!(!st.pass_on());
        assert!(!st.busy);
    }

    #[test]
    fn interactive_waiters_go_before_background_ones_even_when_the_background_one_is_newer() {
        let mut st = State {
            busy: true,
            waiting: vec![
                (1, Priority::Background),
                (2, Priority::Interactive),
                (3, Priority::Background),
                (4, Priority::Background),
            ],
            ..State::default()
        };
        assert!(st.pass_on());
        assert_eq!(st.granted, Some(2), "the person's own request first");
        st.granted = None;
        assert!(st.pass_on());
        assert_eq!(
            st.granted,
            Some(4),
            "then background work, newest first as before"
        );
        st.granted = None;
        assert!(st.pass_on() && st.pass_on());
        assert!(st.waiting.is_empty());
    }

    #[test]
    fn background_marks_only_the_closure_and_restores_the_previous_priority() {
        assert_eq!(current_priority(), Priority::Interactive);
        background(|| {
            assert_eq!(current_priority(), Priority::Background);
            background(|| assert_eq!(current_priority(), Priority::Background));
            assert_eq!(current_priority(), Priority::Background);
        });
        assert_eq!(current_priority(), Priority::Interactive);
    }

    // ---- the gate itself, with real threads and no pacing -----------------------------------

    #[test]
    fn runs_never_overlap() {
        let gate = Arc::new(Gate::new(GateConfig::NONE));
        let (now, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (gate, now, peak) = (gate.clone(), now.clone(), peak.clone());
                thread::spawn(move || {
                    gate.run("test", || {
                        let n = now.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(n, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(5));
                        now.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1, "two yt-dlp runs overlapped");
    }

    #[test]
    fn while_a_run_is_busy_the_latest_request_is_served_first() {
        let gate = Arc::new(Gate::new(GateConfig::NONE));
        let order = Arc::new(Mutex::new(Vec::new()));
        let (hold_tx, hold_rx) = std::sync::mpsc::channel::<()>();
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let first = {
            let gate = gate.clone();
            thread::spawn(move || {
                gate.run("first", || {
                    started_tx.send(()).unwrap();
                    hold_rx.recv().unwrap(); // hold the slot until told
                    Ok(())
                })
            })
        };
        started_rx.recv().unwrap();
        let mut waiters = Vec::new();
        for n in 1..=3 {
            let (gate2, order) = (gate.clone(), order.clone());
            waiters.push(thread::spawn(move || {
                gate2.run("later", || {
                    order.lock().unwrap().push(n);
                    Ok(())
                })
            }));
            while gate.queued() < n {
                thread::yield_now(); // be sure this one is in line before the next arrives
            }
        }
        hold_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        for w in waiters {
            w.join().unwrap().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn a_rate_limit_refuses_the_rest_of_the_line_without_running_them() {
        let gate = Arc::new(Gate::new(cfg(0, 60_000)));
        let ran = Arc::new(AtomicUsize::new(0));
        let (hold_tx, hold_rx) = std::sync::mpsc::channel::<()>();
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let limited = {
            let (gate, ran) = (gate.clone(), ran.clone());
            thread::spawn(move || {
                gate.run("limited", || {
                    ran.fetch_add(1, Ordering::SeqCst);
                    started_tx.send(()).unwrap();
                    hold_rx.recv().unwrap();
                    Err::<(), _>(SourceError::RateLimited)
                })
            })
        };
        started_rx.recv().unwrap();
        let queued: Vec<_> = (1..=3)
            .map(|n| {
                let (gate2, ran) = (gate.clone(), ran.clone());
                let h = thread::spawn(move || {
                    gate2.run("queued", || {
                        ran.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    })
                });
                while gate.queued() < n {
                    thread::yield_now();
                }
                h
            })
            .collect();
        hold_tx.send(()).unwrap();
        assert_eq!(limited.join().unwrap(), Err(SourceError::RateLimited));
        for h in queued {
            assert_eq!(h.join().unwrap(), Err(SourceError::RateLimited));
        }
        assert_eq!(
            ran.load(Ordering::SeqCst),
            1,
            "only the limited run touched the server"
        );
        // And a caller arriving later is refused immediately too.
        assert_eq!(gate.run("late", || Ok(())), Err(SourceError::RateLimited));
    }

    #[test]
    fn the_quiet_gap_is_kept_between_runs() {
        let gate = Gate::new(cfg(80, 0));
        gate.run("a", || Ok(())).unwrap();
        let t = Instant::now();
        gate.run("b", || Ok(())).unwrap();
        assert!(
            t.elapsed() >= Duration::from_millis(80),
            "{:?}",
            t.elapsed()
        );
    }

    #[test]
    fn a_panicking_run_does_not_wedge_the_gate() {
        let gate = Arc::new(Gate::new(GateConfig::NONE));
        let g = gate.clone();
        let r = thread::spawn(move || g.run::<()>("boom", || panic!("tool blew up"))).join();
        assert!(r.is_err());
        assert_eq!(gate.run("after", || Ok(5)), Ok(5));
    }
}
