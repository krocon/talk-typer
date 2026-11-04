import { ApplicationConfig, provideBrowserGlobalErrorListeners } from '@angular/core';

/** Zoneless (Angular default). The STT backend is bound in `main.ts`. */
export const appConfig: ApplicationConfig = {
  providers: [provideBrowserGlobalErrorListeners()],
};
