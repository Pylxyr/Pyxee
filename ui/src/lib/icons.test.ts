import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { BUILTIN_THEMES } from './themes';

const root = resolve(__dirname, '../../..');
const read = (p: string) => readFileSync(resolve(root, p));

/** Sizes in an .ico directory (0 means 256) and whether each entry is a PNG. */
function icoFrames(buf: Buffer) {
  expect(buf.readUInt16LE(0)).toBe(0);
  expect(buf.readUInt16LE(2)).toBe(1);
  const n = buf.readUInt16LE(4);
  return Array.from({ length: n }, (_, i) => {
    const e = 6 + 16 * i;
    const size = buf[e]! || 256;
    const len = buf.readUInt32LE(e + 8);
    const off = buf.readUInt32LE(e + 12);
    return { size, png: buf.subarray(off, off + 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])), inside: off + len <= buf.length };
  });
}

describe('app icons', () => {
  it('icon.ico carries 16 to 256 px, all intact PNG frames', () => {
    const frames = icoFrames(read('crates/app/icons/icon.ico'));
    expect(frames.map((f) => f.size)).toEqual([16, 24, 32, 48, 64, 128, 256]);
    expect(frames.every((f) => f.png && f.inside)).toBe(true);
  });

  it('the tray icon has the small sizes', () => {
    expect(icoFrames(read('crates/app/icons/tray.ico')).map((f) => f.size)).toEqual([16, 24, 32]);
  });

  it('every icon Tauri is configured with exists', () => {
    const conf = JSON.parse(read('crates/app/tauri.conf.json').toString());
    for (const f of conf.bundle.icon as string[]) expect(existsSync(resolve(root, 'crates/app', f)), f).toBe(true);
  });

  it('the PNGs are square and have the stated sizes', () => {
    const dims = (p: string) => {
      const b = read(p);
      return [b.readUInt32BE(16), b.readUInt32BE(20)];
    };
    expect(dims('crates/app/icons/32x32.png')).toEqual([32, 32]);
    expect(dims('crates/app/icons/128x128.png')).toEqual([128, 128]);
    expect(dims('crates/app/icons/128x128@2x.png')).toEqual([256, 256]);
    expect(dims('crates/app/icons/icon.png')).toEqual([512, 512]);
    for (const s of [16, 24, 32]) expect(dims(`crates/app/icons/tray-${s}.png`)).toEqual([s, s]);
  });
});

describe('the logo and the accent colour agree', () => {
  const hex = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  it('the SVG sources and the Logo component use the logo red', () => {
    for (const text of [read('assets/logo/logo.svg').toString(), read('assets/logo/tray.svg').toString(), readFileSync(resolve(__dirname, '../components/Logo.svelte'), 'utf8')]) {
      expect(text).toMatch(/#F00524/i);
    }
  });

  it('the UI accent of Dark and Light is that red, nudged only enough for readable text on buttons', () => {
    // #ec0422 instead of #f00524: white text on it passes 4.5:1 (the exact logo red gives 4.42).
    for (const id of ['dark', 'light']) {
      const accent = BUILTIN_THEMES.find((t) => t.id === id)!.colors.accent;
      const [r, g, b] = hex(accent);
      const [lr, lg, lb] = hex('#f00524');
      expect(Math.max(Math.abs(r! - lr!), Math.abs(g! - lg!), Math.abs(b! - lb!)), id).toBeLessThanOrEqual(32);
    }
  });
});
