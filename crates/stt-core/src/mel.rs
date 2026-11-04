//! Log-mel spectrogram exactly as OpenAI Whisper computes it (`whisper/audio.py`): periodic Hann
//! window, n_fft 400, hop 160, centred STFT with reflect padding, Slaney mel filter bank,
//! `log10`, clamp to max − 8, `(x + 4) / 4`.
//!
//! Own implementation instead of `candle_transformers::…::audio::pcm_to_mel`, which spawns threads
//! and therefore fails in the browser. Filters are computed (librosa `mel`, Slaney) instead of
//! being shipped as binary files.

use realfft::RealFftPlanner;

pub const N_FFT: usize = 400;
pub const HOP: usize = 160;
const N_BINS: usize = N_FFT / 2 + 1;

/// Mel filter bank `[n_mels][N_BINS]` for 16 kHz (librosa.filters.mel, htk=False, norm="slaney").
pub fn mel_filters(n_mels: usize) -> Vec<f32> {
    let sr = 16_000.0_f64;
    let fft_freqs: Vec<f64> = (0..N_BINS).map(|i| i as f64 * sr / N_FFT as f64).collect();
    let (min_mel, max_mel) = (hz_to_mel(0.0), hz_to_mel(sr / 2.0));
    let mel_f: Vec<f64> = (0..n_mels + 2)
        .map(|i| mel_to_hz(min_mel + (max_mel - min_mel) * i as f64 / (n_mels + 1) as f64))
        .collect();
    let mut out = vec![0f32; n_mels * N_BINS];
    for m in 0..n_mels {
        let (lo, center, hi) = (mel_f[m], mel_f[m + 1], mel_f[m + 2]);
        let enorm = 2.0 / (hi - lo);
        for (k, &f) in fft_freqs.iter().enumerate() {
            let lower = (f - lo) / (center - lo);
            let upper = (hi - f) / (hi - center);
            out[m * N_BINS + k] = (lower.min(upper).max(0.0) * enorm) as f32;
        }
    }
    out
}

fn hz_to_mel(f: f64) -> f64 {
    const F_SP: f64 = 200.0 / 3.0;
    const MIN_LOG_HZ: f64 = 1000.0;
    let min_log_mel = MIN_LOG_HZ / F_SP;
    let logstep = 6.4_f64.ln() / 27.0;
    if f >= MIN_LOG_HZ { min_log_mel + (f / MIN_LOG_HZ).ln() / logstep } else { f / F_SP }
}

fn mel_to_hz(m: f64) -> f64 {
    const F_SP: f64 = 200.0 / 3.0;
    const MIN_LOG_HZ: f64 = 1000.0;
    let min_log_mel = MIN_LOG_HZ / F_SP;
    let logstep = 6.4_f64.ln() / 27.0;
    if m >= min_log_mel { MIN_LOG_HZ * (logstep * (m - min_log_mel)).exp() } else { F_SP * m }
}

/// Log-mel spectrogram of `samples` (16 kHz mono), laid out `[n_mels][n_frames]` with
/// `n_frames = samples.len() / HOP`. Callers pad to 30 s (480 000 samples → 3000 frames).
pub fn log_mel(samples: &[f32], filters: &[f32], n_mels: usize) -> Vec<f32> {
    let n_frames = samples.len() / HOP;
    if n_frames == 0 || filters.len() != n_mels * N_BINS {
        return vec![0.0; n_mels * n_frames];
    }
    // centre=True: reflect-pad n_fft/2 on both sides
    let pad = N_FFT / 2;
    let mut padded = Vec::with_capacity(samples.len() + 2 * pad);
    padded.extend((1..=pad).rev().map(|i| samples.get(i).copied().unwrap_or(0.0)));
    padded.extend_from_slice(samples);
    let n = samples.len();
    padded.extend(
        (1..=pad)
            .map(|i| n.checked_sub(1 + i).and_then(|j| samples.get(j)).copied().unwrap_or(0.0)),
    );

    let window: Vec<f32> = (0..N_FFT)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / N_FFT as f32).cos()))
        .collect();
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N_FFT);
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut power = vec![0f32; N_BINS];

    let mut mel = vec![0f32; n_mels * n_frames];
    for t in 0..n_frames {
        let start = t * HOP;
        for (i, x) in input.iter_mut().enumerate() {
            *x = padded.get(start + i).copied().unwrap_or(0.0) * window[i];
        }
        if fft.process(&mut input, &mut spectrum).is_err() {
            continue;
        }
        for (p, c) in power.iter_mut().zip(&spectrum) {
            *p = c.norm_sqr();
        }
        for m in 0..n_mels {
            let row = &filters[m * N_BINS..(m + 1) * N_BINS];
            let sum: f32 = row.iter().zip(&power).map(|(w, p)| w * p).sum();
            mel[m * n_frames + t] = sum.max(1e-10).log10();
        }
    }
    let max = mel.iter().copied().fold(f32::NEG_INFINITY, f32::max) - 8.0;
    for v in mel.iter_mut() {
        *v = (v.max(max) + 4.0) / 4.0;
    }
    mel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mel_scale_round_trips() {
        for f in [0.0, 440.0, 1000.0, 4000.0, 8000.0] {
            assert!((mel_to_hz(hz_to_mel(f)) - f).abs() < 1e-6);
        }
    }

    #[test]
    fn filters_have_expected_shape_and_are_triangles() {
        let f = mel_filters(80);
        assert_eq!(f.len(), 80 * N_BINS);
        assert!(f.iter().all(|w| *w >= 0.0));
        for m in 0..80 {
            assert!(f[m * N_BINS..(m + 1) * N_BINS].iter().any(|w| *w > 0.0), "empty band {m}");
        }
    }

    #[test]
    fn tone_peaks_in_the_right_band() {
        let sr = 16_000.0;
        let samples: Vec<f32> = (0..16_000)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
            .collect();
        let filters = mel_filters(80);
        let mel = log_mel(&samples, &filters, 80);
        let frames = samples.len() / HOP;
        let t = frames / 2;
        let band = (0..80).max_by(|&a, &b| mel[a * frames + t].total_cmp(&mel[b * frames + t]));
        // 1 kHz lies at mel 15 → band ≈ 15 / (45.2 / 81) − 1 ≈ 26
        let band = band.unwrap();
        assert!((25..=28).contains(&band), "peak in band {band}");
    }
}

#[cfg(test)]
mod reference {
    /// Peaks of a few bands, taken from OpenAI's `mel_filters.npz` (80 bins). The whole bank was
    /// compared once: max. deviation 2e-9.
    #[test]
    fn matches_official_whisper_filters() {
        let ours = super::mel_filters(80);
        for (m, k, expected) in [
            (0, 1, 0.024_862_595_f32),
            (10, 10, 0.019_908_218_f32),
            (40, 43, 0.014_735_566_f32),
            (79, 192, 0.003_164_711_8_f32),
        ] {
            let got = ours[m * super::N_BINS + k];
            assert!((got - expected).abs() < 1e-7, "band {m} bin {k}: {got} vs {expected}");
        }
    }
}
