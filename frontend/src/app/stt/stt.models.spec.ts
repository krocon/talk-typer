import { describe, expect, it } from 'vitest';
import { toSttError } from './stt.models';

describe('toSttError', () => {
  it('keeps code and message of a backend error', () => {
    expect(toSttError({ code: 'no_model', message: 'no model loaded' })).toEqual({
      code: 'no_model',
      message: 'no model loaded',
    });
  });

  it('maps anything else to internal', () => {
    expect(toSttError(new Error('boom'))).toEqual({ code: 'internal', message: 'boom' });
    expect(toSttError('x')).toEqual({ code: 'internal', message: 'x' });
  });
});
