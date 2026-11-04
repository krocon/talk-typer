# Test audio

- `de-sample.wav` – German, 16 kHz mono PCM16, about 5 s. Generated on macOS (no third-party
  recording, no licence questions):

  ```bash
  say -v Anna -o de.aiff "Guten Morgen. Heute ist ein schöner Tag, und wir testen die Spracherkennung."
  afconvert -f WAVE -d LEI16@16000 -c 1 de.aiff de-sample.wav
  ```

- `de-long.wav` – German, 16 kHz mono PCM16, 38,5 s, varied sentences (chunking, splits):

  ```bash
  say -v Anna -o de-long.aiff "Die Spracherkennung läuft vollständig lokal auf diesem Rechner. …"
  afconvert -f WAVE -d LEI16@16000 -c 1 de-long.aiff de-long.wav
  ```
  Full text: see `streaming_de.rs` / `docs/models.md` (measurements).
