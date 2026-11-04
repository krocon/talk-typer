import { TranscriptionBackend } from './stt-backend';

/** Desktop build: native Whisper via Tauri only – no WASM in the bundle. */
export async function loadBackend(): Promise<TranscriptionBackend> {
  const { TauriBackend } = await import('./tauri-backend');
  return new TauriBackend();
}
