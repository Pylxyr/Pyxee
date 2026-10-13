//! Sleep timer: pause after a time, at the end of the current song, or after N songs. Time is passed in, never read.

use serde::{Deserialize, Serialize};

/// The last stretch of a timed sleep fades the volume out instead of cutting it off.
pub const FADE_MS: u64 = 15_000;
pub const MAX_MINUTES: u32 = 12 * 60;
pub const MAX_TRACKS: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SleepSetting {
    Minutes { minutes: u32 },
    EndOfTrack,
    Tracks { count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Active {
    At { deadline_ms: u64 },
    EndOfTrack,
    Tracks { left: u32 },
}

/// What the UI shows while a timer runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SleepStatus {
    Minutes { remaining_ms: u64 },
    EndOfTrack,
    Tracks { left: u32 },
}

/// Result of a clock tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    /// Multiplier for the volume (1.0 = unchanged) while fading out.
    pub volume_factor: f32,
    /// The time is up: pause now (and put the volume factor back to 1.0 afterwards).
    pub fire: bool,
}

#[derive(Debug, Default)]
pub struct SleepTimer {
    active: Option<Active>,
}

impl SleepTimer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts (or replaces) the timer. Out-of-range values are clamped; zero minutes or tracks cancels.
    pub fn set(&mut self, s: SleepSetting, now_ms: u64) {
        self.active = match s {
            SleepSetting::Minutes { minutes } if minutes > 0 => Some(Active::At {
                deadline_ms: now_ms + u64::from(minutes.min(MAX_MINUTES)) * 60_000,
            }),
            SleepSetting::Tracks { count } if count > 0 => Some(Active::Tracks {
                left: count.min(MAX_TRACKS),
            }),
            SleepSetting::EndOfTrack => Some(Active::EndOfTrack),
            _ => None,
        };
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }

    /// "+15 minutes" on a running time timer; starts one if nothing runs; ignored for song-based timers.
    pub fn extend_minutes(&mut self, minutes: u32, now_ms: u64) {
        match self.active {
            Some(Active::At { deadline_ms }) => {
                let max = now_ms + u64::from(MAX_MINUTES) * 60_000;
                self.active = Some(Active::At {
                    deadline_ms: (deadline_ms + u64::from(minutes) * 60_000).min(max),
                });
            }
            None => self.set(SleepSetting::Minutes { minutes }, now_ms),
            Some(_) => {}
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// True while the player must not start another song after this one (the core's "stop after current").
    pub fn stop_after_current(&self) -> bool {
        matches!(
            self.active,
            Some(Active::EndOfTrack | Active::Tracks { left: 1 })
        )
    }

    pub fn status(&self, now_ms: u64) -> Option<SleepStatus> {
        self.active.map(|a| match a {
            Active::At { deadline_ms } => SleepStatus::Minutes {
                remaining_ms: deadline_ms.saturating_sub(now_ms),
            },
            Active::EndOfTrack => SleepStatus::EndOfTrack,
            Active::Tracks { left } => SleepStatus::Tracks { left },
        })
    }

    /// Call about once a second. Returns `None` when no timed timer is running.
    pub fn tick(&mut self, now_ms: u64) -> Option<Tick> {
        let Some(Active::At { deadline_ms }) = self.active else {
            return None;
        };
        let remaining = deadline_ms.saturating_sub(now_ms);
        if remaining == 0 {
            self.active = None;
            return Some(Tick {
                volume_factor: 1.0,
                fire: true,
            });
        }
        let factor = if remaining < FADE_MS {
            remaining as f32 / FADE_MS as f32
        } else {
            1.0
        };
        Some(Tick {
            volume_factor: factor,
            fire: false,
        })
    }

    /// A song played to its end. Returns true when the timer is now done and playback must stay paused.
    pub fn on_track_finished(&mut self) -> bool {
        match self.active {
            Some(Active::EndOfTrack) | Some(Active::Tracks { left: 1 }) => {
                self.active = None;
                true
            }
            Some(Active::Tracks { left }) => {
                self.active = Some(Active::Tracks { left: left - 1 });
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_timer_counts_down_fades_in_the_last_seconds_and_fires_once() {
        let mut t = SleepTimer::new();
        t.set(SleepSetting::Minutes { minutes: 1 }, 1_000);
        assert_eq!(
            t.status(1_000),
            Some(SleepStatus::Minutes {
                remaining_ms: 60_000
            })
        );
        assert_eq!(
            t.tick(30_000),
            Some(Tick {
                volume_factor: 1.0,
                fire: false
            })
        );
        let fading = t.tick(1_000 + 60_000 - 7_500).unwrap();
        assert!((fading.volume_factor - 0.5).abs() < 1e-6 && !fading.fire);
        let end = t.tick(61_000).unwrap();
        assert!(end.fire);
        assert!(!t.is_active(), "it fires once and is done");
        assert_eq!(t.tick(62_000), None);
    }

    #[test]
    fn a_late_tick_after_the_deadline_still_fires() {
        let mut t = SleepTimer::new();
        t.set(SleepSetting::Minutes { minutes: 5 }, 0);
        assert!(
            t.tick(10 * 60_000).unwrap().fire,
            "e.g. the PC slept past the deadline"
        );
    }

    #[test]
    fn end_of_track_waits_for_the_song_to_finish_and_blocks_the_next_one() {
        let mut t = SleepTimer::new();
        t.set(SleepSetting::EndOfTrack, 0);
        assert!(t.stop_after_current());
        assert_eq!(t.tick(5_000), None, "no clock involved");
        assert!(t.on_track_finished());
        assert!(!t.is_active() && !t.stop_after_current());
    }

    #[test]
    fn n_tracks_counts_finished_songs_and_blocks_only_after_the_last() {
        let mut t = SleepTimer::new();
        t.set(SleepSetting::Tracks { count: 3 }, 0);
        assert!(!t.stop_after_current());
        assert!(!t.on_track_finished());
        assert!(!t.on_track_finished());
        assert!(t.stop_after_current(), "the third is the last one");
        assert_eq!(t.status(0), Some(SleepStatus::Tracks { left: 1 }));
        assert!(t.on_track_finished());
        assert!(!t.is_active());
    }

    #[test]
    fn zero_values_cancel_and_huge_values_are_clamped() {
        let mut t = SleepTimer::new();
        t.set(SleepSetting::Minutes { minutes: 10 }, 0);
        t.set(SleepSetting::Minutes { minutes: 0 }, 0);
        assert!(!t.is_active());
        t.set(SleepSetting::Tracks { count: 0 }, 0);
        assert!(!t.is_active());
        t.set(SleepSetting::Minutes { minutes: u32::MAX }, 0);
        assert_eq!(
            t.status(0),
            Some(SleepStatus::Minutes {
                remaining_ms: u64::from(MAX_MINUTES) * 60_000
            })
        );
        t.set(SleepSetting::Tracks { count: 5_000 }, 0);
        assert_eq!(t.status(0), Some(SleepStatus::Tracks { left: MAX_TRACKS }));
    }

    #[test]
    fn plus_minutes_extends_a_running_timer_starts_one_if_none_and_leaves_song_timers_alone() {
        let mut t = SleepTimer::new();
        t.extend_minutes(15, 1_000);
        assert_eq!(
            t.status(1_000),
            Some(SleepStatus::Minutes {
                remaining_ms: 15 * 60_000
            })
        );
        t.extend_minutes(15, 1_000);
        assert_eq!(
            t.status(1_000),
            Some(SleepStatus::Minutes {
                remaining_ms: 30 * 60_000
            })
        );
        t.extend_minutes(100_000, 1_000);
        assert_eq!(
            t.status(1_000),
            Some(SleepStatus::Minutes {
                remaining_ms: u64::from(MAX_MINUTES) * 60_000
            })
        );
        t.set(SleepSetting::EndOfTrack, 0);
        t.extend_minutes(15, 0);
        assert_eq!(t.status(0), Some(SleepStatus::EndOfTrack));
    }

    #[test]
    fn cancelling_stops_everything() {
        let mut t = SleepTimer::new();
        t.set(SleepSetting::EndOfTrack, 0);
        t.cancel();
        assert!(!t.stop_after_current() && !t.on_track_finished());
    }
}
