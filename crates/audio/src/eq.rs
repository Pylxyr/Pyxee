//! A 10-band graphic equalizer: one peaking biquad per band (RBJ cookbook), a preamp, automatic headroom and a soft
//! limiter, so that a boost cannot clip hard. Pure DSP: it knows nothing about devices or threads.
//!
//! The settings live in an [`EqControl`], a handle of atomics that the UI side writes and the audio callback reads
//! without a lock. The callback owns an [`Equalizer`], which notices a change by a version number, designs the
//! filters again and processes the samples. With the equalizer off, or on but flat (no band moved, preamp 0), a call
//! to `process` costs one atomic load and leaves the samples untouched.

use crate::CHANNELS;
use std::f64::consts::{PI, SQRT_2};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

pub const BANDS: usize = 10;
/// Band centres in Hz: octaves from 31 Hz to 16 kHz.
pub const BAND_HZ: [f64; BANDS] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
/// Gains and the preamp are set in tenths of a dB and kept within this many of them (12 dB).
pub const LIMIT_TENTHS: i32 = 120;
/// One octave wide bands: the usual graphic equalizer.
const Q: f64 = SQRT_2;
/// A band whose centre is above this share of the sample rate does nothing (a device that runs at 8 kHz has no 16 kHz).
const USABLE_SHARE_OF_RATE: f64 = 0.45;
/// Samples below this pass the limiter unchanged.
const KNEE: f32 = 0.95;

/// What the listener chose: all gains and the preamp in tenths of a dB, within `±LIMIT_TENTHS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EqParams {
    pub enabled: bool,
    pub preamp: i32,
    pub gains: [i32; BANDS],
}

#[derive(Default)]
struct ControlShared {
    enabled: AtomicBool,
    preamp: AtomicI32,
    gains: [AtomicI32; BANDS],
    /// Counts `set` calls. Written last, read first, so a reader that sees a version sees at least those values.
    version: AtomicU32,
}

/// The equalizer's settings, shared between the side that changes them and the audio callback. Cloning shares them.
#[derive(Clone, Default)]
pub struct EqControl {
    shared: Arc<ControlShared>,
}

impl EqControl {
    /// Sets everything at once. Values are clamped to `±LIMIT_TENTHS`; a missing gain is 0 and an extra one is ignored.
    pub fn set(&self, enabled: bool, preamp: i32, gains: &[i32]) {
        let s = &*self.shared;
        s.enabled.store(enabled, Ordering::Relaxed);
        s.preamp
            .store(preamp.clamp(-LIMIT_TENTHS, LIMIT_TENTHS), Ordering::Relaxed);
        for (i, slot) in s.gains.iter().enumerate() {
            let g = gains.get(i).copied().unwrap_or(0);
            slot.store(g.clamp(-LIMIT_TENTHS, LIMIT_TENTHS), Ordering::Relaxed);
        }
        s.version.fetch_add(1, Ordering::Release);
    }

    /// The current settings.
    #[must_use]
    pub fn params(&self) -> EqParams {
        self.read().1
    }

    fn version(&self) -> u32 {
        self.shared.version.load(Ordering::Acquire)
    }

    /// The version first, then the values: the values are at least as new as the version. A `set` that runs
    /// meanwhile makes the next `version()` differ, so the reader looks again.
    fn read(&self) -> (u32, EqParams) {
        let s = &*self.shared;
        let version = s.version.load(Ordering::Acquire);
        let mut gains = [0; BANDS];
        for (g, slot) in gains.iter_mut().zip(&s.gains) {
            *g = slot.load(Ordering::Relaxed);
        }
        let params = EqParams {
            enabled: s.enabled.load(Ordering::Relaxed),
            preamp: s.preamp.load(Ordering::Relaxed),
            gains,
        };
        (version, params)
    }
}

/// A biquad's coefficients with `a0` divided out.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Coeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

const IDENTITY: Coeffs = Coeffs {
    b0: 1.0,
    b1: 0.0,
    b2: 0.0,
    a1: 0.0,
    a2: 0.0,
};

