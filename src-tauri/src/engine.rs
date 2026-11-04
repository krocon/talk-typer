//! Speech engine of the desktop app: the loaded Whisper model and the listening session.
//!
//! A session runs on its own thread: cpal capture → downmix → resample to 16 kHz →
//! `stt_core::pipeline` (VAD, chunking, Whisper) → events to the frontend. Recognition never runs
//! on the UI thread or the async runtime.

use crate::capture::{self, AudioMsg};
use crate::error::{AppError, AppResult};
use crate::events::{self, SttStatus};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use stt_core::audio::{Resampler16k, downmix};
use stt_core::chunker::{ChunkKind, ChunkerConfig};
use stt_core::pipeline::Pipeline;
use stt_core::whisper::{ModelBytes, Transcriber, best_device};
use stt_core::{ModelId, Segment};
use tauri::{AppHandle, Emitter};

/// After this much audio of pure digital zeros the microphone is considered blocked
/// (macOS delivers silence instead of an error when access was denied).
const ZERO_WATCHDOG: Duration = Duration::from_millis(1500);
/// Level events per second.
const LEVEL_HZ: u32 = 25;

struct Loaded {
    id: ModelId,
    transcriber: Arc<Mutex<Transcriber>>,
}

struct Session {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

#[derive(Default)]
pub struct Engine {
    loaded: Mutex<Option<Loaded>>,
    session: Mutex<Option<Session>>,
    device: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> AppResult<MutexGuard<'_, T>> {
    m.lock().map_err(|e| AppError::Internal(e.to_string()))
}

impl Engine {
    pub fn loaded_model(&self) -> Option<ModelId> {
        lock(&self.loaded).ok().and_then(|l| l.as_ref().map(|l| l.id))
    }

    /// Loads a model and warms it up (first Metal run compiles shaders). Blocking.
    pub fn load(&self, id: ModelId, bytes: ModelBytes) -> AppResult<()> {
        let device = best_device();
        let mut t = Transcriber::load(bytes, &device)?;
        t.transcribe_window(&[0.0; 16_000], 0, Some("en"))?;
        *lock(&self.loaded)? = Some(Loaded { id, transcriber: Arc::new(Mutex::new(t)) });
        Ok(())
    }

    pub fn set_device(&self, id: Option<String>) -> AppResult<()> {
        *lock(&self.device)? = id;
        Ok(())
    }

