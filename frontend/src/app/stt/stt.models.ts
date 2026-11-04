/**
 * DTOs shared by both backends. Kept in sync by hand with Rust:
 * `stt-core/src/models.rs`, `stt-core/src/segment.rs`, `src-tauri/src/events.rs`, `devices.rs`.
 */

export type ModelId =
  | 'tiny'
  | 'tiny-q'
  | 'base'
  | 'base-q'
  | 'small'
  | 'small-q'
  | 'large-v3-turbo'
  | 'large-v3-turbo-q';

export interface ModelFile {
  role: 'weights' | 'config' | 'tokenizer';
  url: string;
  sha256: string;
  /** Exact size in bytes. */
  size: number;
}

export interface ModelInfo {
  id: ModelId;
  label: string;
  quantized: boolean;
  /** Approximate download size in MB. */
  sizeMb: number;
  melBins: number;
  files: ModelFile[];
  /** Desktop: all files are in the local cache. */
  downloaded?: boolean;
  /** Desktop: currently loaded in memory. */
  loaded?: boolean;
}

export interface TranscriptSegment {
  text: string;
  startMs: number;
  endMs: number;
  /** `false` for preliminary text of a chunk that is still running. */
  isFinal: boolean;
}

export type SttStatus = 'idle' | 'loading' | 'listening' | 'transcribing' | 'error';

/** Persisted choices (desktop: tauri-plugin-store, browser: localStorage). */
export interface Settings {
  modelId?: ModelId | null;
  language?: string | null;
  deviceId?: string | null;
}

export interface InputDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

/** Error codes of `AppError` / `SttError` (Rust) plus browser-only ones. */
export type SttErrorCode =
  | 'invalid_audio'
  | 'unknown_model'
  | 'invalid_model'
  | 'invalid_tokenizer'
  | 'no_model'
  | 'resample'
  | 'inference'
  | 'audio_device'
  | 'microphone_denied'
  | 'no_input_device'
  | 'insecure_context'
  | 'unsupported_browser'
  | 'download_failed'
  | 'checksum_mismatch'
  | 'model_missing'
  | 'io'
  | 'already_listening'
  | 'internal'
  | 'not_implemented';

export interface SttError {
  code: SttErrorCode;
  message: string;
}

export function toSttError(e: unknown): SttError {
  if (typeof e === 'object' && e !== null && 'code' in e && typeof e.code === 'string') {
    const message = 'message' in e && typeof e.message === 'string' ? e.message : '';
    return { code: e.code as SttErrorCode, message };
  }
  return { code: 'internal', message: e instanceof Error ? e.message : String(e) };
}
