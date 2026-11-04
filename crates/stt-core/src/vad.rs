//! Simple energy-based voice activity detection (RMS threshold + hangover).

/// VAD settings, all in 16 kHz samples / linear RMS.
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    /// Analysis frame length (30 ms at 16 kHz).
    pub frame_len: usize,
    /// RMS above which a frame counts as speech.
    pub threshold: f32,
    /// Frames of silence that still count as speech after the last loud frame.
    pub hangover_frames: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self { frame_len: 480, threshold: 0.01, hangover_frames: 15 }
    }
}

/// Streaming VAD: returns one decision per complete frame.
#[derive(Debug)]
pub struct Vad {
    cfg: VadConfig,
    pending: Vec<f32>,
    silent_run: usize,
    /// Smoothed level of the last frame (0…1), for the level meter.
    pub level: f32,
}

impl Vad {
    pub fn new(cfg: VadConfig) -> Self {
        Self { cfg, pending: Vec::new(), silent_run: usize::MAX, level: 0.0 }
    }

    /// Classifies the newly pushed samples frame by frame.
    pub fn push(&mut self, samples: &[f32]) -> Vec<bool> {
        self.pending.extend_from_slice(samples);
        let n = self.cfg.frame_len.max(1);
        let mut out = Vec::with_capacity(self.pending.len() / n);
        for frame in self.pending.chunks_exact(n) {
            let rms = rms(frame);
            self.level = rms;
            if rms >= self.cfg.threshold {
                self.silent_run = 0;
            } else {
                self.silent_run = self.silent_run.saturating_add(1);
            }
            out.push(self.silent_run <= self.cfg.hangover_frames);
        }
        let used = out.len() * n;
        self.pending.drain(..used);
        out
    }
}

pub fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_then_hangover_then_silence() {
        let cfg = VadConfig { frame_len: 10, threshold: 0.1, hangover_frames: 2 };
        let mut vad = Vad::new(cfg);
        let mut input = vec![0.5_f32; 10];
        input.extend(vec![0.0; 40]);
        assert_eq!(vad.push(&input), vec![true, true, true, false, false]);
    }

    #[test]
    fn silence_at_start_is_not_speech() {
        let mut vad = Vad::new(VadConfig { frame_len: 4, ..VadConfig::default() });
        assert_eq!(vad.push(&[0.0; 8]), vec![false, false]);
    }
}
