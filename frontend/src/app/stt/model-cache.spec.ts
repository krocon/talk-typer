import { describe, expect, it } from 'vitest';
import { sha256Hex } from './model-cache';

describe('sha256Hex', () => {
  it('hashes like the catalogue expects (lower-case hex)', async () => {
    const abc = new TextEncoder().encode('abc') as Uint8Array<ArrayBuffer>;
    expect(await sha256Hex(abc)).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    );
  });
});
