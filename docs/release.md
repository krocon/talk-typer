# Release und Distribution

> Struktur übernommen aus comic-commander-tauri / files-and-folders-tauri. **Tags, Veröffentlichung und
> Signierung nur mit Marcs ausdrücklicher Freigabe.** Lizenz: MIT.

## Workflows (`.github/workflows/`)

| Workflow      | Auslöser                                         | Ergebnis                                                                                                      |
| ------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| `ci.yml`      | Push auf `main`, Pull Requests, montags, manuell | Frontend + WASM (Format, Lint, Tests, beide Builds) auf Linux; Rust fmt/clippy/test auf Linux, macOS, Windows |
| `release.yml` | Tag `v*` oder manuell                            | Pakete für macOS (universal), Windows, Linux als **Entwurf** eines GitHub-Releases                            |
| `pages.yml`   | Push auf `main`, manuell                         | Browser-Version auf GitHub Pages: `https://krocon.github.io/talk-typer/`                                      |

Tests, die ein heruntergeladenes Whisper-Modell brauchen, sind `#[ignore]` und laufen in der CI nicht
(lokal: `cargo test --release -p stt-core -- --ignored`, Modelle siehe `docs/models.md`).

## Ablauf eines Releases

1. `main` ist grün (CI).
2. Version in `package.json` setzen (Quelle; `tauri.conf.json` liest sie von dort), gleiche Version in
   `Cargo.toml` (`[workspace.package]`), Commit `chore: release X.Y.Z`, pushen.
3. `git tag vX.Y.Z && git push origin vX.Y.Z` – oder GitHub → Actions → **Release** → Run workflow.
4. Der Workflow prüft Tag ↔ `package.json`, baut auf drei Runnern und legt einen **Entwurf** an
   (Versionen `0.x` als Pre-release). Pakete installieren, ausprobieren, dann veröffentlichen.

## Pakete

| OS      | Pakete                                            | Hinweise                                                                                  |
| ------- | ------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| macOS   | `.app`, `.dmg` (universal: Apple Silicon + Intel) | ab macOS 12, Hardened Runtime, Entitlement `com.apple.security.device.audio-input`, Metal |
| Windows | `.msi`, `-setup.exe` (NSIS)                       | WebView2 wird bei Bedarf nachgeladen; CPU (optional `--features cuda`)                    |
| Linux   | `.deb`, `.rpm`, `.AppImage`                       | gebaut auf `ubuntu-22.04`; Audio über ALSA (PipeWire/PulseAudio über deren ALSA-Plugin)   |

## Signierung (optional, über Repository-Secrets)

Ohne Secrets entstehen unsignierte Pakete (macOS: Gatekeeper-Warnung, Windows: SmartScreen).

**macOS – Developer ID + Notarisierung** (wie Files and Folders):

| Secret                                                       | Inhalt                                                               |
| ------------------------------------------------------------ | -------------------------------------------------------------------- |
| `APPLE_CERTIFICATE`                                          | Developer-ID-Application-Zertifikat als `.p12`, base64               |
| `APPLE_CERTIFICATE_PASSWORD`                                 | Passwort der `.p12`                                                  |
| `APPLE_SIGNING_IDENTITY`                                     | z. B. `Developer ID Application: Marc Kronberg (TEAMID)`             |
| `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_PRIVATE_KEY` | App-Store-Connect-API-Schlüssel für die Notarisierung (empfohlen)    |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`                | alternativ: Notarisierung mit Apple-ID und App-spezifischem Passwort |

**Windows:** `WINDOWS_CERTIFICATE` (`.pfx`, base64) und `WINDOWS_CERTIFICATE_PASSWORD`.

## GitHub Pages

Einmalig: Repository → Settings → Pages → Source **GitHub Actions**. Die Seite ist rein statisch; Modelle
kommen von HuggingFace und liegen danach im Browser-Cache. Mikrofonzugriff funktioniert nur über HTTPS
(GitHub Pages) oder `localhost`.
