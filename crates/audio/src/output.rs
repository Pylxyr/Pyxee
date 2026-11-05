//! The real output device (WASAPI shared mode on Windows, through cpal).

use crate::pipeline::Consumer;
use crate::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// An output device as the system knows it. The id is cpal's stable device id (the same for the same device across
/// restarts, and different for two devices that happen to share a name); the name is what the listener sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdent {
    pub id: String,
    pub name: String,
}

fn ident_of(device: &cpal::Device) -> DeviceIdent {
    let name = device
        .description()
        .map_or_else(|_| "unknown device".into(), |d| d.name().to_string());
    // A device that cannot tell its id is told apart by its name, which is all that was available before cpal 0.18.
    let id = device
        .id()
        .map_or_else(|_| name.clone(), |id| id.to_string());
    DeviceIdent { id, name }
}

pub struct DeviceInfo {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    ident: DeviceIdent,
}

impl DeviceInfo {
    /// The system default output device.
    pub fn default_output() -> Result<Self, AudioError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| AudioError::Unsupported("no audio output device found".into()))?;
        let config = device
            .default_output_config()
            .map_err(|e| open_failure("cannot read the device format", &e))?;
        let ident = ident_of(&device);
        Ok(Self {
            device,
            config,
            ident,
        })
    }

    pub fn rate(&self) -> u32 {
        self.config.sample_rate()
    }

    /// Which device this is (used to notice that the default output changed).
    pub fn ident(&self) -> &DeviceIdent {
        &self.ident
    }

    /// The current default output device without opening it; `None` when there is none. Cheap enough to poll about
    /// twice a second.
    pub fn default_ident() -> Option<DeviceIdent> {
        let device = cpal::default_host().default_output_device()?;
        Some(ident_of(&device))
    }

    /// Starts the stream, pulling from `consumer`. Keep the returned value alive to keep playing: dropping it stops the
    /// stream. (cpal 0.18 streams are `Send + Sync`, so it may be moved to another thread; the app keeps it on the
    /// thread that opens and replaces it.)
    pub fn start(self, consumer: Consumer) -> Result<Output, AudioError> {
        self.start_with(consumer, |e| log::error!("audio stream error: {e}"))
    }

    /// Like `start`, and `on_error` is called (from an audio thread) with the reason when the stream *breaks*, for
    /// example when the device is unplugged. After that the stream is dead: open a new one. A glitch that the stream
    /// survives (see [`fault`]) is only logged.
    pub fn start_with(
        self,
        consumer: Consumer,
        on_error: impl Fn(String) + Send + 'static,
    ) -> Result<Output, AudioError> {
        let ident = self.ident.clone();
        let callbacks = Arc::new(AtomicU64::new(0));
        let rate = u64::from(self.rate());
        let ch = self.config.channels() as usize;
        let cfg = self.config.config();
        let err = |e: cpal::Error| open_failure("cannot open the output device", &e);
        let fail = move |e: cpal::Error| match fault(e.kind()) {
            Fault::Harmless => match e.kind() {
                cpal::ErrorKind::RealtimeDenied => log::warn!("audio stream: {e}"),
                cpal::ErrorKind::DeviceChanged => {
                    log::info!("audio stream followed the system: {e}")
                }
                _ => log::debug!("audio stream glitch: {e}"),
            },
            Fault::Fatal => on_error(e.to_string()),
        };
        let stream = match self.config.sample_format() {
            SampleFormat::F32 => {
                let mut scratch: Vec<f32> = Vec::with_capacity(16 * 1024);
                let calls = Arc::clone(&callbacks);
                self.device
                    .build_output_stream(
                        cfg,
                        move |data: &mut [f32], info| {
                            calls.fetch_add(1, Ordering::Relaxed);
                            let latency = latency_frames(info, rate);
                            scratch.resize(data.len() / ch * 2, 0.0);
                            consumer.pull(&mut scratch, latency);
                            spread(&scratch, ch, data.chunks_exact_mut(ch), |s| s);
                        },
                        fail,
                        None,
                    )
                    .map_err(err)?
            }
            SampleFormat::I16 => {
                let mut scratch: Vec<f32> = Vec::with_capacity(16 * 1024);
                let calls = Arc::clone(&callbacks);
                self.device
                    .build_output_stream(
                        cfg,
                        move |data: &mut [i16], info| {
                            calls.fetch_add(1, Ordering::Relaxed);
                            let latency = latency_frames(info, rate);
                            scratch.resize(data.len() / ch * 2, 0.0);
                            consumer.pull(&mut scratch, latency);
                            spread(&scratch, ch, data.chunks_exact_mut(ch), |s| {
                                (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
                            });
                        },
                        fail,
                        None,
                    )
                    .map_err(err)?
            }
            other => {
                return Err(AudioError::Unsupported(format!(
                    "unsupported device sample format {other:?}"
                )));
            }
        };
        // cpal 0.18 no longer starts a stream by itself.
        stream
            .play()
            .map_err(|e| open_failure("cannot start the output", &e))?;
        Ok(Output {
            _stream: stream,
            ident,
            rate: rate as u32,
            callbacks,
        })
    }
}

