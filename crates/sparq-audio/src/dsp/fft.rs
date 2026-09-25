//! Small real FFT, used only by tests and analysis — never on the audio path in Phase B.
//!
//! Deliberately simple and dependency-free: an iterative radix-2 Cooley–Tukey with a
//! `no-std`-friendly shape. It must be *correct*, not fast; correctness is checked against
//! brute-force DFT in the tests below. A production analyser (WO-014 `ana/*`) will use a real
//! library and GPU compute.

/// Compute the forward FFT in place. `data.len()` must be a power of two.
///
/// Layout: `[re0, im0, re1, im1, ...]`.
///
/// # Panics
/// Panics if the length is not a power of two or is zero. Callers in tests are expected to pass a
/// valid size; the audio path never calls this.
pub fn fft_in_place(data: &mut [f64]) {
    let n = data.len() / 2;
    assert!(n > 0 && n.is_power_of_two(), "fft_in_place needs a power-of-two frame, got {n}");

    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            data.swap(2 * i, 2 * j);
            data.swap(2 * i + 1, 2 * j + 1);
        }
    }

    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let u_re = data[2 * (i + k)];
                let u_im = data[2 * (i + k) + 1];
                let t_re = data[2 * (i + k + len / 2)] * cr - data[2 * (i + k + len / 2) + 1] * ci;
                let t_im = data[2 * (i + k + len / 2)] * ci + data[2 * (i + k + len / 2) + 1] * cr;
                data[2 * (i + k)] = u_re + t_re;
                data[2 * (i + k) + 1] = u_im + t_im;
                data[2 * (i + k + len / 2)] = u_re - t_re;
                data[2 * (i + k + len / 2) + 1] = u_im - t_im;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// Hann window coefficients for a frame of `n` samples.
#[must_use]
pub fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n as f64 - 1.0).max(1.0)).cos()
        })
        .collect()
}

/// Magnitude spectrum of a real signal, Hann-windowed, returned as one value per bin up to Nyquist.
#[must_use]
pub fn magnitude_spectrum(samples: &[f32]) -> Vec<f64> {
    let n = samples.len().next_power_of_two();
    let w = hann(n);
    let mut data = vec![0.0f64; 2 * n];
    for (i, &s) in samples.iter().take(n).enumerate() {
        data[2 * i] = f64::from(s) * w[i];
    }
    fft_in_place(&mut data);
    (0..=n / 2)
        .map(|k| {
            let re = data[2 * k];
            let im = data[2 * k + 1];
            (re * re + im * im).sqrt() / (n as f64)
        })
        .collect()
}

/// Bin index nearest to `hz` for a frame of `n` samples at `sample_rate`.
#[must_use]
pub fn bin_for_hz(hz: f64, n: usize, sample_rate: u32) -> usize {
    let bin = (hz * n as f64 / f64::from(sample_rate)).round() as usize;
    bin.min(n / 2)
}

/// Peak magnitude above `from_hz`, and the frequency it occurred at.
#[must_use]
pub fn peak_above(mag: &[f64], from_hz: f64, n: usize, sample_rate: u32) -> (f64, f64) {
    let start = bin_for_hz(from_hz, n, sample_rate);
    let mut best = (0.0f64, 0.0f64);
    for (k, &v) in mag.iter().enumerate().skip(start) {
        if v > best.0 {
            best = (v, k as f64 * f64::from(sample_rate) / n as f64);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn dft(x: &[f64], k: usize) -> (f64, f64) {
        let n = x.len();
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in x.iter().enumerate() {
            let a = -2.0 * std::f64::consts::PI * k as f64 * i as f64 / n as f64;
            re += v * a.cos();
            im += v * a.sin();
        }
        (re, im)
    }

    #[test]
    fn fft_matches_brute_force_dft() {
        let n = 64;
        // A deterministic, non-trivial signal.
        let x: Vec<f64> =
            (0..n).map(|i| (i as f64 * 0.3).sin() + 0.5 * (i as f64 * 1.7).cos() - 0.2).collect();
        let mut data = vec![0.0f64; 2 * n];
        for (i, &v) in x.iter().enumerate() {
            data[2 * i] = v;
        }
        fft_in_place(&mut data);
        for k in 0..n {
            let (re, im) = dft(&x, k);
            assert!((data[2 * k] - re).abs() < 1e-9, "bin {k} real: {} vs {re}", data[2 * k]);
            assert!(
                (data[2 * k + 1] - im).abs() < 1e-9,
                "bin {k} imag: {} vs {im}",
                data[2 * k + 1]
            );
        }
    }

    #[test]
    fn sine_shows_up_in_the_right_bin() {
        let sr = 48_000u32;
        let n = 4096;
        let f = 1000.0;
        let x: Vec<f32> = (0..n)
            .map(|i| ((2.0 * std::f64::consts::PI * f * i as f64 / f64::from(sr)).sin()) as f32)
            .collect();
        let mag = magnitude_spectrum(&x);
        let k = bin_for_hz(f, n, sr);
        let peak =
            mag[k..k + 3].iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap();
        assert_eq!(k + peak.0, k, "peak should be at or immediately above bin {k}");
        let (above, _) = peak_above(&mag, 3000.0, n, sr);
        assert!(mag[k] > above * 10.0, "1 kHz sine should dominate everything above 3 kHz");
    }

    #[test]
    #[should_panic]
    fn non_power_of_two_panics() {
        let mut d = vec![0.0f64; 2 * 100];
        fft_in_place(&mut d);
    }
}
