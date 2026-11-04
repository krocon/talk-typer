# Prompt: Lokale Spracherkennung mit Whisper – Tauri 2 (Desktop) + WASM (Browser)

Du bist ein erfahrener Rust-, Tauri-2- und Angular-Entwickler. Baue mit mir eine App, die Mikrofonton aufnimmt und die Sprache **vollständig lokal** mit OpenAI Whisper in Text umwandelt. Es gibt keine Cloud-API und keinen Server für die Erkennung.

Die App läuft aus **einer Codebasis** auf vier Zielen:

- **macOS, Windows, Linux** als Tauri-2-Desktop-App (Whisper läuft nativ in Rust)
- **Browser** als reine Web-App (Whisper läuft als Rust→WASM in einem Web Worker)

Der Stack entspricht meinen anderen Projekten (Comic Commander, AppIconic): **Tauri 2 + Rust + Angular (aktuelle Version, Standalone Components, Signals)**.

---

## github

github: https://github.com/krocon/talk-typer

lokal: '/Users/marckronberg/WebstormProjects/talk-typer'


## Platzhalter (vor Start anpassen)

| Feld | Wert |
|---|---|
| App-Name | `Talk Typer` |
| Bundle-ID | `io.github.krocon.talk-typer` |
| Repo | `github.com/krocon/talk-typer` |
| Lizenz | MIT |
| Standardsprache der Erkennung | Deutsch (`de`), umschaltbar + Auto-Erkennung |

---

## Architektur

nimm dir ein Beispiel an:

* /Users/marckronberg/WebstormProjects/files-and-folders-tauri
* /Users/marckronberg/WebstormProjects/comic-commander-tauri
* /Users/marckronberg/WebstormProjects/sonic-lab

```
<repo>/
├── Cargo.toml                 # Rust-Workspace
├── crates/
│   ├── stt-core/              # Plattformneutral: Resampling, Mel-Spektrogramm,
│   │                          # Whisper-Modell (candle), Decoding, Chunking/VAD.
│   │                          # Lädt Modelle aus Bytes, kein Dateisystem, kein Netzwerk.
│   └── stt-wasm/              # wasm-bindgen-Wrapper um stt-core für den Browser
├── src-tauri/                 # Tauri-2-App: Commands, native Audioaufnahme (cpal),
│                              # Modell-Download & -Cache, Events ans Frontend
└── frontend/                  # Angular-App, gemeinsam für Desktop und Browser
    └── src/app/stt/
        ├── stt-backend.ts         # Interface TranscriptionBackend
        ├── tauri-backend.ts       # Implementierung über invoke()/listen()
        ├── wasm-backend.ts        # Implementierung über Web Worker + stt-wasm
        ├── audio-capture.worklet.ts  # AudioWorklet (nur Browser)
        └── stt.worker.ts          # Web Worker, lädt stt-wasm
```

### Kernentscheidungen

1. **Eine Whisper-Implementierung für alle Ziele:** HuggingFace **`candle`** (`candle-core`, `candle-nn`, `candle-transformers`), weil es nativ *und* nach `wasm32-unknown-unknown` kompiliert. Orientiere dich am offiziellen candle-Whisper-Beispiel und am candle-WASM-Whisper-Beispiel.
    - Native Feature-Flags: `metal` auf macOS, optional `cuda` auf Windows/Linux, sonst CPU (mit `accelerate`/`mkl` falls sinnvoll).
    - WASM: CPU mit `+simd128`.
2. **Modelle:** mehrsprachige Whisper-Modelle (keine `.en`-Varianten!). Unterstütze `tiny`, `base`, `small`, jeweils auch quantisiert (GGUF via `candle-transformers::models::whisper::quantized_model`). Standard im Browser: `tiny` bzw. `base` quantisiert. Standard auf dem Desktop: `base`, wählbar bis `small`.
3. **Audioaufnahme:**
    - **Desktop:** nativ in Rust mit **`cpal`**, nicht über `getUserMedia` im WebView. Grund: Die Mediensupport-Lage in WebKitGTK (Linux) und die Berechtigungen in WKWebView (macOS) sind unzuverlässig.
    - **Browser:** `getUserMedia` + `AudioWorklet`, Samples werden per `postMessage` (transferable `Float32Array`) an den Worker gereicht.
