import { SttErrorCode, SttStatus } from './stt/stt.models';

/** UI texts (German first; i18n later). */
export const TEXTS = {
  appName: 'Talk Typer',
  start: 'Aufnahme starten',
  stop: 'Aufnahme stoppen',
  model: 'Modell',
  language: 'Sprache',
  device: 'Eingabegerät',
  systemDefault: 'Systemstandard',
  copy: 'Kopieren',
  clear: 'Leeren',
  transcript: 'Transkript',
  emptyTranscript: 'Drücke den Aufnahme-Button und sprich. Der Text erscheint hier.',
  privacy: 'Alles bleibt auf diesem Gerät – nur das Modell wird einmalig geladen.',
  desktop: 'Desktop (nativ)',
  web: 'Browser (WASM)',
  status: {
    idle: 'Bereit',
    loading: 'Modell wird geladen …',
    listening: 'Hört zu …',
    transcribing: 'Erkennt Text …',
    error: 'Fehler',
  } satisfies Record<SttStatus, string>,
  errors: {
    invalid_audio: 'Das Audiosignal konnte nicht verarbeitet werden.',
    unknown_model: 'Unbekanntes Modell.',
    invalid_model: 'Die Modelldateien sind beschädigt.',
    invalid_tokenizer: 'Der Tokenizer des Modells ist beschädigt.',
    no_model: 'Es ist noch kein Modell geladen.',
    resample: 'Das Audiosignal konnte nicht umgerechnet werden.',
    inference: 'Die Spracherkennung ist fehlgeschlagen.',
    audio_device: 'Das Mikrofon konnte nicht geöffnet werden.',
    microphone_denied:
      'Der Mikrofonzugriff ist gesperrt. Bitte in den Datenschutzeinstellungen des Systems erlauben.',
    no_input_device: 'Es wurde kein Mikrofon gefunden.',
    insecure_context: 'Mikrofonzugriff ist nur über HTTPS oder localhost möglich.',
    unsupported_browser:
      'Dieser Browser unterstützt AudioWorklet oder WebAssembly-SIMD nicht. Bitte eine aktuelle Version von Chrome, Edge, Firefox oder Safari verwenden.',
    internal: 'Ein interner Fehler ist aufgetreten.',
    not_implemented: 'Diese Funktion ist noch nicht fertig (Gerüst, Phase 1).',
  } satisfies Record<SttErrorCode, string>,
};

export const LANGUAGES: readonly { code: string; label: string }[] = [
  { code: 'auto', label: 'Automatisch' },
  { code: 'de', label: 'Deutsch' },
  { code: 'en', label: 'Englisch' },
  { code: 'fr', label: 'Französisch' },
  { code: 'es', label: 'Spanisch' },
  { code: 'it', label: 'Italienisch' },
  { code: 'nl', label: 'Niederländisch' },
  { code: 'pl', label: 'Polnisch' },
];
