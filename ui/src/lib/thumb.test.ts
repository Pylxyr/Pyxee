import { describe, expect, it } from 'vitest';
import { deviceSize, thumbCandidates } from './thumb';

describe('deviceSize', () => {
  it('scales by the display ratio, rounds up to 64 and caps at 1200', () => {
    expect(deviceSize(56, 1)).toBe(64);
    expect(deviceSize(56, 2)).toBe(128);
    expect(deviceSize(144, 1.5)).toBe(256);
    expect(deviceSize(2000, 2)).toBe(1200);
    expect(deviceSize(50, 0)).toBe(64);
  });
});

describe('thumbCandidates', () => {
  const hq = 'https://i.ytimg.com/vi/abc123/hqdefault.jpg';

  it('uses the bar-free 16:9 still for small covers (the 4:3 hqdefault has black bars baked in)', () => {
    expect(thumbCandidates(hq, 56, 1)).toEqual(['https://i.ytimg.com/vi/abc123/mqdefault.jpg']);
  });

  it('asks for the largest still for big covers and falls back one step at a time', () => {
    expect(thumbCandidates(hq, 176, 2)).toEqual([
      'https://i.ytimg.com/vi/abc123/maxresdefault.jpg',
      'https://i.ytimg.com/vi/abc123/hq720.jpg',
      'https://i.ytimg.com/vi/abc123/mqdefault.jpg',
    ]);
  });

  it('keeps the file type and upgrades http', () => {
    expect(thumbCandidates('http://i.ytimg.com/vi_webp/x/default.webp', 400, 1)[0]).toBe(
      'https://i.ytimg.com/vi_webp/x/maxresdefault.webp',
    );
  });

  it('resizes Google cover URLs to the size shown, and falls back to the original', () => {
    const cover = 'https://lh3.googleusercontent.com/AbC=w120-h120-l90-rj';
    expect(thumbCandidates(cover, 176, 2)).toEqual([
      'https://lh3.googleusercontent.com/AbC=w384-h384-l90-rj',
      cover,
    ]);
    const avatar = 'https://yt3.ggpht.com/xyz=s88-c-k-c0x00ffffff-no-rj';
    expect(thumbCandidates(avatar, 56, 1)[0]).toBe('https://yt3.ggpht.com/xyz=s64-c-k-c0x00ffffff-no-rj');
  });

  it('leaves other URLs (local art, other hosts) alone', () => {
    expect(thumbCandidates('https://example.com/a.jpg', 100, 2)).toEqual(['https://example.com/a.jpg']);
    expect(thumbCandidates('https://lh3.googleusercontent.com/NoParams', 100, 2)).toEqual([
      'https://lh3.googleusercontent.com/NoParams',
    ]);
  });
});
