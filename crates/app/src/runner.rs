//! Threads that drive an engine: the real one, a "no audio device" stand-in that fails honestly,
//! and the simulator (`PYXEE_SIM=1`) for UI work without audio.

use crate::engine::{Engine, Host};
use crate::recorder::HistoryRecorder;
use crate::sim::{catalog, Sim, SimOut, TICK_MS};
use crate::state::{now_ms, publish, with_library, AppState};
use crate::tools;
use pyxee_audio::output::DeviceInfo;
use pyxee_core::{Command, Effect, EngineEvent, Failure};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const SAVE_EVERY: Duration = Duration::from_secs(5);

struct AppHost {
    app: AppHandle,
    recorder: Mutex<HistoryRecorder>,
}

impl AppHost {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            recorder: Mutex::new(HistoryRecorder::default()),
        }
    }

    /// Feeds listening into the history: what became audible, and how long it has been heard.
    fn record(&self, e: &EngineEvent) {
        let Ok(mut rec) = self.recorder.lock() else {
            return;
        };
        match e {
            EngineEvent::Audible { id } => {
                let track = self
                    .app
                    .state::<AppState>()
                    .player
                    .lock()
                    .ok()
                    .and_then(|p| p.queue().get(*id).map(|entry| entry.track.clone()));
                if let Some(track) = track {
                    rec.audible(*id, track);
                }
            }
            EngineEvent::Position { id, ms } => {
                if let Some(track) = rec.position(*id, *ms) {
                    drop(rec);
                    if let Err(err) = with_library(&self.app, |l| l.record_play(&track, now_ms())) {
                        eprintln!("could not record play: {err}");
                    }
                }
            }
            _ => {}
        }
    }
}

impl Host for AppHost {
    fn engine_event(&self, e: EngineEvent) {
        publish(&self.app, |p| p.engine(e.clone()));
        self.record(&e);
    }
    fn command(&self, c: Command) {
        publish(&self.app, |p| p.command(c));
    }
}

pub fn spawn(app: AppHandle, rx: Receiver<Effect>) {
    if std::env::var_os("PYXEE_SIM").is_some() {
        spawn_sim(app, rx);
    } else {
        spawn_real(app, rx);
    }
}

fn spawn_real(app: AppHandle, rx: Receiver<Effect>) {
    thread::spawn(move || {
        let host = Arc::new(AppHost::new(app.clone()));
        let source = Arc::clone(&app.state::<AppState>().source);
        let started = DeviceInfo::default_output().and_then(|device| {
            let engine = Engine::new(
                host.clone(),
                source,
                tools::locate("ffmpeg", "PYXEE_FFMPEG"),
                device.rate(),
            );
            let output = device.start(engine.consumer())?;
            Ok((engine, output))
        });
        match started {
            // `_output` keeps the audio stream alive on this thread.
            Ok((mut engine, _output)) => {
                let mut last_save = Instant::now();
                loop {
                    match rx.recv_timeout(Duration::from_millis(20)) {
                        Ok(effect) => engine.handle(effect),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    engine.poll();
                    if last_save.elapsed() >= SAVE_EVERY {
                        last_save = Instant::now();
                        app.state::<AppState>().save_session();
                    }
                }
            }
            Err(e) => {
                // No usable output: every play request fails with the reason instead of pretending to play.
                let message = e.to_string();
                for effect in rx {
                    if let Effect::StartTrack { id, .. } = effect {
                        host.engine_event(EngineEvent::LoadFailed {
                            id,
                            failure: Failure::Other(message.clone()),
                        });
                    }
                }
            }
        }
    });
}

fn spawn_sim(app: AppHandle, rx: Receiver<Effect>) {
    thread::spawn(move || {
        let mut sim = Sim::new(catalog());
        let tick = Duration::from_millis(TICK_MS);
        let mut next_tick = Instant::now() + tick;
        let mut last_save = Instant::now();
        loop {
            match rx.recv_timeout(next_tick.saturating_duration_since(Instant::now())) {
                Ok(effect) => sim.apply(&effect),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    next_tick += tick;
                    for out in sim.tick(TICK_MS) {
                        match out {
                            SimOut::Engine(e) => publish(&app, |p| p.engine(e)),
                            SimOut::Radio(t) => {
                                publish(&app, |p| p.command(Command::AppendRadio(t)))
                            }
                        }
                    }
                    if last_save.elapsed() >= SAVE_EVERY {
                        last_save = Instant::now();
                        app.state::<AppState>().save_session();
                    }
                }
            }
        }
    });
}
