import { describe, expect, it } from 'vitest';
import { guessOs } from './platform.svelte';

describe('guessOs', () => {
  it('knows WebView2 (Windows) and WebKitGTK (Linux)', () => {
    expect(guessOs('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/130.0')).toBe('windows');
    expect(guessOs('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)')).toBe('linux');
  });

  it('does not take Android for Linux, and says other for anything else', () => {
    expect(guessOs('Mozilla/5.0 (Linux; Android 14)')).toBe('other');
    expect(guessOs('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)')).toBe('other');
  });
});
