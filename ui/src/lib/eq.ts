import type { EqDto } from './bindings/EqDto';

/** Band centres, lowest first. The Rust side has the same ten (`pyxee_audio::eq::BAND_HZ`). */
export const BAND_HZ = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;
/** Gains and the preamp are whole tenths of a dB, 12 dB either way, moved in steps of half a dB. */
export const EQ_LIMIT = 120;
export const EQ_STEP = 5;

export type EqPreset = { id: string; label: string; gains: readonly number[] };

export const EQ_PRESETS: readonly EqPreset[] = [
  { id: 'flat', label: 'Flat', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { id: 'bass', label: 'Bass boost', gains: [60, 50, 40, 20, 0, 0, 0, 0, 0, 0] },
  { id: 'vocal', label: 'Vocal', gains: [-30, -20, -10, 0, 10, 30, 40, 30, 10, 0] },
  { id: 'treble', label: 'Treble', gains: [0, 0, 0, 0, 0, 10, 20, 40, 50, 60] },
];

export const FLAT_EQ: EqDto = { enabled: false, preamp: 0, gains: [...EQ_PRESETS[0]!.gains] };

/** "31", "125", "1k", "16k": short enough to sit under a slider. */
export function bandLabel(hz: number): string {
  return hz >= 1000 ? `${hz / 1000}k` : `${hz}`;
}

/** "1 kHz", "62 Hz": what a screen reader says. */
export function bandName(hz: number): string {
  return hz >= 1000 ? `${hz / 1000} kHz` : `${hz} Hz`;
}

export function clampTenths(tenths: number): number {
  if (!Number.isFinite(tenths)) return 0;
  return Math.max(-EQ_LIMIT, Math.min(EQ_LIMIT, Math.round(tenths)));
}

/** "+3.5", "0", "\u22122" (a real minus sign): short enough to sit over a slider. */
export function formatGain(tenths: number): string {
  const db = tenths / 10;
  if (db === 0) return '0';
  const text = Number.isInteger(db) ? `${Math.abs(db)}` : Math.abs(db).toFixed(1);
  return `${db > 0 ? '+' : '\u2212'}${text}`;
}

/** "+3.5 dB", "0 dB", "\u22122 dB": what a screen reader and the preamp line say. */
export function formatDb(tenths: number): string {
  return `${formatGain(tenths)} dB`;
}

/** The preset whose gains these are, if any. The preamp and the switch are not part of a preset. */
export function presetOf(gains: readonly number[]): EqPreset | null {
  return EQ_PRESETS.find((p) => p.gains.length === gains.length && p.gains.every((g, i) => g === gains[i])) ?? null;
}