4. **Plattform-Weiche im Frontend:** Ein Angular-`InjectionToken<TranscriptionBackend>`. Zur Laufzeit wird anhand von `isTauri()` (aus `@tauri-apps/api/core`) gewählt. Zusätzlich gibt es zwei Build-Konfigurationen (`desktop`, `web`), damit der Web-Build keine Tauri-Abhängigkeiten und der Desktop-Build kein WASM-Bundle mitschleppt (Lazy Import).
5. **Threading:** Die Erkennung läuft **nie** auf dem UI-Thread. Auf dem Desktop läuft sie in einem eigenen Rust-Thread bzw. per `tokio::task::spawn_blocking`, im Browser im Web Worker.

### Interface (Frontend)

```ts
export interface TranscriptionBackend {
  listModels(): Promise<ModelInfo[]>;
  loadModel(id: ModelId, onProgress: (p: number) => void): Promise<void>;
  start(opts: { language: 'auto' | string }): Promise<void>;
  stop(): Promise<void>;
  segments$: Observable<TranscriptSegment>; // { text, startMs, endMs, isFinal }
  status$: Observable<SttStatus>;           // idle | loading | listening | transcribing | error
}
```

---

## Audio- und Erkennungs-Pipeline (in `stt-core`)

1. Eingabe: beliebige Sample-Rate, beliebige Kanäle, `f32`
2. Downmix auf Mono und Resampling auf **16 kHz** (z. B. `rubato`, WASM-kompatibel)
3. Einfache **energiebasierte VAD** (RMS-Schwelle + Hangover), um Sprachabschnitte zu erkennen
4. Chunking: Sprachabschnitte bis max. 30 s, Pausen schließen einen Chunk ab. Lange Rede wird mit ~1–2 s Überlappung geteilt.
5. Log-Mel-Spektrogramm (80 bzw. 128 Bins, je nach Modell) → Encoder → Greedy-Decoding mit Timestamps
6. Ausgabe: Segmente mit Text und Zeitstempeln. Optional: vorläufige Teilergebnisse (`isFinal: false`) während eines laufenden Chunks.

`stt-core` darf **keine** Abhängigkeiten auf `std::fs`, Netzwerk oder Threads haben, die WASM brechen. Modelle, Tokenizer (`tokenizer.json`) und Config werden als `&[u8]` übergeben.

---

## Desktop (Tauri 2)

- **Commands:** `list_models`, `download_model` (mit Fortschritts-Events), `load_model`, `start_listening`, `stop_listening`, `list_input_devices`, `set_input_device`
- **Events:** `stt://segment`, `stt://status`, `stt://download-progress`
- **Modell-Cache:** im App-Datenverzeichnis (`app.path().app_data_dir()`), Download per `reqwest` von HuggingFace mit SHA-256-Prüfung. Keine Modelle ins Bundle packen.
- **macOS:**
    - `NSMicrophoneUsageDescription` in `src-tauri/Info.plist`
    - Entitlement `com.apple.security.device.audio-input` (nötig für Hardened Runtime und Notarisierung, Distribution per Developer ID wie bei Files and Folders)
    - `metal`-Feature aktiv
- **Windows:** saubere Fehlermeldung, wenn der Mikrofonzugriff in den Datenschutzeinstellungen gesperrt ist
- **Linux:** Abhängigkeiten dokumentieren (`libasound2-dev`, PipeWire/PulseAudio über ALSA). Bundles als AppImage, `.deb` und `.rpm`.
- Capabilities/Permissions in `src-tauri/capabilities/` minimal halten

## Browser (WASM)

