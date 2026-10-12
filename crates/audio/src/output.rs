//! The real output device (WASAPI shared mode on Windows, through cpal).

use crate::pipeline::Consumer;
use crate::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

pub struct DeviceInfo {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
}

impl DeviceInfo {
    /// The system default output device.
    pub fn default_output() -> Result<Self, AudioError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| AudioError::Unsupported("no audio output device found".into()))?;
        let config = device
            .default_output_config()
            .map_err(|e| AudioError::Unsupported(format!("cannot read the device format: {e}")))?;
        Ok(Self { device, config })
    }

    pub fn rate(&self) -> u32 {
        self.config.sample_rate().0
    }

    /// Starts the stream, pulling from `consumer`. Keep the returned value alive to keep playing;
    /// it must stay on the thread that created it.
    pub fn start(self, consumer: Consumer) -> Result<Output, AudioError> {
        let rate = u64::from(self.rate());
        let ch = self.config.channels() as usize;
        let cfg = self.config.config();
        let err = |e: cpal::BuildStreamError| {
            AudioError::Unsupported(format!("cannot open the output device: {e}"))
        };
        let fail = |e| log::error!("audio stream error: {e}");
        let stream = match self.config.sample_format() {
            SampleFormat::F32 => {
                let mut scratch: Vec<f32> = Vec::with_capacity(16 * 1024);
                self.device
                    .build_output_stream(
                        &cfg,
                        move |data: &mut [f32], info| {
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
                self.device
                    .build_output_stream(
                        &cfg,
                        move |data: &mut [i16], info| {
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
        stream
            .play()
            .map_err(|e| AudioError::Unsupported(format!("cannot start the output: {e}")))?;
        Ok(Output { _stream: stream })
    }
}

fn latency_frames(info: &cpal::OutputCallbackInfo, rate: u64) -> u64 {
    let ts = info.timestamp();
    ts.playback
        .duration_since(&ts.callback)
        .map_or(0, |d| d.as_micros() as u64 * rate / 1_000_000)
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
}
