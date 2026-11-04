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

`config.json` und `tokenizer.json` kommen bei allen Varianten aus dem jeweiligen OpenAI-Repository.

Standard: Desktop `base` (mit Metal/GPU schnell genug), Browser `tiny-q` (kleinster Download, CPU/WASM).

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

Hochgeladen am 04.11.2026, Commit `648d42f7e6c729e764d9969bcd8b19240252e8d1`; auf diesen Commit zeigen die
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

- Q8_0 kostet hier keine Qualität, ist auf der CPU aber kaum schneller – der Gewinn ist die 3,6-fach
  kleinere Datei (wichtig im Browser).
- Metal ist 4–6× schneller als die CPU. Der allererste Metal-Lauf auf einem Rechner dauert länger
  (Shader werden kompiliert); die App wärmt das Modell deshalb beim Laden vor.
- Synthetische Sprache ist leichter als echte Aufnahmen; die WER-Werte sind nur ein Vergleich der
  Modelle untereinander. Browser-Werte (WASM, CPU, ohne Threads) folgen in Phase 4; erwartet wird
  deutlich langsamer als die native CPU (Schätzung, noch nicht gemessen).