    /// Starts listening. Returns once the microphone is open (or failed to open).
    pub fn start(&self, app: AppHandle, language: Option<String>) -> AppResult<()> {
        let mut session = lock(&self.session)?;
        if session.as_ref().is_some_and(|s| !s.thread.is_finished()) {
            return Err(AppError::AlreadyListening);
        }
        let transcriber = lock(&self.loaded)?
            .as_ref()
            .map(|l| l.transcriber.clone())
            .ok_or(stt_core::SttError::NoModel)?;
        let device = lock(&self.device)?.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let stop2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("stt-session".into())
            .spawn(move || {
                let run = SessionRun { app: app.clone(), transcriber, language, stop: stop2 };
                if let Err(e) = run.run(device.as_deref(), ready_tx) {
                    emit_error(&app, &e);
                }
                let _ = app.emit(events::STATUS, SttStatus::Idle);
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;
        *session = Some(Session { stop, thread });
        drop(session);
        ready_rx.recv().map_err(|_| AppError::Internal("session thread ended".into()))?
    }

    /// Stops listening; the remaining speech is transcribed before this returns. Blocking.
    pub fn stop(&self) -> AppResult<()> {
        let session = lock(&self.session)?.take();
        if let Some(s) = session {
            s.stop.store(true, Ordering::Relaxed);
            s.thread.join().map_err(|_| AppError::Internal("session thread panicked".into()))?;
        }
        Ok(())
    }
}

fn emit_error(app: &AppHandle, e: &AppError) {
    let _ = app.emit(events::ERROR, e);
    let _ = app.emit(events::STATUS, SttStatus::Error);
}

struct SessionRun {
    app: AppHandle,
    transcriber: Arc<Mutex<Transcriber>>,
    language: Option<String>,
    stop: Arc<AtomicBool>,
}

impl SessionRun {
    fn run(&self, device: Option<&str>, ready: mpsc::SyncSender<AppResult<()>>) -> AppResult<()> {
        let (tx, rx) = mpsc::channel();
        let backlog = Arc::new(AtomicUsize::new(0));
        let capture = match capture::open(device, tx, backlog.clone()) {
            Ok(c) => c,
            Err(e) => {
                // reported by `start_listening` itself
                let _ = ready.send(Err(e));
                return Ok(());
            }
        };
        let _ = ready.send(Ok(()));
        let _ = self.app.emit(events::STATUS, SttStatus::Listening);

        let mut t = lock(&self.transcriber)?;
        let (sample_rate, channels) = (capture.sample_rate, capture.channels);
        let rate_samples = sample_rate as usize * channels;
        let mut resampler = Resampler16k::new(sample_rate)?;
        let mut pipeline = Pipeline::new(ChunkerConfig::default(), self.language.clone());
        let mut capture = Some(capture);
        let mut level_at = Instant::now();
        let mut zero_samples = 0usize;
        let mut heard_signal = false;

        loop {
            if capture.is_some() && self.stop.load(Ordering::Relaxed) {
                // Stops the microphone; its sender goes away, so the channel drains and then
                // reports `Disconnected`.
                capture = None;
            }
            let raw = match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(AudioMsg::Samples(raw)) => raw,
                Ok(AudioMsg::Error(e)) => return Err(AppError::AudioDevice(e)),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            backlog.fetch_sub(raw.len(), Ordering::Relaxed);
            if !heard_signal {
                if raw.iter().any(|s| *s != 0.0) {
                    heard_signal = true;
                } else {
                    zero_samples += raw.len();
                    if zero_samples as f64 / rate_samples as f64 > ZERO_WATCHDOG.as_secs_f64() {
                        return Err(AppError::MicrophoneDenied);
                    }
                }
            }
            let mono = downmix(&raw, channels)?;
            let samples = resampler.push(&mono)?;
            self.process(&mut pipeline, &mut t, &samples, &backlog, rate_samples)?;
            if level_at.elapsed() >= Duration::from_secs(1) / LEVEL_HZ {
                level_at = Instant::now();
                let _ = self.app.emit(events::LEVEL, pipeline.level());
            }
        }
        let tail = resampler.flush()?;
        self.process(&mut pipeline, &mut t, &tail, &backlog, rate_samples)?;
        let _ = self.app.emit(events::STATUS, SttStatus::Transcribing);
        self.emit_segments(pipeline.flush(&mut t)?);
        let _ = self.app.emit(events::LEVEL, 0.0_f32);
        Ok(())
    }

    /// Runs the pipeline on new samples. Preliminary results are skipped while more than half a
    /// second of audio is waiting – final text has priority over live feedback.
    fn process(
        &self,
        pipeline: &mut Pipeline,
        t: &mut Transcriber,
        samples: &[f32],
        backlog: &AtomicUsize,
        rate_samples: usize,
    ) -> AppResult<()> {
        let mut chunks = pipeline.feed(samples);
        while !chunks.is_empty() {
            for chunk in &chunks {
                let behind = backlog.load(Ordering::Relaxed) > rate_samples / 2;
                if chunk.kind == ChunkKind::Partial && behind {
                    continue;
                }
                if chunk.kind != ChunkKind::Partial {
                    let _ = self.app.emit(events::STATUS, SttStatus::Transcribing);
                }
                let segments = pipeline.transcribe(t, chunk)?;
                self.emit_segments(segments);
                if chunk.kind != ChunkKind::Partial {
                    let _ = self.app.emit(events::STATUS, SttStatus::Listening);
                }
            }
            chunks = pipeline.feed(&[]);
        }
        Ok(())
    }

    fn emit_segments(&self, segments: Vec<Segment>) {
        for s in segments {
            let _ = self.app.emit(events::SEGMENT, s);
        }
    }
}
