//! Audio output recovery: what to do when the output device goes away, comes back, or the default device changes
//! (Bluetooth earbuds switched off and on, headphones unplugged, the user picks another output). Pure: time and
//! observations are passed in, actions come out, so every case is unit tested. The app (`runner.rs`) owns the real
//! stream, polls the default device, and carries the actions out.
//!
//! The rules:
//! - Any stream error, a default device that is a different one than the stream runs on, or output that has stopped
//!   pulling audio for a while counts as a problem. Exactly one "lost" line is logged per outage, not one per retry.
//! - While playing, the player is paused (so the UI says so instead of pretending to play) and one notice is shown.
//! - The stream is rebuilt on the default device at most once per second until it works.
//! - When it works again, the player resumes only if *we* paused it and nobody has touched playback since.

/// How often a lost output is retried.
pub const RETRY_MS: u64 = 1_000;
/// Playing, but the output asked for no audio for this long: the device is gone without having said so.
pub const STALL_MS: u64 = 3_000;
/// A failure to reopen the output is logged again at most this often (and at once when the reason changes).
pub const REPEAT_LOG_MS: u64 = 60_000;

/// What the listener's own actions did to playback (anything but our own pause).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserEffect {
    /// Resume, a new song, a seek, stop: the listener has taken over, so we must not resume on their behalf.
    TookOver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open the default output device now and move playback onto it (`reason` is for the log).
    Rebuild { reason: String },
    /// Pause the player: nothing can be heard.
    PausePlayer,
    /// Resume the player: it was paused only because the device was lost.
    ResumePlayer,
    /// A message for the listener.
    Notice(String),
    /// A log line (the caller picks the level: `warn` when `warn`, else info).
    Log { warn: bool, text: String },
}

/// What the app sees on each check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub now_ms: u64,
    /// Name of the system default output device; `None` when there is no output device at all.
    pub default_device: Option<String>,
    /// How many times the output callback has run since the stream started.
    pub callbacks: u64,
    /// The player is playing (not paused, something is loaded).
    pub playing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Running,
    Lost { last_try_ms: Option<u64> },
}

#[derive(Debug, Clone)]
pub struct DeviceWatch {
    state: State,
    /// The device the stream runs on.
    active: Option<String>,
    last_callbacks: u64,
    last_progress_ms: u64,
    /// We paused the player because the device was lost.
    auto_paused: bool,
    last_fail_log: Option<(String, u64)>,
}

impl DeviceWatch {
    /// `device`: the device the stream was just opened on; `now_ms`: now.
    pub fn new(device: Option<String>, now_ms: u64) -> Self {
        Self {
            state: if device.is_some() {
                State::Running
            } else {
                State::Lost { last_try_ms: None }
            },
            active: device,
            last_callbacks: 0,
            last_progress_ms: now_ms,
            auto_paused: false,
            last_fail_log: None,
        }
    }

    pub fn is_lost(&self) -> bool {
        matches!(self.state, State::Lost { .. })
    }

    fn lose(&mut self, why: &str, playing: bool) -> Vec<Action> {
        if self.is_lost() {
            return Vec::new();
        }
        self.state = State::Lost { last_try_ms: None };
        let mut out = vec![Action::Log {
            warn: true,
            text: format!(
                "audio device lost: {why} (was {})",
                self.active.as_deref().unwrap_or("none")
            ),
        }];
        if playing && !self.auto_paused {
            self.auto_paused = true;
            out.push(Action::PausePlayer);
        }
        out.push(Action::Notice(
            "The audio device was disconnected. Playback continues when it is back.".into(),
        ));
        out
    }

    /// The stream's error callback fired (the device was removed or the stream broke).
    pub fn on_stream_error(&mut self, text: &str, playing: bool) -> Vec<Action> {
        self.lose(text, playing)
    }

    /// The listener did something to playback.
    pub fn on_user(&mut self, _e: UserEffect) {
        self.auto_paused = false;
    }

