# Modelle

Talk Typer nutzt ausschließlich **mehrsprachige** Whisper-Modelle (keine `.en`-Varianten). Der Katalog
steht in [`crates/stt-core/src/models.rs`](../crates/stt-core/src/models.rs); jede Datei hat eine feste
URL, eine SHA-256-Prüfsumme und eine Größe. Ein Download, dessen Prüfsumme nicht passt, wird verworfen.

| Id | Gewichte | Quelle der Gewichte | Download gesamt |
|---|---|---|---|
| `tiny` | F32 safetensors | `openai/whisper-tiny` (Commit gepinnt) | 154 MB |
| `tiny-q` | Q8_0 GGUF | `krocon/talk-typer-models` (selbst quantisiert, Commit gepinnt) | 44 MB |
| `base` | F32 safetensors | `openai/whisper-base` (Commit gepinnt) | 293 MB |
| `base-q` | Q8_0 GGUF | `krocon/talk-typer-models` (selbst quantisiert, Commit gepinnt) | 83 MB |
| `small` | F32 safetensors | `openai/whisper-small` (Commit gepinnt) | 969 MB |
| `small-q` | Q8_0 GGUF | `krocon/talk-typer-models` (selbst quantisiert, Commit gepinnt) | 266 MB |
| `large-v3-turbo` | F32 safetensors | `openai/whisper-large-v3-turbo` (Commit gepinnt) | 1621 MB |
| `large-v3-turbo-q` | Q8_0 GGUF | `krocon/talk-typer-models` (selbst quantisiert, Commit gepinnt) | 880 MB |

`config.json` und `tokenizer.json` kommen bei allen Varianten aus dem jeweiligen OpenAI-Repository.

Standard: Desktop `large-v3-turbo-q` (beste Erkennung, mit Metal/GPU schneller als `small`), Browser
`tiny-q` (kleinster Download; im Browser rechnet nur die CPU). `large-v3-turbo` nutzt 128 Mel-Bänder
statt 80 und wird im Browser nicht angeboten (dort zu langsam).

## Speicherort und Entfernen

Jedes Modell wird **einmal** geladen und bleibt danach gespeichert; die App startet dann ohne Netz.
Talk Typer löscht Modelle nie selbst – weder beim Wechsel des Modells noch bei der Deinstallation.
Wer Platz freigeben will (allein `large-v3-turbo-q` belegt rund 880 MB), muss die Dateien **manuell
entfernen**.

### Desktop

Modelle liegen im App-Datenverzeichnis unter `models/<id>/` (`weights.gguf` bzw.
`weights.safetensors`, `config.json`, `tokenizer.json`):

| System  | Ordner                                                          |
| ------- | --------------------------------------------------------------- |
| macOS   | `~/Library/Application Support/io.github.krocon.talk-typer/models/` |
| Windows | `%APPDATA%\io.github.krocon.talk-typer\models\`                 |
| Linux   | `~/.local/share/io.github.krocon.talk-typer/models/`            |

- Einzelnes Modell entfernen: den Unterordner löschen, z. B. `models/large-v3-turbo-q/`. Wählt man das
  Modell später wieder, wird es neu geladen.
- Alles entfernen (nach der Deinstallation): den ganzen Ordner `io.github.krocon.talk-typer` löschen.
  Darin liegen auch die gespeicherten Einstellungen. Unter macOS bleibt er erhalten, wenn die App nur in
  den Papierkorb gezogen wird.

Die App sollte dabei nicht laufen. Downloads werden als `*.part` geschrieben und erst nach geprüfter
Größe und SHA-256 umbenannt; ein abgebrochener Download hinterlässt keine halben Modelldateien. Bereits
fertige Dateien eines Modells bleiben erhalten, die abgebrochene Datei wird beim nächsten Versuch von
vorn geladen.

### Browser

Modelle liegen im Cache Storage der Seite (Cache `talk-typer-models-v1`). Entfernen über die
Website-Daten des Browsers: für `krocon.github.io` „Websitedaten löschen“ (Chrome/Edge:
Schloss-Symbol → Website-Einstellungen → Daten löschen; Firefox: Schloss-Symbol → Cookies und
Website-Daten löschen; Safari: Einstellungen → Datenschutz → Websitedaten verwalten). Achtung: Das
löscht die Daten **aller** Seiten unter `krocon.github.io`, nicht nur die von Talk Typer. Gezielt nur
die Modelle entfernen: Entwicklertools → Application/Speicher → Cache Storage →
`talk-typer-models-v1` löschen. Der Browser kann
den Cache bei Speichermangel auch selbst leeren; im privaten Fenster wird gar nicht gecacht.

## Warum selbst quantisiert?

Das Repository der offiziellen candle-Beispiele (`lmz/candle-whisper`) enthält quantisiert nur `tiny`.
Andere GGUF-Uploads auf HuggingFace stammen von Dritten und nutzen meist das whisper.cpp-Layout, das
candle nicht lädt. Deshalb werden die OpenAI-Gewichte selbst quantisiert – mit derselben Regel wie
candles `tensor-tools quantize` (2-D-Gewichte → Q8_0, Rest F32). Die eigene `tiny`-Datei hat dieselben
167 Tensoren mit denselben Typen und Formen wie die von `lmz`.

Das Werkzeug schreibt die Tensoren sortiert, die Ausgabe ist daher **reproduzierbar**: Wer die
OpenAI-Gewichte quantisiert, erhält bitgenau die Dateien mit den Prüfsummen aus dem Katalog.

## Dateien erzeugen und hochladen

```bash
# erzeugt model-{tiny,base,small}-q80.gguf und prüft sie gegen den Katalog
cargo run --release -p stt-core --example quantize -- --all upload/