/// RBJ "peakingEQ": `gain_db` at `hz`, unity far from it.
fn peaking(rate: f64, hz: f64, gain_db: f64) -> Coeffs {
    let a = 10f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * hz / rate;
    let (sin, cos) = w0.sin_cos();
    let alpha = sin / (2.0 * Q);
    let a0 = 1.0 + alpha / a;
    Coeffs {
        b0: (1.0 + alpha * a) / a0,
        b1: -2.0 * cos / a0,
        b2: (1.0 - alpha * a) / a0,
        a1: -2.0 * cos / a0,
        a2: (1.0 - alpha / a) / a0,
    }
}

/// The gain of a filter at angular frequency `w` (radians per sample), as a ratio.
fn magnitude(c: &Coeffs, w: f64) -> f64 {
    let (s1, c1) = w.sin_cos();
    let (s2, c2) = (2.0 * w).sin_cos();
    let (nr, ni) = (c.b0 + c.b1 * c1 + c.b2 * c2, -(c.b1 * s1 + c.b2 * s2));
    let (dr, di) = (1.0 + c.a1 * c1 + c.a2 * c2, -(c.a1 * s1 + c.a2 * s2));
    ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt()
}

/// The filters for one sample rate and one set of gains.
struct Design {
    coeffs: [Coeffs; BANDS],
    /// A band that is at 0 dB, or too high for the rate, is the identity and is skipped.
    active: [bool; BANDS],
    /// How far the summed response of the bands rises above 0 dB (never negative): the preamp takes it off again.
    headroom_db: f64,
}

fn design(rate: f64, gains: &[i32; BANDS]) -> Design {
    let mut d = Design {
        coeffs: [IDENTITY; BANDS],
        active: [false; BANDS],
        headroom_db: 0.0,
    };
    for (b, &g) in gains.iter().enumerate() {
        if g != 0 && BAND_HZ[b] < rate * USABLE_SHARE_OF_RATE {
            d.coeffs[b] = peaking(rate, BAND_HZ[b], f64::from(g) / 10.0);
            d.active[b] = true;
        }
    }
    if d.active.iter().any(|&a| a) {
        d.headroom_db = peak_response_db(&d, rate).max(0.0);
    }
    d
}

/// The summed response of the active bands at `hz`, in dB.
fn response_db_at(d: &Design, rate: f64, hz: f64) -> f64 {
    let w = 2.0 * PI * hz / rate;
    (0..BANDS)
        .filter(|&b| d.active[b])
        .map(|b| 20.0 * magnitude(&d.coeffs[b], w).log10())
        .sum()
}

/// The highest point of the summed response, looked for every 1/24 octave from 20 Hz to the top of the audible range
/// and at every band centre.
fn peak_response_db(d: &Design, rate: f64) -> f64 {
    let top = (rate * USABLE_SHARE_OF_RATE).min(20_000.0);
    let step = 2f64.powf(1.0 / 24.0);
    let mut peak = f64::NEG_INFINITY;
    let mut hz = 20.0;
    while hz <= top {
        peak = peak.max(response_db_at(d, rate, hz));
        hz *= step;
    }
    BAND_HZ
        .iter()
        .filter(|&&c| c < top)
        .fold(peak, |p, &c| p.max(response_db_at(d, rate, c)))
}

/// The summed response of the equalizer with these gains (tenths of a dB) at `hz`, in dB, without preamp and
/// headroom: what the bands alone do.
#[must_use]
pub fn response_db(rate: u32, gains: &[i32; BANDS], hz: f64) -> f64 {
    let rate = f64::from(rate);
    response_db_at(&design(rate, gains), rate, hz)
}

/// Keeps a sample below full scale: unchanged up to `KNEE`, then bending smoothly towards 1.0 and never reaching it
/// (apart from float rounding at the far end). Smooth at the knee, so it does not add a corner of its own.
#[must_use]
pub fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let over = (a - KNEE) / (1.0 - KNEE);
        (KNEE + (1.0 - KNEE) * over.tanh()).copysign(x)
    }
}