    /// Call about every 500 ms.
    pub fn tick(&mut self, o: &Observation) -> Vec<Action> {
        match self.state.clone() {
            State::Running => {
                if o.callbacks != self.last_callbacks || !o.playing {
                    self.last_callbacks = o.callbacks;
                    self.last_progress_ms = o.now_ms;
                }
                match (&o.default_device, &self.active) {
                    (None, _) => self.lose("no output device is available", o.playing),
                    (Some(now), Some(was)) if now != was => vec![Action::Rebuild {
                        reason: format!("the default output changed from {was} to {now}"),
                    }],
                    _ if o.playing
                        && o.now_ms.saturating_sub(self.last_progress_ms) >= STALL_MS =>
                    {
                        self.lose("the output stopped asking for audio", o.playing)
                    }
                    _ => Vec::new(),
                }
            }
            State::Lost { last_try_ms } => {
                if o.default_device.is_none() {
                    return Vec::new();
                }
                if last_try_ms.is_some_and(|t| o.now_ms.saturating_sub(t) < RETRY_MS) {
                    return Vec::new();
                }
                self.state = State::Lost {
                    last_try_ms: Some(o.now_ms),
                };
                vec![Action::Rebuild {
                    reason: "trying the default output again".into(),
                }]
            }
        }
    }

    /// The stream now runs on `device`.
    pub fn rebuilt(&mut self, device: &str, rate: u32, now_ms: u64) -> Vec<Action> {
        let was_lost = self.is_lost();
        self.state = State::Running;
        self.active = Some(device.to_string());
        self.last_callbacks = 0;
        self.last_progress_ms = now_ms;
        self.last_fail_log = None;
        let mut out = vec![Action::Log {
            warn: false,
            text: if was_lost {
                format!("audio device recovered: {device} at {rate} Hz")
            } else {
                format!("audio output moved to {device} at {rate} Hz")
            },
        }];
        if was_lost {
            out.push(Action::Notice(format!("Audio is back on {device}.")));
        }
        if self.auto_paused {
            self.auto_paused = false;
            out.push(Action::ResumePlayer);
        }
        out
    }

