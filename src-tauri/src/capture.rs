//! Microphone capture with cpal. The stream lives on the session thread that created it; the
//! audio callback only converts samples to `f32` and hands them over through a channel.

use crate::error::{AppError, AppResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

/// Interleaved audio from the callback, or a stream error.
pub enum AudioMsg {
    Samples(Vec<f32>),
    Error(String),
}

pub struct Capture {
    /// Keeps the stream running; dropping it stops the microphone.
    _stream: Stream,
    pub sample_rate: u32,
    pub channels: usize,
}

/// Opens `device_id` (or the default input) and starts streaming into `tx`. `backlog` counts the
/// samples sent but not yet processed (used to skip preliminary results when behind).
pub fn open(
    device_id: Option<&str>,
    tx: Sender<AudioMsg>,
    backlog: Arc<AtomicUsize>,
) -> AppResult<Capture> {
    let host = cpal::default_host();
    let device = match device_id {
        Some(id) => host
            .input_devices()
            .map_err(map_err)?
            .find(|d| d.id().is_ok_and(|d| d.to_string() == id))
            .ok_or(AppError::NoInputDevice)?,
        None => host.default_input_device().ok_or(AppError::NoInputDevice)?,
    };
    let config = device.default_input_config().map_err(map_err)?;
    let sample_rate = config.sample_rate();
    let channels = usize::from(config.channels());
    let format = config.sample_format();
    let stream_config = config.config();

    let err_tx = tx.clone();
    let on_error = move |e: cpal::Error| {
        let _ = err_tx.send(AudioMsg::Error(e.to_string()));
    };
    macro_rules! build {
        ($t:ty, $conv:expr) => {{
            let backlog = backlog.clone();
            device.build_input_stream(
                stream_config.clone(),
                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                    let samples: Vec<f32> = data.iter().map($conv).collect();
                    backlog.fetch_add(samples.len(), Ordering::Relaxed);
                    let _ = tx.send(AudioMsg::Samples(samples));
                },
                on_error,
                None,
            )
        }};
    }
    let stream = match format {
        SampleFormat::F32 => build!(f32, |s: &f32| *s),
        SampleFormat::I16 => build!(i16, |s: &i16| f32::from(*s) / 32_768.0),
        SampleFormat::U16 => build!(u16, |s: &u16| (f32::from(*s) - 32_768.0) / 32_768.0),
        SampleFormat::I32 => build!(i32, |s: &i32| *s as f32 / 2_147_483_648.0),
        other => return Err(AppError::AudioDevice(format!("unsupported sample format {other}"))),
    }
    .map_err(map_err)?;
    stream.play().map_err(map_err)?;
    Ok(Capture { _stream: stream, sample_rate, channels })
}

/// cpal errors → app errors; a blocked microphone (privacy settings) gets its own code.
fn map_err(e: cpal::Error) -> AppError {
    if e.kind() == cpal::ErrorKind::PermissionDenied {
        return AppError::MicrophoneDenied;
    }
    map_message(&e.to_string())
}

/// Fallback for backends that report access problems only in the message (WASAPI E_ACCESSDENIED).
fn map_message(msg: &str) -> AppError {
    let lower = msg.to_lowercase();
    if lower.contains("access is denied") || lower.contains("0x80070005") {
        AppError::MicrophoneDenied
    } else {
        AppError::AudioDevice(msg.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_errors_map_to_microphone_denied() {
        let denied = cpal::Error::new(cpal::ErrorKind::PermissionDenied);
        assert!(matches!(map_err(denied), AppError::MicrophoneDenied));
        assert!(matches!(
            map_message("Access is denied. (0x80070005)"),
            AppError::MicrophoneDenied
        ));
        assert!(matches!(map_message("device busy"), AppError::AudioDevice(_)));
    }
}
