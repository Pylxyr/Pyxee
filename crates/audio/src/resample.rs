//! Streaming stereo resampler (rubato sinc) that preserves exact output length.

use crate::{AudioError, CHANNELS};
use rubato::audioadapter_buffers::direct::SequentialSliceOfSlices;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

const CHUNK: usize = 1024;

struct Active {
    rs: Async<f32>,
    pending: [Vec<f32>; CHANNELS],
    in_frames: u64,
    out_frames: u64,
    ratio: f64,
}

/// Passthrough when rates match, otherwise a high-quality sinc resampler whose
/// output is trimmed to the exact expected length so tracks keep their true duration.
pub struct StreamResampler {
    active: Option<Active>,
}

fn err<E: std::fmt::Display>(e: E) -> AudioError {
    AudioError::Resample(e.to_string())
}

impl StreamResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Result<Self, AudioError> {
        if in_rate == out_rate {
            return Ok(Self { active: None });
        }
        let ratio = f64::from(out_rate) / f64::from(in_rate);
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: Some(0.95),
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let rs = Async::<f32>::new_sinc(ratio, 1.0, &params, CHUNK, CHANNELS, FixedAsync::Input)
            .map_err(err)?;
        // rubato output is already time-aligned (measured in examples/lag.rs), so no delay is skipped.
        Ok(Self {
            active: Some(Active {
                rs,
                pending: [Vec::new(), Vec::new()],
                in_frames: 0,
                out_frames: 0,
                ratio,
            }),
        })
    }

    /// Appends resampled interleaved stereo to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), AudioError> {
        let Some(a) = self.active.as_mut() else {
            out.extend_from_slice(input);
            return Ok(());
        };
        for &[l, r] in input.as_chunks::<CHANNELS>().0 {
            a.pending[0].push(l);
            a.pending[1].push(r);
        }
        a.in_frames += (input.len() / CHANNELS) as u64;
        loop {
            let need = a.rs.input_frames_next();
            if a.pending[0].len() < need {
                return Ok(());
            }
            let res = Self::run(a, need)?;
            Self::emit(a, res, out);
        }
    }

    /// Flushes the filter tail and trims to the exact expected length.
    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), AudioError> {
        let Some(a) = self.active.as_mut() else {
            return Ok(());
        };
        let expected = (a.in_frames as f64 * a.ratio).round() as u64;
        while a.out_frames < expected {
            let need = a.rs.input_frames_next();
            for ch in 0..CHANNELS {
                a.pending[ch].resize(need.max(a.pending[ch].len()), 0.0);
            }
            let res = Self::run(a, need)?;
            Self::emit(a, res, out);
        }
        if a.out_frames > expected {
            let extra = (a.out_frames - expected) as usize;
            out.truncate(out.len() - extra * CHANNELS);
            a.out_frames = expected;
        }
        Ok(())
    }

    /// Resamples the first `need` pending frames and returns the interleaved result.
    fn run(a: &mut Active, need: usize) -> Result<Vec<f32>, AudioError> {
        let res = {
            let chunk = [&a.pending[0][..need], &a.pending[1][..need]];
            let input = SequentialSliceOfSlices::new(&chunk, CHANNELS, need).map_err(err)?;
            a.rs.process(&input, None).map_err(err)?
        };
        a.pending[0].drain(..need);
        a.pending[1].drain(..need);
        Ok(res.take_data())
    }

    fn emit(a: &mut Active, res: Vec<f32>, out: &mut Vec<f32>) {
        a.out_frames += (res.len() / CHANNELS) as u64;
        out.extend_from_slice(&res);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frames: usize, rate: u32) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = (i as f32 / rate as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.5;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn same_rate_is_passthrough() {
        let mut r = StreamResampler::new(48_000, 48_000).unwrap();
        let input = tone(100, 48_000);
        let mut out = Vec::new();
        r.process(&input, &mut out).unwrap();
        r.finish(&mut out).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn length_is_exact_and_no_leading_silence() {
        let mut r = StreamResampler::new(44_100, 48_000).unwrap();
        let input = tone(44_100, 44_100);
        let mut out = Vec::new();
        for c in input.chunks(2 * 1000) {
            r.process(c, &mut out).unwrap();
        }
        r.finish(&mut out).unwrap();
        assert_eq!(out.len() / 2, 48_000);
        // Delay removed: the output starts at the signal, not at filter latency zeros.
        let first_loud = out.chunks(2).position(|f| f[0].abs() > 0.01).unwrap();
        assert!(first_loud < 40, "signal starts late: {first_loud}");
    }
}
