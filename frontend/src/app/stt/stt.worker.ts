/// <reference lib="webworker" />
import { WorkerRequest, WorkerResponse } from './stt-worker-protocol';

/**
 * Web Worker that owns the stt-wasm engine (phase 4: loads `stt_wasm.js`, keeps the model in
 * memory, caches the model files in Cache Storage). Phase 1: protocol only.
 */
function post(msg: WorkerResponse, transfer: Transferable[] = []): void {
  postMessage(msg, transfer);
}

addEventListener('message', ({ data }: MessageEvent<WorkerRequest>) => {
  switch (data.type) {
    case 'load':
    case 'start':
      post({
        type: 'failed',
        id: data.id,
        error: { code: 'not_implemented', message: 'WASM engine follows in phase 4' },
      });
      break;
    case 'stop':
      post({ type: 'done', id: data.id });
      break;
    case 'audio':
      break;
  }
});
