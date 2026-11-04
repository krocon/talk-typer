import { ModelId, ModelInfo, SttError, SttStatus, TranscriptSegment } from './stt.models';

/** Messages main thread → `stt.worker.ts`. */
export type WorkerRequest =
  | { type: 'list'; id: number }
  | { type: 'load'; id: number; modelId: ModelId }
  | { type: 'start'; id: number; sampleRate: number; channels: number; language: string }
  /** `sentAt` (Date.now()) lets the worker notice when it falls behind. */
  | { type: 'audio'; samples: Float32Array; sentAt: number }
  | { type: 'stop'; id: number };

/** Messages `stt.worker.ts` → main thread. */
export type WorkerResponse =
  | { type: 'done'; id: number }
  | { type: 'models'; id: number; models: ModelInfo[] }
  | { type: 'failed'; id: number; error: SttError }
  | { type: 'progress'; modelId: ModelId; received: number; total: number }
  | { type: 'segment'; segment: TranscriptSegment }
  | { type: 'status'; status: SttStatus }
  | { type: 'level'; level: number }
  | { type: 'error'; error: SttError };
