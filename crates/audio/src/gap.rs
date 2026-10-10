//! Detects digital silence (gaps) in interleaved PCM.

/// A run of near-silent frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    pub start_frame: usize,
    pub len_frames: usize,
}

/// Finds runs of at least `min_run_frames` consecutive frames where every channel is
/// below `threshold` in absolute value. A pure sine crosses zero for only a frame or two,
/// so a run of several frames means real silence.
pub fn find_gaps(
    samples: &[f32],
    channels: usize,
    threshold: f32,
    min_run_frames: usize,
) -> Vec<Gap> {
    let mut gaps = Vec::new();
    let mut run_start: Option<usize> = None;
    let frames = samples.len() / channels;
    for f in 0..frames {
        let quiet = samples[f * channels..(f + 1) * channels]
            .iter()
            .all(|s| s.abs() < threshold);
        match (quiet, run_start) {
            (true, None) => run_start = Some(f),
            (false, Some(s)) => {
                if f - s >= min_run_frames {
                    gaps.push(Gap {
                        start_frame: s,
                        len_frames: f - s,
                    });
                }
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = run_start {
        if frames - s >= min_run_frames {
            gaps.push(Gap {
                start_frame: s,
                len_frames: frames - s,
            });
        }
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frames: usize, offset: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = (((i + offset) as f32) * 0.06).sin() * 0.5;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn continuous_sine_has_no_gaps() {
        assert!(find_gaps(&sine(48_000, 0), 2, 1e-3, 8).is_empty());
    }

    #[test]
    fn inserted_silence_is_found() {
        let mut s = sine(1000, 0);
        s.extend(std::iter::repeat_n(0.0, 2 * 50));
        s.extend(sine(1000, 1000));
        let gaps = find_gaps(&s, 2, 1e-3, 8);
        assert_eq!(
            gaps,
            vec![Gap {
                start_frame: 1000,
                len_frames: 50
            }]
        );
    }

    #[test]
    fn trailing_silence_is_found() {
        let mut s = sine(100, 0);
        s.extend(std::iter::repeat_n(0.0, 2 * 20));
        assert_eq!(find_gaps(&s, 2, 1e-3, 8).len(), 1);
    }
}
