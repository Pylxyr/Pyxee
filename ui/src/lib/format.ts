/** 83_000 -> "1:23", 3_725_000 -> "1:02:05". Unknown durations render as "–:––". */
export function formatTime(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return '–:––';
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** "0:42 / 3:45"; when the length is unknown only the elapsed time, never a dead "–:––". */
export function formatProgress(positionMs: number, durationMs: number | null | undefined): string {
  return durationMs ? `${formatTime(positionMs)} / ${formatTime(durationMs)}` : formatTime(positionMs);
}

/** Stable hue (0-359) from a string, used for placeholder art. */
export function hueFor(key: string): number {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) >>> 0;
  return h % 360;
}

export interface DayGroup<T> {
  label: string;
  items: T[];
}

/** Groups newest-first items under "Today", "Yesterday" or a date, by the viewer's local calendar day. */
export function groupByDay<T>(items: T[], at: (item: T) => number, now: number): DayGroup<T>[] {
  const day = (ms: number) => {
    const d = new Date(ms);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  };
  const today = day(now);
  const groups: DayGroup<T>[] = [];
  for (const item of items) {
    const d = day(at(item));
    const diff = Math.round((today - d) / 86_400_000);
    const label =
      diff === 0 ? 'Today' : diff === 1 ? 'Yesterday' : new Date(d).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric', year: diff > 300 ? 'numeric' : undefined });
    const last = groups.at(-1);
    if (last && last.label === label) last.items.push(item);
    else groups.push({ label, items: [item] });
  }
  return groups;
}
