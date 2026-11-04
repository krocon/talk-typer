import { SttError } from './stt.models';

/** Smallest module using a SIMD instruction (i8x16.splat) – from wasm-feature-detect. */
const SIMD_PROBE = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15,
  253, 98, 11,
]);

/** Checks everything the browser backend needs; `null` = supported. */
export function checkBrowserSupport(): SttError | null {
  if (!globalThis.isSecureContext) {
    return { code: 'insecure_context', message: 'microphone needs HTTPS or localhost' };
  }
  if (!navigator.mediaDevices?.getUserMedia) {
    return { code: 'unsupported_browser', message: 'getUserMedia missing' };
  }
  if (typeof AudioWorkletNode === 'undefined') {
    return { code: 'unsupported_browser', message: 'AudioWorklet missing' };
  }
  if (typeof WebAssembly !== 'object' || !WebAssembly.validate(SIMD_PROBE)) {
    return { code: 'unsupported_browser', message: 'WebAssembly SIMD missing' };
  }
  return null;
}
