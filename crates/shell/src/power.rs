//! Sleep/resume and network changes, detected without Windows messages: a clock that jumps more than the monotonic
//! clock means the PC slept; a changed local address or a failed probe means the network changed.

/// Detects "the machine was asleep" from two clocks sampled on a timer.
#[derive(Debug)]
pub struct ResumeDetector {
    last: Option<(u64, u64)>,
    gap_ms: u64,
}

impl ResumeDetector {
    /// `gap_ms`: how much longer than the monotonic clock the wall clock must have advanced (a few seconds
    /// absorbs timer jitter and small clock corrections).
    pub fn new(gap_ms: u64) -> Self {
        Self { last: None, gap_ms }
    }

    /// `wall_ms`: wall clock (jumps with sleep); `mono_ms`: a clock that does not count sleep (on Windows
    /// `Instant` does not advance while asleep). Returns how long the PC was asleep, if it just woke up.
    pub fn tick(&mut self, wall_ms: u64, mono_ms: u64) -> Option<u64> {
        let slept = self.last.and_then(|(w0, m0)| {
            let wall = wall_ms.saturating_sub(w0);
            let mono = mono_ms.saturating_sub(m0);
            (wall > mono + self.gap_ms).then(|| wall - mono)
        });
        self.last = Some((wall_ms, mono_ms));
        slept
    }
}

/// What a probe saw: reachable or not, and the local address used (changes with Wi-Fi/VPN switches).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetState {
    pub online: bool,
    pub local_addr: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetEvent {
    WentOffline,
    CameOnline,
    /// Still online but through another connection.
    Switched,
}

#[derive(Debug, Default)]
pub struct NetworkWatcher {
    last: Option<NetState>,
}

impl NetworkWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// The first observation only sets the baseline.
    pub fn observe(&mut self, now: NetState) -> Option<NetEvent> {
        let ev = match &self.last {
            None => None,
            Some(prev) => match (prev.online, now.online) {
                (true, false) => Some(NetEvent::WentOffline),
                (false, true) => Some(NetEvent::CameOnline),
                (true, true) if prev.local_addr != now.local_addr && now.local_addr.is_some() => {
                    Some(NetEvent::Switched)
                }
                _ => None,
            },
        };
        self.last = Some(now);
        ev
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Resumed,
    Net(NetEvent),
}

/// The player's situation as far as this policy cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situation {
    Playing,
    /// Stopped by a failure that trying again can fix (offline, rate limited, stream refused).
    FailedRetryable,
    /// Anything else: idle, paused, loading, a permanent failure.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Try the current song again from where it stopped.
    Retry,
    Notice(&'static str),
}

/// What to do when the PC wakes or the network changes.
pub fn decide(trigger: Trigger, now: Situation) -> Vec<Action> {
    match (trigger, now) {
        (Trigger::Net(NetEvent::WentOffline), Situation::Playing) => vec![Action::Notice(
            "You are offline. Playback continues until what is buffered runs out.",
        )],
        (Trigger::Net(NetEvent::CameOnline), Situation::FailedRetryable) => {
            vec![Action::Notice("Back online. Trying again."), Action::Retry]
        }
        (Trigger::Resumed, Situation::FailedRetryable)
        | (Trigger::Net(NetEvent::Switched), Situation::FailedRetryable) => {
            vec![Action::Retry]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(online: bool, addr: &str) -> NetState {
        NetState {
            online,
            local_addr: Some(addr.into()),
        }
    }

    #[test]
    fn a_normal_ticking_clock_never_looks_like_a_sleep() {
        let mut d = ResumeDetector::new(5_000);
        assert_eq!(d.tick(1_000_000, 10_000), None);
        for i in 1..100u64 {
            assert_eq!(
                d.tick(1_000_000 + i * 2_000 + 300, 10_000 + i * 2_000),
                None,
                "jitter of 300 ms is fine"
            );
        }
    }

    #[test]
    fn a_wall_clock_jump_is_a_sleep_of_that_length() {
        let mut d = ResumeDetector::new(5_000);
        d.tick(1_000_000, 10_000);
        // 30 minutes asleep: the monotonic clock advanced 2 s, the wall clock 30 min + 2 s.
        assert_eq!(d.tick(1_000_000 + 1_802_000, 12_000), Some(1_800_000));
        // And only once.
        assert_eq!(d.tick(1_000_000 + 1_804_000, 14_000), None);
    }

    #[test]
    fn a_clock_set_backwards_is_not_a_sleep() {
        let mut d = ResumeDetector::new(5_000);
        d.tick(1_000_000, 10_000);
        assert_eq!(d.tick(500_000, 12_000), None);
    }

    #[test]
    fn network_changes_are_reported_once_each() {
        let mut w = NetworkWatcher::new();
        assert_eq!(w.observe(net(true, "192.168.0.5")), None, "baseline");
        assert_eq!(w.observe(net(true, "192.168.0.5")), None);
        assert_eq!(w.observe(net(true, "10.0.0.2")), Some(NetEvent::Switched));
        assert_eq!(
            w.observe(NetState {
                online: false,
                local_addr: None
            }),
            Some(NetEvent::WentOffline)
        );
        assert_eq!(
            w.observe(NetState {
                online: false,
                local_addr: None
            }),
            None
        );
        assert_eq!(w.observe(net(true, "10.0.0.2")), Some(NetEvent::CameOnline));
    }

    #[test]
    fn starting_offline_then_coming_online_is_reported() {
        let mut w = NetworkWatcher::new();
        assert_eq!(
            w.observe(NetState {
                online: false,
                local_addr: None
            }),
            None
        );
        assert_eq!(w.observe(net(true, "1.2.3.4")), Some(NetEvent::CameOnline));
    }

    #[test]
    fn coming_back_online_retries_a_failed_song_and_says_so() {
        let a = decide(
            Trigger::Net(NetEvent::CameOnline),
            Situation::FailedRetryable,
        );
        assert_eq!(
            a,
            vec![Action::Notice("Back online. Trying again."), Action::Retry]
        );
    }

    #[test]
    fn waking_up_retries_only_a_song_that_failed() {
        assert_eq!(
            decide(Trigger::Resumed, Situation::FailedRetryable),
            vec![Action::Retry]
        );
        assert!(decide(Trigger::Resumed, Situation::Playing).is_empty());
        assert!(decide(Trigger::Resumed, Situation::Other).is_empty());
    }

    #[test]
    fn going_offline_while_playing_only_informs() {
        let a = decide(Trigger::Net(NetEvent::WentOffline), Situation::Playing);
        assert!(matches!(a.as_slice(), [Action::Notice(_)]));
        assert!(decide(Trigger::Net(NetEvent::WentOffline), Situation::Other).is_empty());
    }

    #[test]
    fn switching_connections_never_interrupts_a_song_that_is_playing() {
        assert!(decide(Trigger::Net(NetEvent::Switched), Situation::Playing).is_empty());
        assert_eq!(
            decide(Trigger::Net(NetEvent::Switched), Situation::FailedRetryable),
            vec![Action::Retry]
        );
    }
}