    /// Opening the default output failed.
    pub fn rebuild_failed(&mut self, error: &str, now_ms: u64, playing: bool) -> Vec<Action> {
        let mut out = self.lose(error, playing);
        // The device is still lost: a retry is due in RETRY_MS. Log the reason once, then rarely.
        if !matches!(self.state, State::Lost { .. }) {
            return out;
        }
        self.state = State::Lost {
            last_try_ms: Some(now_ms),
        };
        let repeat = match &self.last_fail_log {
            Some((text, at)) => text != error || now_ms.saturating_sub(*at) >= REPEAT_LOG_MS,
            None => true,
        };
        if repeat && out.iter().all(|a| !matches!(a, Action::Log { .. })) {
            out.push(Action::Log {
                warn: true,
                text: format!("audio output still unavailable: {error}"),
            });
        }
        if repeat {
            self.last_fail_log = Some((error.to_string(), now_ms));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(now_ms: u64, device: Option<&str>, callbacks: u64, playing: bool) -> Observation {
        Observation {
            now_ms,
            default_device: device.map(str::to_string),
            callbacks,
            playing,
        }
    }
    fn logs(a: &[Action]) -> usize {
        a.iter().filter(|a| matches!(a, Action::Log { .. })).count()
    }
    fn watch() -> DeviceWatch {
        DeviceWatch::new(Some("Earbuds".into()), 0)
    }

    #[test]
    fn a_healthy_stream_needs_nothing() {
        let mut w = watch();
        for i in 1..20u64 {
            assert!(w
                .tick(&obs(i * 500, Some("Earbuds"), i * 50, true))
                .is_empty());
        }
    }

    #[test]
    fn a_stream_error_while_playing_pauses_once_and_says_so_once() {
        let mut w = watch();
        let a = w.on_stream_error("device not available", true);
        assert_eq!(a.iter().filter(|a| **a == Action::PausePlayer).count(), 1);
        assert_eq!(logs(&a), 1);
        assert!(a
            .iter()
            .any(|a| matches!(a, Action::Notice(m) if m.contains("disconnected"))));
        // cpal calls the error callback again; nothing more happens.
        assert!(w.on_stream_error("device not available", true).is_empty());
        assert!(w.is_lost());
    }

    #[test]
    fn a_stream_error_while_paused_does_not_touch_the_player() {
        let mut w = watch();
        let a = w.on_stream_error("gone", false);
        assert!(!a.contains(&Action::PausePlayer));
        let back = w.rebuilt("Earbuds", 48_000, 5_000);
        assert!(!back.contains(&Action::ResumePlayer), "it was not playing");
    }

    #[test]
    fn retries_once_a_second_without_a_log_storm() {
        let mut w = watch();
        w.on_stream_error("gone", true);
        let mut rebuilds = 0;
        let mut log_lines = 0;
        for i in 0..40u64 {
            let now = 1_000 + i * 500;
            for a in w.tick(&obs(now, Some("Speakers"), 0, false)) {
                if matches!(a, Action::Rebuild { .. }) {
                    rebuilds += 1;
                    log_lines += logs(&w.rebuild_failed("cannot open", now, false));
                }
            }
        }
        assert_eq!(
            rebuilds, 20,
            "20 s of ticks (every 500 ms), one try per second"
        );
        assert_eq!(log_lines, 1, "the failure is logged once, not every second");
    }

    #[test]
    fn a_different_failure_reason_is_logged_again() {
        let mut w = watch();
        w.on_stream_error("gone", true);
        assert_eq!(
            logs(&w.rebuild_failed("format not supported", 1_000, false)),
            1
        );
        assert_eq!(
            logs(&w.rebuild_failed("format not supported", 2_000, false)),
            0
        );
        assert_eq!(logs(&w.rebuild_failed("access denied", 3_000, false)), 1);
        assert_eq!(
            logs(&w.rebuild_failed("access denied", 3_000 + REPEAT_LOG_MS, false)),
            1
        );
    }

    #[test]
    fn nothing_is_tried_while_there_is_no_device_at_all() {
        let mut w = watch();
        w.on_stream_error("gone", true);
        for i in 1..10u64 {
            assert!(w.tick(&obs(i * 1_000, None, 0, false)).is_empty());
        }
        assert!(!w.tick(&obs(10_000, Some("Speakers"), 0, false)).is_empty());
    }

    #[test]
    fn recovery_resumes_what_we_paused_and_says_one_line() {
        let mut w = watch();
        w.on_stream_error("gone", true);
        let a = w.rebuilt("Earbuds", 44_100, 8_000);
        assert!(a.contains(&Action::ResumePlayer));
        assert_eq!(logs(&a), 1);
        assert!(
            matches!(&a[0], Action::Log { warn: false, text } if text.contains("recovered") && text.contains("44100"))
        );
        assert!(!w.is_lost());
        // A second recovery without a new loss resumes nothing.
        assert!(!w
            .rebuilt("Earbuds", 44_100, 9_000)
            .contains(&Action::ResumePlayer));
    }

    #[test]
    fn if_the_listener_took_over_we_do_not_resume_for_them() {
        let mut w = watch();
        w.on_stream_error("gone", true);
        w.on_user(UserEffect::TookOver); // they hit play, picked another song or pressed stop
        assert!(!w
            .rebuilt("Earbuds", 48_000, 4_000)
            .contains(&Action::ResumePlayer));
    }

    #[test]
    fn a_new_default_device_is_followed_without_a_loss() {
        let mut w = watch();
        let a = w.tick(&obs(500, Some("Speakers"), 10, true));
        assert!(
            matches!(&a[..], [Action::Rebuild { reason }] if reason.contains("Earbuds") && reason.contains("Speakers"))
        );
        let done = w.rebuilt("Speakers", 48_000, 600);
        assert!(done
            .iter()
            .all(|a| !matches!(a, Action::Notice(_) | Action::ResumePlayer)));
        assert!(w.tick(&obs(1_100, Some("Speakers"), 20, true)).is_empty());
    }

    #[test]
    fn the_device_vanishing_from_the_system_is_a_loss() {
        let mut w = watch();
        let a = w.tick(&obs(500, None, 10, true));
        assert!(a.contains(&Action::PausePlayer));
        assert!(w.is_lost());
    }

    #[test]
    fn output_that_silently_stops_pulling_is_caught_after_three_seconds() {
        let mut w = watch();
        assert!(w.tick(&obs(500, Some("Earbuds"), 40, true)).is_empty());
        assert!(w.tick(&obs(2_000, Some("Earbuds"), 40, true)).is_empty());
        let a = w.tick(&obs(500 + STALL_MS, Some("Earbuds"), 40, true));
        assert!(a.contains(&Action::PausePlayer), "{a:?}");
    }

    #[test]
    fn a_paused_player_is_never_mistaken_for_a_stalled_device() {
        let mut w = watch();
        for i in 1..30u64 {
            assert!(w
                .tick(&obs(i * 1_000, Some("Earbuds"), 0, false))
                .is_empty());
        }
        // ...and playing again starts the stall clock from now, not from before.
        assert!(w.tick(&obs(31_000, Some("Earbuds"), 0, true)).is_empty());
    }

    #[test]
    fn starting_with_no_device_waits_for_one() {
        let mut w = DeviceWatch::new(None, 0);
        assert!(w.is_lost());
        assert!(matches!(
            &w.tick(&obs(0, Some("Speakers"), 0, false))[..],
            [Action::Rebuild { .. }]
        ));
    }
}
