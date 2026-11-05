# Talk Typer

Lokale Spracherkennung mit [OpenAI Whisper](https://github.com/openai/whisper): Mikrofon an, sprechen,
Text erscheint. **Vollständig lokal** – kein Audio und kein Text verlässt das Gerät. Der einzige
Netzwerkzugriff ist der einmalige Download des Modells (von HuggingFace, per SHA-256 geprüft).

Eine Codebasis, vier Ziele:

| Ziel    | Whisper läuft …                          | Aufnahme                              |
| ------- | ---------------------------------------- | ------------------------------------- |
| macOS   | nativ in Rust, GPU über **Metal**        | `cpal` (CoreAudio)                    |
| Windows | nativ in Rust, CPU (optional CUDA)       | `cpal` (WASAPI)                       |
| Linux   | nativ in Rust, CPU (optional CUDA)       | `cpal` (ALSA / PipeWire / PulseAudio) |
| Browser | als **WASM** im Web Worker, CPU mit SIMD | `getUserMedia` + AudioWorklet         |

Stack: Tauri 2 · Rust · [candle](https://github.com/huggingface/candle) · Angular 22 (Standalone, Signals, zoneless).

## Funktionen

- Großer Aufnahme-Button mit Pegel und Wellenform, laufendes Transkript mit Zeitmarken
- Modell, Sprache (Deutsch voreingestellt, Auto-Erkennung möglich) und Eingabegerät (Desktop) wählbar
- Export als `.txt` und `.srt`, Kopieren, Leeren; Einstellungen werden gespeichert
- Hell/Dunkel nach Systemeinstellung; breites Fenster: Transkript als Blatt rechts, sonst im Display

## Aufbau

```
Cargo.toml              Rust-Workspace
crates/stt-core/        plattformneutral: Resampling, VAD, Abschnitte, Log-Mel, Whisper (candle),
                        Tokenizer, Modellkatalog – ohne Dateisystem/Netzwerk/Threads (läuft auch als WASM)
crates/stt-wasm/        wasm-bindgen-Hülle um stt-core für den Browser
src-tauri/              Tauri-App: Commands, cpal-Aufnahme, Modell-Download und -Cache, Export, Einstellungen
frontend/               Angular-App für Desktop und Browser (gemeinsame UI)
docs/                   models.md (Modelle, Messwerte), release.md (CI, Pakete, Signierung)
```

Die Plattform-Weiche ist ein `InjectionToken<TranscriptionBackend>`; die Build-Konfigurationen `desktop`
und `web` sorgen dafür, dass der Web-Build keine Tauri-API und der Desktop-Build kein WASM enthält.

## Voraussetzungen

- Node.js ≥ 22.22 und pnpm 10 (`corepack enable`)
- Rust (stable, ≥ 1.93) – `rustup target add wasm32-unknown-unknown` für den Browser-Build
- `wasm-pack` für den Browser-Build: `cargo install wasm-pack`
- **macOS:** Xcode Command Line Tools
- **Windows:** Microsoft C++ Build Tools, WebView2 (in Windows 10/11 enthalten)
- **Linux** (Debian/Ubuntu):
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev libasound2-dev
  ```
  Audio läuft über ALSA; PipeWire und PulseAudio werden über ihr ALSA-Plugin angesprochen.

Danach einmal `pnpm install`.

## Starten und bauen

| Ziel    | Entwickeln                               | Bauen                                                                           |
| ------- | ---------------------------------------- | ------------------------------------------------------------------------------- |
| macOS   | `pnpm tauri:dev`                         | `pnpm tauri:build:mac` (universal)                                              |
| Windows | `pnpm tauri:dev`                         | `pnpm tauri:build:windows`                                                      |
| Linux   | `pnpm tauri:dev`                         | `pnpm tauri:build:linux`                                                        |
| Browser | `pnpm start:web` → http://localhost:1440 | `pnpm build:web` → `dist/talk-typer-web/browser` (statisch, z. B. GitHub Pages) |

Promo-Seite (`site/`): `pnpm start:site` → http://localhost:1450, `pnpm build:site` → `dist/talk-typer-site`.
Für krocon.de baut `pnpm build:deploy` Seite und Web-App nach `dist/deploy/talk-typer/` (Seite) und
`dist/deploy/talk-typer/app/` (Web-App, Base-Href `/products/talk-typer/app/`); den Ordner
`dist/deploy/talk-typer` nach `/products/` auf den Server kopieren.

NVIDIA-GPU unter Windows/Linux (CUDA-Toolkit nötig): `pnpm tauri build -- --features cuda`.

Prüfungen: `pnpm check` (Format, Lint, Frontend-Tests, beide Builds, `cargo fmt`, `clippy` nativ und
wasm32, Rust-Tests). Transkription einer WAV-Datei ohne App:

```bash
cargo run --release -p stt-core --features metal --example transcribe -- aufnahme.wav --model large-v3-turbo-q --fetch
```

## Modelle und Geschwindigkeit

Nur mehrsprachige Whisper-Modelle. Größen sind Downloads (einmalig, danach im Cache):

| Modell              | Download     | Desktop      | Browser                    |
| ------------------- | ------------ | ------------ | -------------------------- |
| `tiny-q`            | 44 MB        | ✓            | **Standard**               |
| `base-q`            | 83 MB        | ✓            | ✓ (langsamer als Echtzeit) |
| `tiny` / `base`     | 154 / 293 MB | ✓            | ✓                          |
| `small` / `small-q` | 969 / 266 MB | ✓            | –                          |
| `large-v3-turbo-q`  | 880 MB       | **Standard** | –                          |
| `large-v3-turbo`    | 1,6 GB       | ✓            | –                          |

Gemessen auf einem Apple-Silicon-Mac mit 38,5 s deutscher Sprache (Details: [docs/models.md](docs/models.md)):

- **Nativ mit Metal/GPU nahezu Echtzeit und schneller:** `large-v3-turbo-q` braucht 4,0 s und macht
  in der Testaufnahme keinen Fehler; `base` 2,3 s.
- **Nativ auf der CPU:** `large-v3-turbo-q` 25 s, `base` 10 s, `tiny-q` 5 s.
- **Im Browser (WASM, CPU, ohne Threads)** spürbar langsamer: `tiny-q` 30 s, `base-q` 66 s. Deshalb arbeitet
  der Browser abschnittsweise (≤ 12 s, nach Sprechpausen) statt Wort für Wort; der Text erscheint einige
  Sekunden nach dem Sprechen. Für beste Erkennung ist die Desktop-App gedacht.

## Datenschutz

Audio wird nur im Arbeitsspeicher verarbeitet und nicht gespeichert. Text verlässt das Gerät nur, wenn
du ihn selbst kopierst oder exportierst. Die App lädt ausschließlich Modelldateien von
`huggingface.co` (feste Versionen, SHA-256-geprüft).

Geladene Modelle bleiben dauerhaft gespeichert und werden von der App nie gelöscht, auch nicht bei
der Deinstallation. Wer den Speicherplatz zurückhaben will, muss sie manuell entfernen. Speicherorte und
Anleitung: [docs/models.md → Speicherort und Entfernen](docs/models.md#speicherort-und-entfernen).

## Lizenz

MIT – siehe [LICENSE](LICENSE). Whisper-Modelle: MIT (OpenAI).
