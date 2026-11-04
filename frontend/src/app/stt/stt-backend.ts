import { InjectionToken } from '@angular/core';
import { Observable } from 'rxjs';
import {
  InputDevice,
  ModelId,
  ModelInfo,
  SttError,
  SttStatus,
  TranscriptSegment,
} from './stt.models';

/**
 * One speech-to-text engine. Desktop: native Whisper in Rust (`TauriBackend`);
 * browser: Whisper as WASM in a Web Worker (`WasmBackend`).
 */
export interface TranscriptionBackend {
  /** `desktop` or `web` – for platform-specific UI (device selection, HTTPS hint). */
  readonly platform: 'desktop' | 'web';
  listModels(): Promise<ModelInfo[]>;
  loadModel(id: ModelId, onProgress: (p: number) => void): Promise<void>;
  start(opts: { language: 'auto' | string }): Promise<void>;
  stop(): Promise<void>;
  /** Input devices (desktop only; the browser picks via getUserMedia). */
  listInputDevices(): Promise<InputDevice[]>;
  setInputDevice(id: string | null): Promise<void>;
  readonly segments$: Observable<TranscriptSegment>;
  readonly status$: Observable<SttStatus>;
  /** Input level (RMS, 0…1) for the level meter and the waveform. */
  readonly level$: Observable<number>;
  /** Errors that happen while listening (device lost, microphone blocked …). */
  readonly errors$: Observable<SttError>;
}

/** Bound in `main.ts` after the platform's backend module was loaded lazily. */
export const TRANSCRIPTION_BACKEND = new InjectionToken<TranscriptionBackend>(
  'TranscriptionBackend',
);
