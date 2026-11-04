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

type Request = Extract<WorkerRequest, { id: number }>;
type RequestWithoutId = Request extends infer R
  ? R extends Request
    ? Omit<R, 'id'>
    : never
  : never;

interface Capture {
  stream: MediaStream;
  context: AudioContext;
  node: AudioWorkletNode;
}

/** getUserMedia errors → app error codes. */
function micError(e: unknown): SttError {
  const name = e instanceof DOMException ? e.name : '';
  if (name === 'NotAllowedError' || name === 'SecurityError') {
    return { code: 'microphone_denied', message: name };
  }
  if (name === 'NotFoundError' || name === 'OverconstrainedError') {
    return { code: 'no_input_device', message: name };
  }
  if (name === 'NotReadableError') return { code: 'audio_device', message: name };
  return toSttError(e);
}

/** Browser backend: AudioWorklet → Web Worker → stt-wasm (Whisper on the CPU, WASM SIMD). */
export class WasmBackend implements TranscriptionBackend {
  readonly platform = 'web' as const;
  private readonly worker = new Worker(new URL('./stt.worker', import.meta.url), {
    type: 'module',
  });
  private readonly messages = new Subject<WorkerResponse>();
  private nextId = 1;
  private capture: Capture | null = null;

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
    const id = this.nextId++;
    const reply = new Promise<ModelInfo[]>((resolve, reject) => {
      const sub = this.messages.subscribe((m) => {
        if (m.type === 'models' && m.id === id) {
          sub.unsubscribe();
          resolve(m.models);
        } else if (m.type === 'failed' && m.id === id) {
          sub.unsubscribe();
          reject(m.error);
        }
      });
    });
    this.worker.postMessage({ type: 'list', id } satisfies WorkerRequest);
    return reply;
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
    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        audio: { channelCount: 1, echoCancellation: false, noiseSuppression: false },
      });
    } catch (e) {
      throw micError(e);
    }
    const context = new AudioContext();
    try {
      await context.audioWorklet.addModule(new URL('worklets/audio-capture.js', document.baseURI));
      const source = context.createMediaStreamSource(stream);
      const node = new AudioWorkletNode(context, 'audio-capture', {
        numberOfInputs: 1,
        numberOfOutputs: 1,
        channelCount: 1,
        channelCountMode: 'explicit',
      });
      // Some engines (Safari) only run a worklet that is connected to the destination: silent gain.
      const mute = context.createGain();
      mute.gain.value = 0;
      source.connect(node).connect(mute).connect(context.destination);
      await this.request({
        type: 'start',
        sampleRate: context.sampleRate,
        channels: 1,
        language: opts.language,
      });
      node.port.onmessage = (ev: MessageEvent<Float32Array>) => {
        const samples = ev.data;
        this.worker.postMessage(
          { type: 'audio', samples, sentAt: Date.now() } satisfies WorkerRequest,
          [samples.buffer],
        );
      };
      if (context.state === 'suspended') await context.resume();
      this.capture = { stream, context, node };
    } catch (e) {
      stream.getTracks().forEach((t) => t.stop());
      void context.close();
      throw toSttError(e);
    }
  }

  async stop(): Promise<void> {
    const c = this.capture;
    this.capture = null;
    if (c) {
      c.node.port.onmessage = null;
      c.node.disconnect();
      c.stream.getTracks().forEach((t) => t.stop());
      await c.context.close();
    }
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
