import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Observable, map } from 'rxjs';
import { TranscriptionBackend } from './stt-backend';
import {
  InputDevice,
  ModelId,
  ModelInfo,
  Settings,
  SttStatus,
  TranscriptSegment,
  toSttError,
} from './stt.models';

/** Event names = `src-tauri/src/events.rs`. */
const EV_SEGMENT = 'stt://segment';
const EV_STATUS = 'stt://status';
const EV_DOWNLOAD = 'stt://download-progress';
const EV_LEVEL = 'stt://level';
const EV_ERROR = 'stt://error';

interface DownloadProgress {
  modelId: ModelId;
  received: number;
  total: number;
}

/** The only place that calls `invoke()`; errors become `SttError`. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toSttError(e);
  }
}

function fromEvent<T>(name: string): Observable<T> {
  return new Observable<T>((sub) => {
    const unlisten = listen<T>(name, (ev) => sub.next(ev.payload));
    return () => void unlisten.then((fn) => fn());
  });
}

/** Desktop backend: Whisper runs natively in Rust, audio is captured with cpal. */
export class TauriBackend implements TranscriptionBackend {
  readonly platform = 'desktop' as const;
  readonly segments$ = fromEvent<TranscriptSegment>(EV_SEGMENT);
  readonly status$ = fromEvent<SttStatus>(EV_STATUS);
  readonly level$ = fromEvent<number>(EV_LEVEL);
  readonly errors$ = fromEvent<unknown>(EV_ERROR).pipe(map(toSttError));

  listModels(): Promise<ModelInfo[]> {
    return call<ModelInfo[]>('list_models');
  }

  async loadModel(id: ModelId, onProgress: (p: number) => void): Promise<void> {
    const unlisten = await listen<DownloadProgress>(EV_DOWNLOAD, (ev) => {
      const { modelId, received, total } = ev.payload;
      if (modelId === id && total > 0) onProgress(received / total);
    });
    try {
      await call<void>('download_model', { modelId: id });
      await call<void>('load_model', { modelId: id });
    } finally {
      unlisten();
    }
  }

  start(opts: { language: 'auto' | string }): Promise<void> {
    return call<void>('start_listening', { language: opts.language });
  }

  stop(): Promise<void> {
    return call<void>('stop_listening');
  }

  listInputDevices(): Promise<InputDevice[]> {
    return call<InputDevice[]>('list_input_devices');
  }

  setInputDevice(id: string | null): Promise<void> {
    return call<void>('set_input_device', { deviceId: id });
  }

  async saveText(fileName: string, contents: string): Promise<boolean> {
    return (await call<string | null>('save_text', { fileName, contents })) !== null;
  }

  loadSettings(): Promise<Settings> {
    return call<Settings>('load_settings');
  }

  saveSettings(settings: Settings): Promise<void> {
    return call<void>('save_settings', { settings });
  }
}