/// The equalizer a pipeline's audio callback owns: settings in, samples through. Not shared between threads.
pub struct Equalizer {
    control: EqControl,
    rate: f64,
    /// The version of `control` that the filters below were designed for.
    applied: u32,
    design: Design,
    /// Transposed direct form II state, per channel and band; in `f64`, because the low bands have poles close to 1.
    z: [[[f64; 2]; BANDS]; CHANNELS],
    /// The equalizer should be in the signal path (on, and not flat).
    want: bool,
    /// It was in the path for the end of the last buffer.
    in_path: bool,
    /// Preamp minus headroom, as a ratio, now and as it was at the end of the last buffer.
    gain: f32,
    prev_gain: f32,
}

impl Equalizer {
    #[must_use]
    pub fn new(rate: u32, control: EqControl) -> Self {
        let mut e = Self {
            control,
            rate: f64::from(rate),
            applied: 0,
            design: design(f64::from(rate), &[0; BANDS]),
            z: [[[0.0; 2]; BANDS]; CHANNELS],
            want: false,
            in_path: false,
            gain: 1.0,
            prev_gain: 1.0,
        };
        e.refresh();
        e
    }

    /// Reads the settings again and designs the filters.
    fn refresh(&mut self) {
        let (version, p) = self.control.read();
        self.applied = version;
        let old = std::mem::replace(&mut self.design, design(self.rate, &p.gains));
        for b in 0..BANDS {
            // An identity filter has no state: forgetting a band's keeps it in step with what it would have been.
            if old.active[b] && !self.design.active[b] {
                for ch in &mut self.z {
                    ch[b] = [0.0; 2];
                }
            }
        }
        let preamp_db = f64::from(p.preamp) / 10.0;
        self.gain = 10f64.powf((preamp_db - self.design.headroom_db) / 20.0) as f32;
        self.want = p.enabled && (p.preamp != 0 || self.design.active.iter().any(|&a| a));
    }

    /// Forgets the audio the filters have seen: called where the buffer is thrown away (a seek, a skip, a stop), so
    /// the tail of the old audio does not ring into the new. The equalizer then fades in over the next buffer.
    pub fn reset(&mut self) {
        self.z = [[[0.0; 2]; BANDS]; CHANNELS];
        self.in_path = false;
        self.prev_gain = self.gain;
    }

    /// Runs interleaved stereo samples through the equalizer, in place. A change of settings is picked up here. An
    /// equalizer that turns on or off, or goes flat, fades over this buffer, so the switch does not click, and the
    /// preamp ramps the same way.
    pub fn process(&mut self, samples: &mut [f32]) {
        if self.control.version() != self.applied {
            self.refresh();
        }
        if !self.in_path && !self.want {
            return;
        }
        let frames = samples.len() / CHANNELS;
        if frames == 0 {
            return;
        }
        let fading = self.in_path != self.want;
        let (mix_from, mix_to) = (
            if self.in_path { 1.0f32 } else { 0.0 },
            if self.want { 1.0f32 } else { 0.0 },
        );
        let (g_from, g_to) = (self.prev_gain, self.gain);
        for (i, frame) in samples.as_chunks_mut::<CHANNELS>().0.iter_mut().enumerate() {
            let t = (i + 1) as f32 / frames as f32;
            let g = g_from + (g_to - g_from) * t;
            let mix = mix_from + (mix_to - mix_from) * t;
            for (ch, sample) in frame.iter_mut().enumerate() {
                let dry = if sample.is_finite() { *sample } else { 0.0 };
                let mut v = f64::from(dry);
                for b in 0..BANDS {
                    if self.design.active[b] {
                        let c = &self.design.coeffs[b];
                        let z = &mut self.z[ch][b];
                        let y = c.b0 * v + z[0];
                        z[0] = c.b1 * v - c.a1 * y + z[1];
                        z[1] = c.b2 * v - c.a2 * y;
                        v = y;
                    }
                }
                let wet = soft_limit(v as f32 * g);
                *sample = if fading { dry + (wet - dry) * mix } else { wet };
            }
        }
        self.in_path = self.want;
        self.prev_gain = self.gain;
        if !self.in_path {
            self.z = [[[0.0; 2]; BANDS]; CHANNELS];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATES: [u32; 6] = [8_000, 22_050, 44_100, 48_000, 96_000, 192_000];

    fn control(enabled: bool, preamp: i32, gains: [i32; BANDS]) -> EqControl {
        let c = EqControl::default();
        c.set(enabled, preamp, &gains);
        c
    }

    fn sine(rate: u32, hz: f64, frames: usize, amp: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = (2.0 * PI * hz * i as f64 / f64::from(rate)).sin() as f32 * amp;
                [s, s]
            })
            .collect()
    }

