import { TranscriptSegment } from '../stt/stt.models';

/** Plain text: one line per segment. */
export function toTxt(segments: readonly TranscriptSegment[]): string {
  const lines = segments.map((s) => s.text.trim()).filter(Boolean);
  return lines.length ? `${lines.join('\n')}\n` : '';
}

/** `HH:MM:SS,mmm` (SubRip). */
export function srtTime(ms: number): string {
  const t = Math.max(0, Math.round(ms));
  const pad = (n: number, w = 2): string => String(n).padStart(w, '0');
  const h = Math.floor(t / 3_600_000);
  const m = Math.floor((t % 3_600_000) / 60_000);
  const s = Math.floor((t % 60_000) / 1000);
  return `${pad(h)}:${pad(m)}:${pad(s)},${pad(t % 1000, 3)}`;
}

/** SubRip subtitles: numbered cues with start/end times (CRLF line ends, as players expect). */
export function toSrt(segments: readonly TranscriptSegment[]): string {
  const cues = segments
    .filter((s) => s.text.trim())
    .map((s, i) => {
      const end = Math.max(s.endMs, s.startMs + 500);
      return `${i + 1}\r\n${srtTime(s.startMs)} --> ${srtTime(end)}\r\n${s.text.trim()}\r\n`;
    });
  return cues.join('\r\n');
}

/** `talk-typer-2026-11-04-1905.txt` */
export function exportFileName(ext: 'txt' | 'srt', date = new Date()): string {
  const pad = (n: number): string => String(n).padStart(2, '0');
  const stamp = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}`;
  return `talk-typer-${stamp}.${ext}`;
}