fn latency_frames(info: &cpal::OutputCallbackInfo, rate: u64) -> u64 {
    let ts = info.timestamp();
    // Saturates to zero when the playback instant is not after the callback instant.
    ts.playback.duration_since(ts.callback).as_micros() as u64 * rate / 1_000_000
}

/// What a stream error means for the stream that reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    /// The stream goes on playing: nothing to rebuild.
    Harmless,
    /// The stream is dead, or must be opened again.
    Fatal,
}

fn fault(kind: cpal::ErrorKind) -> Fault {
    use cpal::ErrorKind as K;
    match kind {
        // Reported *while the stream goes on*: a buffer under- or overrun (a glitch), a refused real-time priority
        // (it plays, with more risk of glitches), and a change of the route that the stream follows by itself.
        K::Xrun | K::RealtimeDenied | K::DeviceChanged => Fault::Harmless,
        // Gone, invalidated, or not told apart from that. `ErrorKind` can grow: a kind not known yet counts as fatal,
        // because a rebuild that was not needed costs a moment and a dead stream that nobody noticed costs the music.
        _ => Fault::Fatal,
    }
}

/// A sentence about why a device could not be used, for the kinds of failure that have a usual reason.
fn hint(kind: cpal::ErrorKind) -> Option<&'static str> {
    use cpal::ErrorKind as K;
    match kind {
        K::DeviceNotAvailable => Some("the device is not available (unplugged or disabled)"),
        K::DeviceBusy => Some("another program is using the device"),
        K::PermissionDenied => Some("the system did not allow access to the audio device"),
        K::UnsupportedConfig => Some("the device does not support the format"),
        K::HostUnavailable => Some("the system's audio service is not running"),
        _ => None,
    }
}

fn open_failure(what: &str, e: &cpal::Error) -> AudioError {
    AudioError::Unsupported(match hint(e.kind()) {
        Some(h) => format!("{what}: {h} ({e})"),
        None => format!("{what}: {e}"),
    })
}

/// Copies stereo frames into device frames of `ch` channels (extra channels stay silent).
fn spread<'a, T: Copy + Default + 'a>(
    stereo: &[f32],
    ch: usize,
    frames: impl Iterator<Item = &'a mut [T]>,
    conv: impl Fn(f32) -> T,
) {
    for (i, frame) in frames.enumerate() {
        frame.fill(T::default());
        frame[0] = conv(stereo[i * 2]);
        if ch > 1 {
            frame[1] = conv(stereo[i * 2 + 1]);
        }
    }
}

pub struct Output {
    _stream: Stream,
    ident: DeviceIdent,
    rate: u32,
    callbacks: Arc<AtomicU64>,
}

impl Output {
    /// The device this stream plays on.
    pub fn ident(&self) -> &DeviceIdent {
        &self.ident
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// How many times the device has asked for audio since this stream started (it stops growing when the device
    /// is gone without having said so).
    pub fn callbacks(&self) -> u64 {
        self.callbacks.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::ErrorKind as K;

    #[test]
    fn a_glitch_the_stream_survives_is_not_a_lost_device() {
        for kind in [K::Xrun, K::RealtimeDenied, K::DeviceChanged] {
            assert_eq!(fault(kind), Fault::Harmless, "{kind:?}");
        }
    }

    #[test]
    fn a_stream_that_is_gone_or_unknown_is_rebuilt() {
        for kind in [
            K::DeviceNotAvailable,
            K::StreamInvalidated,
            K::HostUnavailable,
            K::BackendError,
            K::DeviceBusy,
            K::ResourceExhausted,
            K::Other,
        ] {
            assert_eq!(fault(kind), Fault::Fatal, "{kind:?}");
        }
    }

    #[test]
    fn the_usual_reasons_are_told_in_words() {
        assert!(hint(K::DeviceBusy).unwrap().contains("another program"));
        assert!(hint(K::DeviceNotAvailable).unwrap().contains("unplugged"));
        assert!(hint(K::PermissionDenied).is_some());
        assert_eq!(hint(K::Other), None);
    }

    /// The stream and what holds it may be handed to another thread (cpal 0.18).
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn the_output_may_move_between_threads() {
        fn needs<T: Send + Sync>() {}
        needs::<Output>();
    }
}