    /// The peak of the second half of the left channel (after the filters settled).
    fn settled_peak(samples: &[f32]) -> f32 {
        let frames = samples.len() / CHANNELS;
        (frames / 2..frames)
            .map(|i| samples[i * CHANNELS].abs())
            .fold(0.0, f32::max)
    }

    fn db(ratio: f32) -> f32 {
        20.0 * ratio.log10()
    }

    #[test]
    fn a_band_has_exactly_its_gain_at_its_centre_and_none_far_away() {
        for rate in [44_100u32, 48_000, 96_000] {
            for (b, &hz) in BAND_HZ.iter().enumerate() {
                for tenths in [-120, -45, 30, 120] {
                    let mut gains = [0; BANDS];
                    gains[b] = tenths;
                    let at_centre = response_db(rate, &gains, hz);
                    assert!(
                        (at_centre - f64::from(tenths) / 10.0).abs() < 1e-6,
                        "band {b} at {rate} Hz, {tenths}: {at_centre}"
                    );
                }
            }
        }
        let mut gains = [0; BANDS];
        gains[5] = 120;
        assert!(response_db(48_000, &gains, 40.0).abs() < 0.5);
        assert!(response_db(48_000, &gains, 15_000.0).abs() < 0.5);
    }

    #[test]
    fn a_real_signal_comes_out_as_the_response_says() {
        let rate = 48_000;
        let mut gains = [0; BANDS];
        gains[5] = -80; // -8 dB at 1 kHz: nothing to compensate, so no headroom is taken off
        let mut eq = Equalizer::new(rate, control(true, 0, gains));
        let mut s = sine(rate, 1000.0, 48_000, 0.5);
        eq.process(&mut s); // the first buffer fades the equalizer in
        s = sine(rate, 1000.0, 48_000, 0.5);
        eq.process(&mut s);
        let got = db(settled_peak(&s) / 0.5);
        assert!((got + 8.0).abs() < 0.1, "{got} dB");
    }

    #[test]
    fn off_and_flat_leave_every_sample_untouched() {
        let rate = 48_000;
        let original = sine(rate, 440.0, 4_800, 1.3); // even beyond full scale
        let mut gains = [0; BANDS];
        gains[3] = 60;
        for c in [
            control(false, 50, gains),
            control(true, 0, [0; BANDS]),
            EqControl::default(),
        ] {
            let mut eq = Equalizer::new(rate, c);
            let mut s = original.clone();
            eq.process(&mut s);
            assert_eq!(s, original);
        }
    }

    #[test]
    fn boosting_never_clips_even_with_every_band_and_the_preamp_up() {
        for rate in RATES {
            let eq_control = control(true, 120, [120; BANDS]);
            let mut eq = Equalizer::new(rate, eq_control);
            for hz in [40.0f64, 100.0, 440.0, 1000.0, 5000.0, 12_000.0] {
                let mut s = sine(rate, hz.min(f64::from(rate) * 0.4), rate as usize / 4, 1.0);
                eq.process(&mut s);
                assert!(
                    s.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
                    "{hz} Hz at {rate} Hz"
                );
            }
        }
    }

