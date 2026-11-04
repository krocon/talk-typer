/// <reference lib="webworker" />
import { loadModelFile, ModelFileError } from './model-cache';
import { WorkerRequest, WorkerResponse } from './stt-worker-protocol';
import { ModelId, ModelInfo, SttError, SttErrorCode, TranscriptSegment } from './stt.models';

/**
 * Web Worker that owns the stt-wasm engine: loads the WASM module and the model (Cache Storage),
 * keeps the model in memory and transcribes the microphone blocks sent by `WasmBackend`.
 * Messages are handled one after another; audio that arrives during a transcription waits.
 */

/** Models offered in the browser (CPU/WASM): `small` is too slow there. */
const WEB_MODELS: readonly ModelId[] = ['tiny-q', 'base-q', 'tiny', 'base'];
/** Skip preliminary results when audio waits longer than this. */
const BEHIND_MS = 500;
/**
 * Chunking for the browser (WASM on the CPU, no threads – several times slower than native):
 * no preliminary results (each costs a full encoder run), chunks of at most 12 s and a short
 * closing pause, so text arrives in small portions instead of after a 30 s block.
 */
const PARTIAL_EVERY_MS = 0;
const MAX_CHUNK_MS = 12_000;
const PAUSE_MS = 300;
const LEVEL_EVERY_MS = 40;

interface SttEngineJs {
  start(
    sampleRate: number,
    channels: number,
    language: string,
    partialEveryMs: number,
    maxChunkMs: number,
    pauseMs: number,
  ): void;
  feed(samples: Float32Array): number;
  nextKind(): '' | 'final' | 'split' | 'partial';
  runNext(allowPartial: boolean): string;
  level(): number;
  stop(): string;
  free(): void;
}

interface SttWasmModule {
  default(init: { module_or_path: string | URL }): Promise<unknown>;
  listModels(): string;
  SttEngine: new (
    weights: Uint8Array,
    config: Uint8Array,
    tokenizer: Uint8Array,
    quantized: boolean,
  ) => SttEngineJs;
}

let wasm: Promise<SttWasmModule> | null = null;
let engine: { id: ModelId; engine: SttEngineJs } | null = null;
let levelAt = 0;

function post(msg: WorkerResponse): void {
  postMessage(msg);
}

/** `pnpm wasm` puts the module into `public/wasm/` – loaded at runtime, not bundled. */
function loadWasm(): Promise<SttWasmModule> {
  wasm ??= (async () => {
    const js = new URL('wasm/stt_wasm.js', self.location.href);
    const mod = (await import(/* @vite-ignore */ js.href)) as SttWasmModule;
    await mod.default({ module_or_path: new URL('wasm/stt_wasm_bg.wasm', self.location.href) });
    return mod;
  })();
  return wasm;
}

/** Rust errors arrive as "code: message". */
function toError(e: unknown): SttError {
  if (e instanceof ModelFileError) return { code: e.code, message: e.message };
  const text = e instanceof Error ? e.message : String(e);
  const m = /^([a-z_]+): (.*)$/s.exec(text);
  return m ? { code: m[1] as SttErrorCode, message: m[2] } : { code: 'internal', message: text };
}

async function models(): Promise<ModelInfo[]> {
  const all = JSON.parse((await loadWasm()).listModels()) as ModelInfo[];
  return WEB_MODELS.map((id) => all.find((m) => m.id === id)).filter((m) => m !== undefined);
}

async function load(modelId: ModelId): Promise<void> {
  if (engine?.id === modelId) return;
  const mod = await loadWasm();
  const info = (await models()).find((m) => m.id === modelId);
  if (!info) throw { code: 'unknown_model', message: modelId } satisfies SttError;
  post({ type: 'status', status: 'loading' });
  const total = info.files.reduce((sum, f) => sum + f.size, 0);
  const received = new Map<string, number>();
  let reportedAt = 0;
  const bytes = new Map<string, Uint8Array>();
  for (const file of info.files) {
    bytes.set(
      file.role,
      await loadModelFile(file, (n) => {
        received.set(file.role, n);
        const now = Date.now();
        if (now - reportedAt > 100) {
          reportedAt = now;
          const sum = [...received.values()].reduce((a, b) => a + b, 0);
          post({ type: 'progress', modelId, received: sum, total });
        }
      }),
    );
  }
  post({ type: 'progress', modelId, received: total, total });
  engine?.engine.free();
  engine = null;
  const get = (role: string): Uint8Array => bytes.get(role) ?? new Uint8Array();
  engine = {
    id: modelId,
    engine: new mod.SttEngine(get('weights'), get('config'), get('tokenizer'), info.quantized),
  };
}

function emitSegments(json: string): void {
  for (const segment of JSON.parse(json) as TranscriptSegment[]) {
    post({ type: 'segment', segment });
  }
}

function onAudio(samples: Float32Array, sentAt: number): void {
  if (!engine) return;
  const e = engine.engine;
  try {
    e.feed(samples);
    const now = Date.now();
    if (now - levelAt >= LEVEL_EVERY_MS) {
      levelAt = now;
      post({ type: 'level', level: e.level() });
    }
    for (let kind = e.nextKind(); kind !== ''; kind = e.nextKind()) {
      const heavy = kind !== 'partial';
      if (heavy) post({ type: 'status', status: 'transcribing' });
      emitSegments(e.runNext(Date.now() - sentAt <= BEHIND_MS));
      if (heavy) post({ type: 'status', status: 'listening' });
    }
  } catch (err) {
    post({ type: 'error', error: toError(err) });
  }
}

addEventListener('message', async ({ data }: MessageEvent<WorkerRequest>) => {
  if (data.type === 'audio') {
    onAudio(data.samples, data.sentAt);
    return;
  }
  try {
    switch (data.type) {
      case 'list':
        post({ type: 'models', id: data.id, models: await models() });
        return;
      case 'load':
        await load(data.modelId);
        break;
      case 'start':
        if (!engine) throw { code: 'no_model', message: 'no model loaded' } satisfies SttError;
        engine.engine.start(
          data.sampleRate,
          data.channels,
          data.language,
          PARTIAL_EVERY_MS,
          MAX_CHUNK_MS,
          PAUSE_MS,
        );
        post({ type: 'status', status: 'listening' });
        break;
      case 'stop':
        if (engine) {
          post({ type: 'status', status: 'transcribing' });
          emitSegments(engine.engine.stop());
          post({ type: 'level', level: 0 });
        }
        post({ type: 'status', status: 'idle' });
        break;
    }
    post({ type: 'done', id: data.id });
  } catch (e) {
    const error = typeof e === 'object' && e !== null && 'code' in e ? (e as SttError) : toError(e);
    post({ type: 'failed', id: data.id, error });
  }
});
