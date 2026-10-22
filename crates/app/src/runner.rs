//! Threads that drive an engine: the real one (which also watches the audio output device and rebuilds the stream
//! when it is lost, comes back or is replaced as the default), and the simulator (`PYXEE_SIM=1`) for UI work without audio.

use crate::engine::{Engine, Host};
use crate::recorder::HistoryRecorder;
use crate::sim::{catalog, Sim, SimOut, TICK_MS};
use crate::state::{now_ms, publish, with_library, AppState};
use pyxee_audio::output::{DeviceInfo, Output};
use pyxee_audio::AudioError;
use pyxee_core::{Command, Effect, EngineEvent, Failure};
use pyxee_shell::audiodev::{Action as DeviceAction, DeviceWatch, Observation, UserEffect};
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
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
                        log::warn!("could not record play: {err}");
                    }
                }
            }
            EngineEvent::TrackDetails { id, .. } => {
                // The player has already filled the queue entry; keep a saved copy in step.
                drop(rec);
                let track = self
                    .app
                    .state::<AppState>()
                    .player
                    .lock()
                    .ok()
                    .and_then(|p| p.queue().get(*id).map(|entry| entry.track.clone()));
                if let Some(track) = track {
                    if let Err(err) = with_library(&self.app, |l| l.refresh_track(&track)) {
                        log::warn!("could not update a saved song: {err}");
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
        crate::shell::on_engine_event(&self.app, &e);
    }
    fn notice(&self, message: String) {
        log::info!("notice shown to the listener: {message}");
        if let Err(e) = crate::shell::emit_noted(
            &self.app,
            crate::state::NOTICE_EVENT,
            crate::dto::NoticeDto { message },
        ) {
            log::warn!("could not send a notice to the window: {e}");
        }
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

/// How often the default output device is looked at (the system gives no "device changed" callback through cpal).
const DEVICE_CHECK_EVERY: Duration = Duration::from_millis(500);
/// The rate assumed for the engine when there is no output device at start; the first device that appears sets it.
const FALLBACK_RATE: u32 = 48_000;

/// Opens the default output device and starts the stream on it, pulling from `engine`. When the device runs at
/// another sample rate than the engine was built for, the engine is rebuilt at the new rate first.
fn open_output(engine: &mut Engine, errors: &Sender<String>) -> Result<Output, AudioError> {
    let device = DeviceInfo::default_output()?;
    engine.change_rate(device.rate());
    let tx = errors.clone();
    device.start_with(engine.consumer(), move |e| {
        let _ = tx.send(e);
    })
}

fn spawn_real(app: AppHandle, rx: Receiver<Effect>) {
    thread::spawn(move || {
        let host = Arc::new(AppHost::new(app.clone()));
        let source = Arc::clone(&app.state::<AppState>().source);
        let first = DeviceInfo::default_output();
        let rate = first.as_ref().map_or(FALLBACK_RATE, DeviceInfo::rate);
        let mut engine = Engine::new(
            host.clone(),
            source,
            rate,
            app.state::<AppState>().volume.clone(),
        );
        let (err_tx, err_rx) = channel::<String>();
        let clock = Instant::now();
        let since_start = || clock.elapsed().as_millis() as u64;
        // `output` keeps the audio stream alive on this thread. `None` while there is no usable device.
        let mut output: Option<Output> = None;
        let mut watch = DeviceWatch::new(None, 0);
        match first.and_then(|d| {
            let tx = err_tx.clone();
            d.start_with(engine.consumer(), move |e| {
                let _ = tx.send(e);
            })
        }) {
            Ok(o) => {
                watch = DeviceWatch::new(Some(o.name().to_string()), since_start());
                output = Some(o);
            }
            Err(e) => log::warn!("audio output unavailable at start: {e}"),
        }
        let mut last_save = Instant::now();
        let mut last_check = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(effect) => {
                    if matches!(
                        effect,
                        Effect::Resume
                            | Effect::StartTrack { .. }
                            | Effect::Stop
                            | Effect::Seek { .. }
                    ) {
                        watch.on_user(UserEffect::TookOver);
                    }
                    match effect {
                        // No device: say so instead of playing into the void. Retry works once one is back.
                        Effect::StartTrack { id, .. } if watch.is_lost() => {
                            host.engine_event(EngineEvent::LoadFailed {
                                id,
                                failure: Failure::Other(
                                    "No audio output device. Connect one and press play again."
                                        .into(),
                                ),
                            })
                        }
                        effect => engine.handle(effect),
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            engine.poll();
            let mut todo: VecDeque<DeviceAction> = VecDeque::new();
            while let Ok(e) = err_rx.try_recv() {
                todo.extend(watch.on_stream_error(&e, engine.is_playing()));
            }
            if last_check.elapsed() >= DEVICE_CHECK_EVERY {
                last_check = Instant::now();
                todo.extend(watch.tick(&Observation {
                    now_ms: since_start(),
                    default_device: DeviceInfo::default_name(),
                    callbacks: output.as_ref().map_or(0, Output::callbacks),
                    playing: engine.is_playing(),
                }));
            }
            while let Some(action) = todo.pop_front() {
                match action {
                    DeviceAction::Rebuild { reason } => {
                        // The old stream (dead, or healthy on a device that is no longer the default) goes first.
                        output = None;
                        match open_output(&mut engine, &err_tx) {
                            Ok(o) => {
                                // Errors reported by the stream that was just closed are not about this one.
                                while err_rx.try_recv().is_ok() {}
                                todo.extend(watch.rebuilt(o.name(), o.rate(), since_start()));
                                output = Some(o);
                            }
                            Err(e) => {
                                log::debug!("audio output rebuild ({reason}) failed: {e}");
                                todo.extend(watch.rebuild_failed(
                                    &e.to_string(),
                                    since_start(),
                                    engine.is_playing(),
                                ));
                            }
                        }
                    }
                    DeviceAction::PausePlayer => host.command(Command::Pause),
                    DeviceAction::ResumePlayer => host.command(Command::Resume),
                    DeviceAction::Notice(message) => host.notice(message),
                    DeviceAction::Log { warn: true, text } => log::warn!("{text}"),
                    DeviceAction::Log { warn: false, text } => log::info!("{text}"),
                }
            }
            if last_save.elapsed() >= SAVE_EVERY {
                last_save = Instant::now();
                app.state::<AppState>().save_session();
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