    #[test]
    fn headroom_makes_a_full_scale_tone_stay_below_full_scale_without_the_limiter() {
        let rate = 48_000;
        let mut gains = [0; BANDS];
        gains[4] = 120;
        gains[5] = 120;
        gains[6] = 120; // three neighbours add up to more than 12 dB between them
        let d = design(f64::from(rate), &gains);
        assert!(d.headroom_db > 12.0, "{}", d.headroom_db);
        // The summed response, minus the headroom, has no point above 0 dB.
        let top = (0..400)
            .map(|i| 20.0 * 2f64.powf(f64::from(i) / 40.0))
            .take_while(|&hz| hz < 20_000.0)
            .map(|hz| response_db_at(&d, f64::from(rate), hz) - d.headroom_db)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(top < 0.05, "{top}");
    }

    #[test]
    fn cuts_need_no_headroom_and_flat_needs_none() {
        let mut gains = [-60; BANDS];
        assert!(design(48_000.0, &gains).headroom_db.abs() < f64::EPSILON);
        gains = [0; BANDS];
        assert!(design(48_000.0, &gains).headroom_db.abs() < f64::EPSILON);
    }

    #[test]
    fn extreme_settings_give_no_nan_at_any_rate() {
        let patterns: [[i32; BANDS]; 4] = [
            [120; BANDS],
            [-120; BANDS],
            [120, -120, 120, -120, 120, -120, 120, -120, 120, -120],
            [-120, 120, -120, 120, -120, 120, -120, 120, -120, 120],
        ];
        for rate in RATES {
            for gains in patterns {
                let mut eq = Equalizer::new(rate, control(true, -120, gains));
                let mut noise = Vec::new();
                let mut x = 12_345u32;
                for _ in 0..rate as usize * CHANNELS / 2 {
                    x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    noise.push((x >> 8) as f32 / (1u32 << 23) as f32 - 1.0);
                }
                eq.process(&mut noise);
                assert!(noise.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
            }
        }
    }

    #[test]
    fn a_nan_or_infinity_in_the_stream_does_not_poison_the_filters() {
        let rate = 48_000;
        let mut eq = Equalizer::new(rate, control(true, 0, [60; BANDS]));
        let mut s = sine(rate, 300.0, 4_800, 0.5);
        s[10] = f32::NAN;
        s[11] = f32::INFINITY;
        eq.process(&mut s);
        assert!(s.iter().all(|v| v.is_finite()));
        let mut after = sine(rate, 300.0, 4_800, 0.5);
        eq.process(&mut after);
        assert!(after.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn another_rate_gives_other_coefficients_that_still_hit_the_gain_at_the_centre() {
        let mut gains = [0; BANDS];
        gains[6] = 90;
        let a = design(44_100.0, &gains);
        let b = design(48_000.0, &gains);
        assert_ne!(a.coeffs[6], b.coeffs[6]);
        for (d, rate) in [(&a, 44_100.0), (&b, 48_000.0)] {
            assert!((response_db_at(d, rate, BAND_HZ[6]) - 9.0).abs() < 1e-6);
        }
    }

    #[test]
    fn a_band_above_what_the_rate_allows_is_off_and_the_rest_still_work() {
        let d = design(8_000.0, &[60; BANDS]);
        assert!(!d.active[9] && !d.active[8], "8 kHz and 16 kHz at 8 kHz");
        assert!(d.active[0] && d.active[6]);
        let mut eq = Equalizer::new(8_000, control(true, 0, [60; BANDS]));
        let mut s = sine(8_000, 500.0, 8_000, 0.3);
        eq.process(&mut s);
        assert!(s.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn the_limiter_is_clear_below_the_knee_smooth_above_it_and_never_reaches_full_scale() {
        for x in [0.0f32, 0.3, -0.5, KNEE, -KNEE] {
            assert!((soft_limit(x) - x).abs() < f32::EPSILON, "{x}");
        }
        let mut last = 0.0;
        for i in 0..2000 {
            let x = 0.9 + i as f32 * 0.005; // up to 10.9
            let y = soft_limit(x);
            assert!(y >= last && y <= 1.0, "{x} -> {y}");
            assert!((soft_limit(-x) + y).abs() < f32::EPSILON, "odd symmetry");
            last = y;
        }
        // No step at the knee: a hair above it is a hair above the value at it.
        assert!((soft_limit(KNEE + 1e-4) - KNEE).abs() < 2e-4);
    }

    #[test]
    fn a_settings_change_is_picked_up_by_the_next_buffer() {
        let rate = 48_000;
        let control = EqControl::default();
        let mut eq = Equalizer::new(rate, control.clone());
        let mut s = sine(rate, 1000.0, 48_000, 0.4);
        let original = s.clone();
        eq.process(&mut s);
        assert_eq!(s, original, "off at first");
        let mut gains = [0; BANDS];
        gains[5] = -60;
        control.set(true, 0, &gains);
        for _ in 0..2 {
            s = sine(rate, 1000.0, 48_000, 0.4);
            eq.process(&mut s);
        }
        let got = db(settled_peak(&s) / 0.4);
        assert!((got + 6.0).abs() < 0.1, "{got} dB");
        control.set(false, 0, &gains);
        for _ in 0..2 {
            s = sine(rate, 1000.0, 48_000, 0.4);
            eq.process(&mut s);
        }
        assert_eq!(s, sine(rate, 1000.0, 48_000, 0.4), "off again, untouched");
    }

    #[test]
    fn turning_it_on_or_off_fades_over_one_buffer_instead_of_switching() {
        let rate = 48_000;
        let control = EqControl::default();
        let mut eq = Equalizer::new(rate, control.clone());
        control.set(true, -60, &[0; BANDS]); // preamp only: -6 dB
        let dc = vec![0.5f32; 960 * CHANNELS];
        let mut s = dc.clone();
        eq.process(&mut s);
        assert!(
            (s[0] - 0.5).abs() < 0.01,
            "starts at the dry level: {}",
            s[0]
        );
        let last = s[s.len() - CHANNELS];
        assert!((last - 0.25).abs() < 0.01, "ends at the wet level: {last}");
        assert!(s.windows(2).all(|w| w[1] <= w[0] + 1e-6), "no jump back up");
    }

    #[test]
    fn the_preamp_ramps_across_a_buffer() {
        let rate = 48_000;
        let control = control(true, -60, [0; BANDS]);
        let mut eq = Equalizer::new(rate, control.clone());
        let mut warm = vec![0.5f32; 960 * CHANNELS];
        eq.process(&mut warm);
        control.set(true, 0, &[0; BANDS]); // back to 0 dB, but the equalizer is flat: it fades out
        let mut s = vec![0.5f32; 960 * CHANNELS];
        eq.process(&mut s);
        assert!(
            s[0] < 0.27 && s[s.len() - 1] > 0.49,
            "{} {}",
            s[0],
            s[s.len() - 1]
        );
    }

    #[test]
    fn reset_forgets_the_old_audio() {
        let rate = 48_000;
        let mut gains = [0; BANDS];
        gains[0] = 120;
        let c = control(true, 0, gains);
        let mut eq = Equalizer::new(rate, c);
        let mut loud = sine(rate, 40.0, 24_000, 0.8);
        eq.process(&mut loud);
        eq.reset();
        let mut silence = vec![0.0f32; 4_800 * CHANNELS];
        eq.process(&mut silence);
        assert!(
            silence.iter().all(|&v| v == 0.0),
            "nothing rings on after a reset"
        );
    }

    #[test]
    fn a_control_clamps_pads_and_counts_its_changes() {
        let c = EqControl::default();
        assert_eq!(c.params(), EqParams::default());
        let v0 = c.version();
        c.set(true, 999, &[500, -500, 7]);
        let p = c.params();
        assert!(p.enabled);
        assert_eq!(p.preamp, LIMIT_TENTHS);
        assert_eq!(&p.gains[..4], &[LIMIT_TENTHS, -LIMIT_TENTHS, 7, 0]);
        assert_eq!(c.version(), v0 + 1);
        let clone = c.clone();
        clone.set(false, 0, &[0; BANDS + 4]);
        assert_eq!(c.params(), EqParams::default(), "a clone shares the values");
    }
}
