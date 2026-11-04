import { isTauri } from '@tauri-apps/api/core';
import { TranscriptionBackend } from './stt-backend';

/**
 * Default (dev) variant: picks the backend at runtime. The `desktop` and `web` build
 * configurations replace this file (angular.json `fileReplacements`) so each bundle only
 * contains its own backend.
 */
export async function loadBackend(): Promise<TranscriptionBackend> {
  if (isTauri()) {
    const { TauriBackend } = await import('./tauri-backend');
    return new TauriBackend();
  }
  const { WasmBackend } = await import('./wasm-backend');
  return new WasmBackend();
}
