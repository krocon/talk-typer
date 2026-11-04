// AudioWorklet (browser only): forwards microphone blocks to the main thread, which hands them
// to the STT worker as transferable Float32Arrays. Bundled by scripts/build-worklets.mjs to
// /worklets/audio-capture.js (no Blob URL, strict CSP).

// Minimal AudioWorkletGlobalScope typings (not part of TypeScript's DOM lib).
declare abstract class AudioWorkletProcessor {
  readonly port: MessagePort;
  abstract process(inputs: Float32Array[][], outputs: Float32Array[][]): boolean;
}
declare function registerProcessor(name: string, ctor: new () => AudioWorkletProcessor): void;

/** ~85 ms at 48 kHz: few messages, low latency. */
const BLOCK = 4096;

class AudioCaptureProcessor extends AudioWorkletProcessor {
  private buffer = new Float32Array(BLOCK);
  private filled = 0;

  process(inputs: Float32Array[][]): boolean {
    const mono = inputs[0]?.[0];
    if (!mono) return true;
    let offset = 0;
    while (offset < mono.length) {
      const n = Math.min(BLOCK - this.filled, mono.length - offset);
      this.buffer.set(mono.subarray(offset, offset + n), this.filled);
      this.filled += n;
      offset += n;
      if (this.filled === BLOCK) {
        const out = this.buffer;
        this.port.postMessage(out, [out.buffer]);
        this.buffer = new Float32Array(BLOCK);
        this.filled = 0;
      }
    }
    return true;
  }
}

registerProcessor('audio-capture', AudioCaptureProcessor);
