import { DestroyRef, Injectable, computed, effect, inject, signal } from '@angular/core';
import { LANGUAGES } from '../texts';
import { TRANSCRIPTION_BACKEND } from './stt-backend';
import {
  InputDevice,
  ModelId,
  ModelInfo,
  Settings,
  SttError,
  SttStatus,
  TranscriptSegment,
  toSttError,
} from './stt.models';

/** Level values kept for the waveform (25 per second → 16 s). */
const HISTORY = 400;

/** UI state of the transcriber (one per app). Talks to the backend only through the interface. */
@Injectable({ providedIn: 'root' })
export class SttStore {
  private readonly backend = inject(TRANSCRIPTION_BACKEND);

  readonly platform = this.backend.platform;
  readonly status = signal<SttStatus>('idle');
  readonly models = signal<ModelInfo[]>([]);
  /** Desktop: best quality, fast enough with Metal/GPU; browser: smallest (CPU/WASM). */
  readonly modelId = signal<ModelId>(this.platform === 'desktop' ? 'large-v3-turbo-q' : 'tiny-q');
  readonly language = signal<string>('de');
  readonly devices = signal<InputDevice[]>([]);
  readonly deviceId = signal<string | null>(null);
  readonly segments = signal<TranscriptSegment[]>([]);
  readonly preliminary = signal<string>('');
  readonly level = signal(0);
  /** Recent input levels (oldest first) for the waveform. */
  readonly levels = signal<number[]>([]);
  readonly progress = signal<number | null>(null);
  readonly error = signal<SttError | null>(null);
  /** `Date.now()` when listening started; `null` when not listening. */
  readonly startedAt = signal<number | null>(null);
  /** Duration of the last recording in ms (shown while idle). */
  readonly lastDuration = signal(0);
  /** Start/stop request in flight. */
  readonly busy = signal(false);
  /** Waiting for the microphone (the first start may wait for the system permission dialog). */
  readonly openingMic = signal(false);

  /** Settings are saved only after they were loaded (no overwrite with defaults). */
  private readonly settingsLoaded = signal(false);

  readonly listening = computed(() => this.startedAt() !== null);
  readonly model = computed(() => this.models().find((m) => m.id === this.modelId()));
  readonly text = computed(() =>
    this.segments()
      .map((s) => s.text.trim())
      .filter(Boolean)
      .join(' '),
  );

  constructor() {
    const subs = [
      this.backend.status$.subscribe((s) => this.status.set(s)),
      this.backend.level$.subscribe((l) => this.pushLevel(l)),
      this.backend.segments$.subscribe((seg) => this.addSegment(seg)),
      this.backend.errors$.subscribe((e) => this.fail(e)),
    ];
    inject(DestroyRef).onDestroy(() => subs.forEach((s) => s.unsubscribe()));
    effect(() => {
      const settings = {
        modelId: this.modelId(),
        language: this.language(),
        deviceId: this.deviceId(),
      };
      if (this.settingsLoaded()) void this.backend.saveSettings(settings).catch(() => undefined);
    });
  }

  async init(): Promise<void> {
    try {
      await this.refreshModels();
      await this.refreshDevices();
      await this.applySettings();
    } catch (e) {
      this.fail(e);
    } finally {
      this.settingsLoaded.set(true);
    }
  }

  /** Desktop: re-reads the input devices (plugged in / removed). */
  async refreshDevices(): Promise<void> {
    if (this.platform !== 'desktop') return;
    try {
      this.devices.set(await this.backend.listInputDevices());
    } catch (e) {
      this.fail(e);
    }
  }

  /** Applies saved choices that are still valid (model offered, language known, device present). */
  private async applySettings(): Promise<void> {
    const s = await this.backend.loadSettings().catch(() => ({}) as Settings);
    if (s.modelId && this.models().some((m) => m.id === s.modelId)) this.modelId.set(s.modelId);
    if (s.language && LANGUAGES.some((l) => l.code === s.language)) this.language.set(s.language);
    if (s.deviceId && this.devices().some((d) => d.id === s.deviceId)) {
      await this.selectDevice(s.deviceId);
    }
  }

  /** Saves the transcript as .txt or .srt; `false` when cancelled or nothing to save. */
  async export(contents: string, fileName: string): Promise<boolean> {
    if (!contents) return false;
    try {
      return await this.backend.saveText(fileName, contents);
    } catch (e) {
      this.fail(e);
      return false;
    }
  }

  async start(): Promise<void> {
    if (this.listening() || this.busy()) return;
    this.error.set(null);
    this.busy.set(true);
    try {
      this.status.set('loading');
      this.progress.set(0);
      await this.backend.loadModel(this.modelId(), (p) => this.progress.set(p));
      this.progress.set(null);
      void this.refreshModels();
      this.levels.set([]);
      this.preliminary.set('');
      this.openingMic.set(true);
      await this.backend.start({ language: this.language() });
      this.startedAt.set(Date.now());
      this.status.set('listening');
    } catch (e) {
      this.fail(e);
    } finally {
      this.openingMic.set(false);
      this.busy.set(false);
    }
  }

  async stop(): Promise<void> {
    const started = this.startedAt();
    if (started === null || this.busy()) return;
    this.busy.set(true);
    this.lastDuration.set(Date.now() - started);
    this.startedAt.set(null);
    try {
      this.status.set('transcribing');
      await this.backend.stop();
      this.status.set('idle');
    } catch (e) {
      this.fail(e);
    } finally {
      this.preliminary.set('');
      this.level.set(0);
      this.busy.set(false);
    }
  }

  async selectDevice(id: string | null): Promise<void> {
    this.deviceId.set(id);
    try {
      await this.backend.setInputDevice(id);
    } catch (e) {
      this.fail(e);
    }
  }

  clear(): void {
    this.segments.set([]);
    this.preliminary.set('');
    this.lastDuration.set(0);
    this.levels.set([]);
  }

  private async refreshModels(): Promise<void> {
    this.models.set(await this.backend.listModels());
  }

  private pushLevel(l: number): void {
    this.level.set(l);
    if (this.startedAt() === null) return;
    this.levels.update((list) => {
      const next = list.length >= HISTORY ? list.slice(list.length - HISTORY + 1) : list.slice();
      next.push(l);
      return next;
    });
  }

  private addSegment(seg: TranscriptSegment): void {
    if (seg.isFinal) {
      this.segments.update((list) => [...list, seg]);
      this.preliminary.set('');
    } else {
      this.preliminary.set(seg.text);
    }
  }

  private fail(e: unknown): void {
    this.progress.set(null);
    this.startedAt.set(null);
    this.status.set('error');
    this.error.set(toSttError(e));
  }
}
