import { TestBed } from '@angular/core/testing';
import { EMPTY } from 'rxjs';
import { describe, expect, it, vi } from 'vitest';
import { TRANSCRIPTION_BACKEND, TranscriptionBackend } from './stt-backend';
import { UpdateInfo } from './stt.models';
import { SttStore } from './stt.store';

function setup(extra: Partial<TranscriptionBackend>): SttStore {
  const backend = {
    platform: 'desktop',
    segments$: EMPTY,
    status$: EMPTY,
    level$: EMPTY,
    errors$: EMPTY,
    saveSettings: () => Promise.resolve(),
    ...extra,
  } as unknown as TranscriptionBackend;
  TestBed.configureTestingModule({
    providers: [{ provide: TRANSCRIPTION_BACKEND, useValue: backend }],
  });
  return TestBed.inject(SttStore);
}

const newer: UpdateInfo = {
  current: '0.1.0',
  latest: '0.2.0',
  url: 'https://github.com/krocon/talk-typer/releases/tag/v0.2.0',
  available: true,
};

describe('SttStore update check', () => {
  it('is not offered without backend support (browser)', async () => {
    const store = setup({});
    expect(store.canCheckUpdate).toBe(false);
    await store.checkUpdate();
    expect(store.update()).toEqual({ kind: 'idle' });
  });

  it('stores the result and opens the release page', async () => {
    const openReleasePage = vi.fn(() => Promise.resolve());
    const store = setup({ checkUpdate: () => Promise.resolve(newer), openReleasePage });
    await store.checkUpdate();
    expect(store.update()).toEqual({ kind: 'done', info: newer });
    await store.openRelease();
    expect(openReleasePage).toHaveBeenCalledWith(newer.url);
  });

  it('keeps a failure local (no app error)', async () => {
    const store = setup({ checkUpdate: () => Promise.reject({ code: 'update_check_failed' }) });
    await store.checkUpdate();
    expect(store.update()).toEqual({ kind: 'failed' });
    expect(store.error()).toBeNull();
    expect(store.status()).toBe('idle');
  });
});
