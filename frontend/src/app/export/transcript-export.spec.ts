import { describe, expect, it } from 'vitest';
import { exportFileName, srtTime, toSrt, toTxt } from './transcript-export';

const seg = (text: string, startMs: number, endMs: number) => ({
  text,
  startMs,
  endMs,
  isFinal: true,
});

describe('transcript export', () => {
  it('writes one line per segment as text', () => {
    expect(
      toTxt([seg(' Hallo. ', 0, 900), seg('', 900, 1000), seg('Wie geht’s?', 1000, 2000)]),
    ).toBe('Hallo.\nWie geht’s?\n');
    expect(toTxt([])).toBe('');
  });

  it('formats SubRip times', () => {
    expect(srtTime(0)).toBe('00:00:00,000');
    expect(srtTime(3_723_456)).toBe('01:02:03,456');
    expect(srtTime(-5)).toBe('00:00:00,000');
  });

  it('writes numbered SubRip cues, skipping empty ones', () => {
    const srt = toSrt([
      seg('Guten Morgen.', 0, 1500),
      seg(' ', 1500, 1600),
      seg('Ä ö ü', 2000, 2000),
    ]);
    expect(srt).toBe(
      '1\r\n00:00:00,000 --> 00:00:01,500\r\nGuten Morgen.\r\n' +
        '\r\n' +
        '2\r\n00:00:02,000 --> 00:00:02,500\r\nÄ ö ü\r\n',
    );
  });

  it('names files after the date', () => {
    expect(exportFileName('srt', new Date(2026, 10, 4, 9, 5))).toBe(
      'talk-typer-2026-11-04-0905.srt',
    );
  });
});
