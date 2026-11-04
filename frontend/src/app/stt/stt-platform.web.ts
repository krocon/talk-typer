import { TranscriptionBackend } from './stt-backend';

/** Web build: Whisper as WASM in a Web Worker – no Tauri API in the bundle. */
export async function loadBackend(): Promise<TranscriptionBackend> {
  const { WasmBackend } = await import('./wasm-backend');
  return new WasmBackend();
}