# einmalig: Repository anlegen und hochladen (HuggingFace-CLI, angemeldet als krocon)
hf repos create krocon/talk-typer-models --public
hf upload krocon/talk-typer-models upload/ .
```

Hochgeladen am 04.11.2026: tiny/base/small in Commit `648d42f`, large-v3-turbo in Commit
`725f98c8c2c0655e18e3750cb60b5c764d1f5180`; auf diesen Commit (er enthält alle vier Dateien) zeigen die
URLs im Katalog. Neue Dateien → neuer Commit → URLs und Prüfsummen im Katalog anpassen.

Ist das Repository nicht erreichbar, erzeugt das Entwicklerwerkzeug (`--fetch` bei
`examples/transcribe.rs`) quantisierte Modelle lokal aus den OpenAI-Gewichten.

## Messwerte

Apple-Silicon-Mac, macOS, Release-Build, deutsche Aufnahme von 38,5 s (macOS-Stimme „Anna“),
Sprache fest auf Deutsch. Wortfehlerrate (WER) gegen den gesprochenen Text, Zahlen normalisiert.

| Modell | CPU | Metal | WER |
|---|---|---|---|
| `tiny` | 5,9 s | – | 5,4 % |
| `tiny-q` | 5,2 s | – | 5,4 % |
| `base` | 10,2 s | 2,3 s | 6,5 % |
| `base-q` | 11,3 s | 3,1 s | 6,5 % |
| `small` | 34,7 s | 6,0 s | 3,3 % |
| `small-q` | 30,8 s | 5,1 s | 3,3 % |
| `large-v3-turbo` | 30,0 s | 4,9 s | 0 % |
| `large-v3-turbo-q` | 24,9 s | 4,0 s | 0 % |

- Q8_0 kostet hier keine Qualität, ist auf der CPU aber kaum schneller – der Gewinn ist die 3,6-fach
  kleinere Datei (wichtig im Browser).
- Metal ist 4–6× schneller als die CPU. Der allererste Metal-Lauf auf einem Rechner dauert länger
  (Shader werden kompiliert); die App wärmt das Modell deshalb beim Laden vor.
- Synthetische Sprache ist leichter als echte Aufnahmen; die WER-Werte sind nur ein Vergleich der
  Modelle untereinander. Browser-Werte (WASM, CPU, ohne Threads) folgen in Phase 4; erwartet wird
  deutlich langsamer als die native CPU (Schätzung, noch nicht gemessen).

### Browser (WASM, CPU, ohne Threads)

Gleiche Aufnahme im Browser-Worker (Chromium, Apple Silicon):

| Modell | erstes Laden | Durchsatz | Text |
|---|---|---|---|
| `tiny-q` | 5 s (44 MB), danach 0,2 s aus dem Cache | 38,5 s Audio in 29,6 s | wie nativ |
| `base-q` | 13 s (83 MB) | 38,5 s Audio in 65,8 s (langsamer als Echtzeit) | wie nativ |

Live im Echtzeit-Takt mit `tiny-q`: Text erscheint 6–13 s nach dem Sprechen, 6 s nach dem Stopp ist
alles fertig. Der Browser nutzt deshalb kürzere Abschnitte (≤ 12 s, Pause 0,3 s) und keine
vorläufigen Ergebnisse (`stt.worker.ts`). Für gute Erkennung ist die Desktop-App gedacht.
