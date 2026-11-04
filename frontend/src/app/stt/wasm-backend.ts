import { Observable, Subject, filter, map } from 'rxjs';
import { checkBrowserSupport } from './browser-support';
import { TranscriptionBackend } from './stt-backend';
import { WorkerRequest, WorkerResponse } from './stt-worker-protocol';
import {
  InputDevice,
  ModelId,
  ModelInfo,
  SttError,
  SttStatus,
  TranscriptSegment,
  toSttError,
} from './stt.models';

/**
 * Models offered in the browser (subset of `stt-core/src/models.rs`, display data only; the worker
 * resolves the files). Phase 4 takes this list from stt-wasm (`listModels`).
 */
const WEB_MODELS: ModelInfo[] = [
  {
    id: 'tiny-q',
    label: 'Tiny (quantisiert)',
    quantized: true,
    sizeMb: 44,
    melBins: 80,
    files: [],
  },
  {
    id: 'base-q',
    label: 'Base (quantisiert)',
    quantized: true,
    sizeMb: 83,
    melBins: 80,
    files: [],
  },
  { id: 'tiny', label: 'Tiny', quantized: false, sizeMb: 154, melBins: 80, files: [] },
  { id: 'base', label: 'Base', quantized: false, sizeMb: 293, melBins: 80, files: [] },
];

type Request = WorkerRequest extends infer R ? (R extends { id: number } ? R : never) : never;
type RequestWithoutId = Request extends infer R
  ? R extends Request
    ? Omit<R, 'id'>
    : never
  : never;

/** Browser backend: AudioWorklet → Web Worker → stt-wasm (Whisper on the CPU, WASM SIMD). */
export class WasmBackend implements TranscriptionBackend {
  readonly platform = 'web' as const;
  private readonly worker = new Worker(new URL('./stt.worker', import.meta.url), {
    type: 'module',
  });
  private readonly messages = new Subject<WorkerResponse>();
  private nextId = 1;

  readonly segments$: Observable<TranscriptSegment> = this.messages.pipe(
    filter((m) => m.type === 'segment'),
    map((m) => m.segment),
  );
  readonly status$: Observable<SttStatus> = this.messages.pipe(
    filter((m) => m.type === 'status'),
    map((m) => m.status),
  );
  readonly level$: Observable<number> = this.messages.pipe(
    filter((m) => m.type === 'level'),
    map((m) => m.level),
  );
  readonly errors$: Observable<SttError> = this.messages.pipe(
    filter((m) => m.type === 'error'),
    map((m) => m.error),
  );

  constructor() {
    this.worker.addEventListener('message', (ev: MessageEvent<WorkerResponse>) =>
      this.messages.next(ev.data),
    );
  }

  async listModels(): Promise<ModelInfo[]> {
    return WEB_MODELS;
  }

  async loadModel(id: ModelId, onProgress: (p: number) => void): Promise<void> {
    const sub = this.messages.subscribe((m) => {
      if (m.type === 'progress' && m.modelId === id && m.total > 0)
        onProgress(m.received / m.total);
    });
    try {
      await this.request({ type: 'load', modelId: id });
    } finally {
      sub.unsubscribe();
    }
  }

  async start(opts: { language: 'auto' | string }): Promise<void> {
    const unsupported = checkBrowserSupport();
    if (unsupported) throw unsupported;
    // Phase 4: getUserMedia + AudioWorklet (`/worklets/audio-capture.js`) → worker.
    await this.request({ type: 'start', sampleRate: 16_000, language: opts.language });
  }

  async stop(): Promise<void> {
    await this.request({ type: 'stop' });
  }

  async listInputDevices(): Promise<InputDevice[]> {
    return [];
  }

  async setInputDevice(): Promise<void> {
    // The browser chooses the device itself (getUserMedia permission prompt).
  }

  private request(msg: RequestWithoutId): Promise<void> {
    const id = this.nextId++;
    return new Promise<void>((resolve, reject) => {
      const sub = this.messages.subscribe((m) => {
        if ((m.type === 'done' || m.type === 'failed') && m.id === id) {
          sub.unsubscribe();
          if (m.type === 'done') resolve();
          else reject(toSttError(m.error));
        }
      });
      this.worker.postMessage({ ...msg, id } as WorkerRequest);
    });
  }
}
