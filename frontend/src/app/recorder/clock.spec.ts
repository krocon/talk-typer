import { describe, expect, it } from 'vitest';
import { formatClock, formatDuration } from './clock';

describe('clock', () => {
  it('formats durations as HH:MM:SS', () => {
    expect(formatDuration(0)).toBe('00:00:00');
    expect(formatDuration(730_999)).toBe('00:12:10');
    expect(formatDuration(3_600_000 + 61_000)).toBe('01:01:01');
    expect(formatDuration(-5)).toBe('00:00:00');
  });

  it('formats the wall clock', () => {
    expect(formatClock(new Date(2026, 10, 4, 4, 35))).toBe('04:35');
  });
});