- `stt-wasm` mit `wasm-bindgen`, gebaut über `wasm-pack build --target web` mit `RUSTFLAGS="-C target-feature=+simd128"`
- Der Worker lädt das WASM-Modul und hält das Modell im Speicher
- **Modell-Cache:** Cache Storage API oder OPFS, damit das Modell nur einmal geladen wird. Zeige Größe und Fortschritt beim ersten Download an.
- Hinweis in der UI: Mikrofonzugriff nur über HTTPS oder `localhost`
- Läuft in aktuellen Chrome-, Edge-, Firefox- und Safari-Versionen. Feature-Detection für `AudioWorklet` und WASM-SIMD mit verständlicher Fehlermeldung.
- Optional später: WASM-Threads (benötigt COOP/COEP-Header für `SharedArrayBuffer`). Für den ersten Wurf **ohne** Threads.
- Deploybar als statische Seite (z. B. GitHub Pages)

---

## UI (Angular, für beide Ziele gleich)

- Großer Aufnahme-Button (Start/Stopp) mit Pegelanzeige
- Laufendes Transkript; vorläufiger Text ist optisch abgesetzt
- Auswahl: Modell, Sprache (Auto/Deutsch/Englisch/…), Eingabegerät (nur Desktop)
- Aktionen: Kopieren, als `.txt` speichern, als `.srt` speichern, Leeren
- Status- und Fehlermeldungen (Modell lädt, kein Mikrofon, Zugriff verweigert)
- Hell/Dunkel nach Systemeinstellung

---

## Vorgehen in Phasen

Arbeite die Phasen nacheinander ab. Am Ende jeder Phase muss etwas **lauffähig** sein. Zeige mir kurz, was erreicht ist, bevor du weitermachst.

1. **Gerüst:** Rust-Workspace, Tauri-2-App, Angular-Frontend, beide Build-Konfigurationen. `cargo check` für native und `wasm32-unknown-unknown` ist grün.
2. **stt-core offline:** Transkription einer WAV-Datei per CLI-Testbinary (`cargo run --example transcribe -- sample.wav`). Ein Integrationstest mit kurzer deutscher Beispieldatei.
3. **Desktop live:** `cpal`-Aufnahme → VAD/Chunking → Transkript in der UI. Modell-Download mit Fortschritt.
4. **Browser live:** AudioWorklet → Worker → `stt-wasm` → Transkript in derselben UI. Modell-Cache im Browser.
5. **Feinschliff:** Export (`.txt`, `.srt`), Geräteauswahl, Fehlerfälle, Einstellungen persistieren (Desktop: Tauri-Store-Plugin, Browser: `localStorage` mit try/catch)
6. **Distribution:** GitHub-Actions-Workflow mit `tauri-action` für macOS (universal, signiert + notarisiert), Windows und Linux. Separater Workflow für den Web-Build auf GitHub Pages.

---

## Qualitätsanforderungen

- Rust: `cargo clippy -- -D warnings` und `cargo fmt` sauber, keine `unwrap()` in Produktivcode, Fehler über `thiserror`
- TypeScript: `strict: true`, kein `any`
- Kein Audio und kein Text verlässt das Gerät. Der einzige Netzwerkzugriff ist der Modell-Download.
- README mit Build-Anleitung für alle vier Ziele, Systemvoraussetzungen und Hinweisen zu Modellgrößen und Geschwindigkeit
- Erwartete Performance dokumentieren: nativ mit Metal/GPU nahezu Echtzeit, im Browser (CPU/WASM) mit `tiny`/`base` spürbar langsamer, deshalb chunkweise statt Wort für Wort


## screens

Nimm die Screens nur als grobe Vorlage:

prompts/01-screen.png
prompts/02-screen.png
prompts/03-screen.png
prompts/04-screen.png
prompts/05-screen.png
prompts/06-screen.png
prompts/07-screen.png



### Frag nach, bevor du von diesen Entscheidungen abweichst, besonders bei der Wahl der Whisper-Implementierung, der Audioaufnahme oder bei neuen schweren Abhängigkeiten.