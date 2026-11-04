import { DecimalPipe } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  ElementRef,
  OnInit,
  computed,
  effect,
  inject,
  signal,
  viewChildren,
} from '@angular/core';
import { exportFileName, toSrt, toTxt } from './export/transcript-export';
import { formatClock, formatDuration } from './recorder/clock';
import { Waveform } from './recorder/waveform';
import { ModelId } from './stt/stt.models';
import { SttStore } from './stt/stt.store';
import { LANGUAGES, TEXTS } from './texts';

/** Same breakpoint as `.stage` in app.css. */
const WIDE_QUERY = '(min-width: 900px)';

@Component({
  selector: 'app-root',
  imports: [DecimalPipe, Waveform],
  templateUrl: './app.html',
  styleUrl: './app.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class App implements OnInit {
  protected readonly store = inject(SttStore);
  protected readonly t = TEXTS;
  protected readonly languages = LANGUAGES;

  protected readonly settingsOpen = signal(false);
  /** Wide window: transcript on a sheet to the right; otherwise inside the display. */
  protected readonly wide = signal(false);
  protected readonly copied = signal(false);
  /** Format that was just saved (button feedback). */
  protected readonly saved = signal<'txt' | 'srt' | null>(null);
  private readonly now = signal(Date.now());

  protected readonly clock = computed(() => formatClock(new Date(this.now())));
  protected readonly timer = computed(() => {
    const started = this.store.startedAt();
    return formatDuration(started === null ? this.store.lastDuration() : this.now() - started);
  });
  protected readonly errorText = computed(() => {
    const e = this.store.error();
    return e ? this.t.errors[e.code] : '';
  });
  protected readonly languageCode = computed(() => this.store.language().toUpperCase());
  protected readonly wordCount = computed(() => {
    const text = this.store.text();
    return text ? text.split(/\s+/).filter(Boolean).length : 0;
  });
  protected readonly formatStamp = (ms: number): string => formatDuration(ms).slice(3);

  private readonly scrollers = viewChildren<ElementRef<HTMLElement>>('scroller');

  constructor() {
    // keep the newest text in view
    effect(() => {
      this.store.segments();
      this.store.preliminary();
      const els = this.scrollers();
      queueMicrotask(() =>
        els.forEach((e) => (e.nativeElement.scrollTop = e.nativeElement.scrollHeight)),
      );
    });
    const timer = setInterval(() => this.now.set(Date.now()), 250);
    const query = globalThis.matchMedia?.(WIDE_QUERY);
    const onChange = (): void => this.wide.set(query?.matches ?? false);
    onChange();
    query?.addEventListener('change', onChange);
    inject(DestroyRef).onDestroy(() => {
      clearInterval(timer);
      query?.removeEventListener('change', onChange);
    });
  }

  ngOnInit(): void {
    void this.store.init();
  }

  /** Desktop: "Base ✓" when cached, else "Base · ↓ 293 MB"; browser: "Base · 293 MB". */
  protected modelOptionLabel(m: { label: string; sizeMb: number; downloaded?: boolean }): string {
    if (m.downloaded === undefined) return `${m.label} · ${m.sizeMb} MB`;
    return m.downloaded ? `${m.label} ✓` : `${m.label} · ↓ ${m.sizeMb} MB`;
  }

  protected onModel(ev: Event): void {
    this.store.modelId.set((ev.target as HTMLSelectElement).value as ModelId);
  }

  protected onLanguage(ev: Event): void {
    this.store.language.set((ev.target as HTMLSelectElement).value);
  }

  protected onDevice(ev: Event): void {
    const v = (ev.target as HTMLSelectElement).value;
    void this.store.selectDevice(v === '' ? null : v);
  }

  protected toggleSettings(): void {
    const open = !this.settingsOpen();
    this.settingsOpen.set(open);
    if (open) void this.store.refreshDevices();
  }

  protected async save(format: 'txt' | 'srt'): Promise<void> {
    const segments = this.store.segments();
    const contents = format === 'txt' ? toTxt(segments) : toSrt(segments);
    if (await this.store.export(contents, exportFileName(format))) {
      this.saved.set(format);
      setTimeout(() => this.saved.set(null), 1500);
    }
  }

  protected record(): void {
    this.settingsOpen.set(false);
    void this.store.start();
  }

  protected async copy(): Promise<void> {
    await navigator.clipboard.writeText(this.store.text());
    this.copied.set(true);
    setTimeout(() => this.copied.set(false), 1500);
  }
}
