import { beforeEach, describe, expect, it } from 'vitest';
import { look } from './look.svelte';
import { DEFAULT_LOOK } from './look';
import { seekMode, setSeekMode } from './seekmode.svelte';

beforeEach(() => {
  localStorage.clear();
  look.reset();
  document.documentElement.removeAttribute('data-corners');
});

describe('look store', () => {
  it('saves a choice and puts it on the root element', () => {
    look.set('corners', 'square');
    look.apply();
    expect(JSON.parse(localStorage.getItem('pyxee.look')!).corners).toBe('square');
    expect(document.documentElement.getAttribute('data-corners')).toBe('square');
  });
  it('takes the attribute off again for the default, and for a reset', () => {
    look.set('corners', 'soft');
    look.apply();
    look.reset();
    look.apply();
    expect(document.documentElement.hasAttribute('data-corners')).toBe(false);
    expect(look.value).toEqual(DEFAULT_LOOK);
    expect(localStorage.getItem('pyxee.look')).toBeNull();
  });
});

describe('seek bar choice', () => {
  it('is kept in storage', () => {
    setSeekMode('steps');
    expect(seekMode.value).toBe('steps');
    expect(localStorage.getItem('pyxee.seek')).toBe('steps');
  });
});
