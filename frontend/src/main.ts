import { bootstrapApplication } from '@angular/platform-browser';
import { App } from './app/app';
import { appConfig } from './app/app.config';
import { TRANSCRIPTION_BACKEND } from './app/stt/stt-backend';
import { loadBackend } from './app/stt/stt-platform';

// The backend module is imported lazily (desktop: Tauri, web: WASM) and bound before bootstrap,
// so components can inject `TRANSCRIPTION_BACKEND` synchronously.
loadBackend()
  .then((backend) =>
    bootstrapApplication(App, {
      providers: [...appConfig.providers, { provide: TRANSCRIPTION_BACKEND, useValue: backend }],
    }),
  )
  .catch((err: unknown) => console.error(err));
