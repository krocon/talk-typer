import { DecimalPipe } from '@angular/common';
import { ChangeDetectionStrategy, Component, computed, inject, OnInit } from '@angular/core';
import { SttStore } from './stt/stt.store';
import { ModelId } from './stt/stt.models';
import { LANGUAGES, TEXTS } from './texts';

@Component({
  selector: 'app-root',
  imports: [DecimalPipe],
  templateUrl: './app.html',
  styleUrl: './app.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class App implements OnInit {
  protected readonly store = inject(SttStore);
  protected readonly t = TEXTS;
  protected readonly languages = LANGUAGES;

  protected readonly errorText = computed(() => {
    const e = this.store.error();
    return e ? this.t.errors[e.code] : '';
  });
  protected readonly levelScale = computed(() => 1 + Math.min(this.store.level() * 4, 0.35));

  ngOnInit(): void {
    void this.store.init();
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

  protected async copy(): Promise<void> {
    await navigator.clipboard.writeText(this.store.text());
  }
}
