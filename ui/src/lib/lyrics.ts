import type { LyricLineDto } from './bindings/LyricLineDto';
import type { SleepStatusDto } from './bindings/SleepStatusDto';

/** Index of the line being sung at `positionMs` (the last one that has started); -1 before the first line. */
export function currentLine(lines: readonly LyricLineDto[], positionMs: number): number {
  let lo = 0;
  let hi = lines.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid]!.timeMs <= positionMs) lo = mid + 1;
    else hi = mid;
  }
  return lo - 1;
}

/** "28 min left", "45 s left", "End of song", "2 songs left". */
export function sleepLabel(s: SleepStatusDto | null): string {
  if (!s) return 'Off';
  switch (s.kind) {
    case 'minutes': {
      const secs = Math.ceil(s.remainingMs / 1000);
      if (secs < 60) return `${Math.max(secs, 0)} s left`;
      return `${Math.ceil(secs / 60)} min left`;
    }
    case 'endOfTrack':
      return 'End of song';
    case 'tracks':
      return s.left === 1 ? 'Last song' : `${s.left} songs left`;
  }
}
