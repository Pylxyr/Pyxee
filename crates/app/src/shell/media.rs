//! Media keys, the volume flyout and the lock-screen tile: Windows' System Media Transport Controls through
//! `souvlaki`. (On other systems the same crate speaks MPRIS, which is how this file is compile-checked.)

use crate::dto::TrackDto;
use pyxee_core::Command;
use pyxee_shell::progress::Phase;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub struct Media {
    controls: MediaControls,
    duration: Option<Duration>,
}

pub fn init(app: &AppHandle) -> Option<Media> {
    #[cfg(windows)]
    #[allow(clippy::unnecessary_cast)]
    let hwnd = app
        .get_webview_window(super::MAIN)
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as *mut std::ffi::c_void);
    #[cfg(not(windows))]
    let hwnd = None;
    let config = PlatformConfig {
        dbus_name: "dev.pyxee.app",
        display_name: "Pyxee",
        hwnd,
    };
    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("media keys are not available: {e:?}");
            return None;
        }
    };
    let app = app.clone();
    if let Err(e) = controls.attach(move |event| on_event(&app, event)) {
        log::warn!("could not attach the media key handler: {e:?}");
        return None;
    }
    log::info!("media controls attached");
    Some(Media {
        controls,
        duration: None,
    })
}

/// Which way and how far, as a new position.
fn seek_target(position_ms: u64, dir: SeekDirection, by_ms: u64) -> u64 {
    match dir {
        SeekDirection::Forward => position_ms.saturating_add(by_ms),
        SeekDirection::Backward => position_ms.saturating_sub(by_ms),
    }
}

fn on_event(app: &AppHandle, event: MediaControlEvent) {
    let position = || {
        app.state::<crate::state::AppState>()
            .player
            .lock()
            .map_or(0, |p| crate::dto::state_dto(&p).position_ms)
    };
    let cmd = match event {
        MediaControlEvent::Play => Command::Resume,
        MediaControlEvent::Pause | MediaControlEvent::Stop => Command::Pause,
        MediaControlEvent::Toggle => Command::Toggle,
        MediaControlEvent::Next => Command::Next,
        MediaControlEvent::Previous => Command::Previous,
        MediaControlEvent::Seek(dir) => Command::Seek(seek_target(position(), dir, 10_000)),
        MediaControlEvent::SeekBy(dir, by) => {
            Command::Seek(seek_target(position(), dir, by.as_millis() as u64))
        }
        MediaControlEvent::SetPosition(MediaPosition(to)) => Command::Seek(to.as_millis() as u64),
        _ => return,
    };
    super::command(app, cmd);
}

impl Media {
    pub fn set_track(&mut self, t: &TrackDto) {
        self.duration = t.duration_ms.map(Duration::from_millis);
        let r = self.controls.set_metadata(MediaMetadata {
            title: Some(&t.title),
            artist: Some(&t.artist),
            album: None,
            cover_url: t.thumbnail.as_deref(),
            duration: self.duration,
        });
        if let Err(e) = r {
            log::debug!("media metadata not set: {e:?}");
        }
    }

    pub fn set_playback(&mut self, phase: Phase, position_ms: u64, has_track: bool) {
        let progress = has_track.then(|| MediaPosition(Duration::from_millis(position_ms)));
        let state = match phase {
            Phase::Playing => MediaPlayback::Playing { progress },
            Phase::Paused | Phase::Loading | Phase::Failed if has_track => {
                MediaPlayback::Paused { progress }
            }
            _ => MediaPlayback::Stopped,
        };
        if let Err(e) = self.controls.set_playback(state) {
            log::debug!("media playback state not set: {e:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeking_by_the_media_keys_never_goes_below_zero_or_overflows() {
        assert_eq!(seek_target(30_000, SeekDirection::Forward, 10_000), 40_000);
        assert_eq!(seek_target(30_000, SeekDirection::Backward, 10_000), 20_000);
        assert_eq!(seek_target(3_000, SeekDirection::Backward, 10_000), 0);
        assert_eq!(
            seek_target(u64::MAX - 1, SeekDirection::Forward, 10_000),
            u64::MAX
        );
    }
}
