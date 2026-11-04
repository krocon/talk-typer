//! Downmix to mono and streaming resampling to 16 kHz.

use crate::{SttError, SttResult, WHISPER_SAMPLE_RATE};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

/// Averages interleaved frames of `channels` channels into mono.
pub fn downmix(interleaved: &[f32], channels: usize) -> SttResult<Vec<f32>> {
    match channels {
        0 => Err(SttError::InvalidAudio("zero channels".into())),
        1 => Ok(interleaved.to_vec()),
        n => {
            if !interleaved.len().is_multiple_of(n) {
                return Err(SttError::InvalidAudio(format!(
                    "{} samples are not a multiple of {n} channels",
                    interleaved.len()
                )));
            }
            let scale = 1.0 / n as f32;
            Ok(interleaved.chunks_exact(n).map(|frame| frame.iter().sum::<f32>() * scale).collect())
        }
    }
}

const CHUNK: usize = 1024;

/// Streaming mono resampler from any rate to [`WHISPER_SAMPLE_RATE`].
/// Feed blocks of any size with [`push`](Self::push); call [`flush`](Self::flush) at the end.
pub struct Resampler16k {
    inner: Option<Fft<f32>>,
    pending: Vec<f32>,
}

impl Resampler16k {
    pub fn new(input_rate: u32) -> SttResult<Self> {
        if input_rate == 0 {
            return Err(SttError::InvalidAudio("sample rate 0".into()));
        }
        let inner = if input_rate == WHISPER_SAMPLE_RATE {
            None
        } else {
            Some(
                Fft::<f32>::new(
                    input_rate as usize,
                    WHISPER_SAMPLE_RATE as usize,
                    CHUNK,
                    1,
                    FixedSync::Input,
                )
                .map_err(|e| SttError::Resample(e.to_string()))?,
            )
        };
        Ok(Self { inner, pending: Vec::new() })
    }

    /// Resamples as much of the buffered input as possible.
    pub fn push(&mut self, mono: &[f32]) -> SttResult<Vec<f32>> {
        let Some(rs) = self.inner.as_mut() else {
            return Ok(mono.to_vec());
        };
        self.pending.extend_from_slice(mono);
        let mut out = Vec::new();
        loop {
            let need = rs.input_frames_next();
            if self.pending.len() < need {
                break;
            }
            let input = InterleavedSlice::new(&self.pending[..need], 1, need)
                .map_err(|e| SttError::Resample(e.to_string()))?;
            let block = rs.process(&input, None).map_err(|e| SttError::Resample(e.to_string()))?;
            out.extend_from_slice(&block.take_data());
            self.pending.drain(..need);
        }
        Ok(out)
    }

    /// Pads the remaining input with silence and returns the rest.
    pub fn flush(&mut self) -> SttResult<Vec<f32>> {
        let Some(rs) = self.inner.as_mut() else {
            return Ok(Vec::new());
        };
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }
        let need = rs.input_frames_next();
        let mut buf = std::mem::take(&mut self.pending);
        let real = buf.len().min(need);
        buf.resize(need, 0.0);
        let input =
            InterleavedSlice::new(&buf, 1, need).map_err(|e| SttError::Resample(e.to_string()))?;
        let block = rs.process(&input, None).map_err(|e| SttError::Resample(e.to_string()))?;
        let mut data = block.take_data();
        let keep = (real as u64 * WHISPER_SAMPLE_RATE as u64 / need as u64 * data.len() as u64
            / WHISPER_SAMPLE_RATE as u64) as usize;
        data.truncate(keep.min(data.len()));
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2).unwrap(), vec![0.5, 0.5]);
        assert!(downmix(&[1.0, 0.0, 0.5], 2).is_err());
        assert!(downmix(&[1.0], 0).is_err());
    }

    #[test]
    fn resampling_48k_to_16k_keeps_duration() {
        let mut rs = Resampler16k::new(48_000).unwrap();
        let second = vec![0.1_f32; 48_000];
        let mut out = rs.push(&second).unwrap();
        out.extend(rs.flush().unwrap());
        let diff = (out.len() as i64 - 16_000).abs();
        assert!(diff < 1_200, "got {} samples", out.len());
    }

    #[test]
    fn passthrough_at_16k() {
        let mut rs = Resampler16k::new(16_000).unwrap();
        assert_eq!(rs.push(&[0.25; 10]).unwrap().len(), 10);
    }
}
