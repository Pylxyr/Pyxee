use pyxee_audio::resample::StreamResampler;
fn main() {
    let (ri, ro) = (44_100u32, 48_000u32);
    let n = 88_200usize;
    let input: Vec<f32> = (0..n)
        .flat_map(|i| {
            let v = (i as f32 / ri as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            [v, v]
        })
        .collect();
    let mut r = StreamResampler::new(ri, ro).unwrap();
    let mut out = Vec::new();
    for c in input.chunks(2000) {
        r.process(c, &mut out).unwrap();
    }
    r.finish(&mut out).unwrap();
    let frames = out.len() / 2;
    let ideal = |i: i64| ((i as f64) / ro as f64 * 440.0 * std::f64::consts::TAU).sin() * 0.5;
    let mut best = (0i64, f64::MAX);
    for lag in -50i64..=50 {
        let mut e = 0.0;
        let mut c = 0;
        for i in 1000..frames - 1000 {
            let d = out[i * 2] as f64 - ideal(i as i64 - lag);
            e += d * d;
            c += 1;
        }
        let e = e / c as f64;
        if e < best.1 {
            best = (lag, e);
        }
    }
    println!("frames={frames} best_lag={} mse={:e}", best.0, best.1);
    let tail_quiet = out
        .chunks(2)
        .rev()
        .take(100)
        .filter(|f| f[0].abs() < 1e-3)
        .count();
    println!("quiet frames among last 100: {tail_quiet}");
}
