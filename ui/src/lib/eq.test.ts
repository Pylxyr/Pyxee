import { describe, expect, it } from 'vitest';
import { BAND_HZ, EQ_LIMIT, EQ_PRESETS, EQ_STEP, FLAT_EQ, bandLabel, bandName, clampTenths, formatDb, formatGain, presetOf } from './eq';

describe('eq helpers', () => {
  it('has ten bands, lowest first, and presets that fit them', () => {
    expect(BAND_HZ).toHaveLength(10);
    expect([...BAND_HZ]).toEqual([...BAND_HZ].sort((a, b) => a - b));
    for (const p of EQ_PRESETS) {
      expect(p.gains, p.label).toHaveLength(BAND_HZ.length);
      for (const g of p.gains) {
        expect(Math.abs(g)).toBeLessThanOrEqual(EQ_LIMIT);
        expect(g % EQ_STEP === 0, `${p.label}: ${g} is a whole step`).toBe(true);
      }
    }
  });
  it('starts off and flat, and flat is the first preset', () => {
    expect(FLAT_EQ).toEqual({ enabled: false, preamp: 0, gains: new Array(10).fill(0) });
    expect(EQ_PRESETS[0]!.id).toBe('flat');
    expect(EQ_PRESETS.map((p) => p.label)).toEqual(['Flat', 'Bass boost', 'Vocal', 'Treble']);
  });
  it('names bands for the eye and for a screen reader', () => {
    expect(BAND_HZ.map(bandLabel)).toEqual(['31', '62', '125', '250', '500', '1k', '2k', '4k', '8k', '16k']);
    expect(bandName(62)).toBe('62 Hz');
    expect(bandName(1000)).toBe('1 kHz');
    expect(bandName(16000)).toBe('16 kHz');
  });
  it('writes gains with a real minus sign and no needless decimals', () => {
    expect(formatGain(0)).toBe('0');
    expect(formatGain(60)).toBe('+6');
    expect(formatGain(35)).toBe('+3.5');
    expect(formatGain(-20)).toBe('\u22122');
    expect(formatGain(-5)).toBe('\u22120.5');
    expect(formatDb(120)).toBe('+12 dB');
    expect(formatDb(0)).toBe('0 dB');
  });
  it('recognises a preset by its gains only', () => {
    expect(presetOf(EQ_PRESETS[1]!.gains)?.id).toBe('bass');
    expect(presetOf([...EQ_PRESETS[1]!.gains].reverse())).toBeNull();
    expect(presetOf([0, 0, 0])).toBeNull();
    expect(presetOf(new Array(10).fill(0))?.id).toBe('flat');
  });
  it('keeps a value inside what the equalizer takes', () => {
    expect(clampTenths(500)).toBe(EQ_LIMIT);
    expect(clampTenths(-500)).toBe(-EQ_LIMIT);
    expect(clampTenths(4.6)).toBe(5);
    expect(clampTenths(Number.NaN)).toBe(0);
    expect(clampTenths(Number.POSITIVE_INFINITY)).toBe(0);
  });
});
