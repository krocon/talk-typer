import { DestroyRef, Injectable, computed, inject, signal } from '@angular/core';
import { TRANSCRIPTION_BACKEND } from './stt-backend';
import {
  InputDevice,
  ModelId,
  ModelInfo,
  SttError,
  SttStatus,
  TranscriptSegment,
  toSttError,
} from './stt.models';

/** UI state of the transcriber (one per app). Talks to the backend only through the interface. */
@Injectable({ providedIn: 'root' })
export class SttStore {
  private readonly backend = inject(TRANSCRIPTION_BACKEND);

  readonly platform = this.backend.platform;
  readonly status = signal<SttStatus>('idle');
  readonly models = signal<ModelInfo[]>([]);
  readonly modelId = signal<ModelId>(this.platform === 'desktop' ? 'base' : 'tiny-q');
  readonly language = signal<string>('de');
  readonly devices = signal<InputDevice[]>([]);
  readonly deviceId = signal<string | null>(null);
  readonly segments = signal<TranscriptSegment[]>([]);
  readonly preliminary = signal<string>('');
  readonly level = signal(0);
  readonly progress = signal<number | null>(null);
  readonly error = signal<SttError | null>(null);

  readonly listening = computed(() => {
    const s = this.status();
    return s === 'listening' || s === 'transcribing';
  });
  readonly text = computed(() =>
    this.segments()
      .map((s) => s.text.trim())
      .filter(Boolean)
      .join(' '),
  );

  constructor() {
    const subs = [
      this.backend.status$.subscribe((s) => this.status.set(s)),
      this.backend.level$.subscribe((l) => this.level.set(l)),
      this.backend.segments$.subscribe((seg) => this.addSegment(seg)),
    ];
    inject(DestroyRef).onDestroy(() => subs.forEach((s) => s.unsubscribe()));
  }

  async init(): Promise<void> {
    try {
      this.models.set(await this.backend.listModels());
      if (this.platform === 'desktop') this.devices.set(await this.backend.listInputDevices());
    } catch (e) {
      this.fail(e);
    }
  }

  async toggle(): Promise<void> {
    this.error.set(null);
    try {
      if (this.listening()) {
        await this.backend.stop();
        this.status.set('idle');
        return;
      }
      this.status.set('loading');
      this.progress.set(0);
      await this.backend.loadModel(this.modelId(), (p) => this.progress.set(p));
      this.progress.set(null);
      await this.backend.start({ language: this.language() });
      this.status.set('listening');
    } catch (e) {
      this.fail(e);
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
    this.status.set('error');
    this.error.set(toSttError(e));
  }
}
