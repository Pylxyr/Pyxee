import { afterEach, describe, expect, it, vi } from 'vitest';
import { ThemeStore } from './theme.svelte';
import { crossfadeTheme } from './motion';

const doc = document as unknown as { startViewTransition?: unknown };
const original = window.matchMedia;
const store = () => {
  const mem = new Map<string, string>();
  return new ThemeStore({ getItem: (k) => mem.get(k) ?? null, setItem: (k, v) => void mem.set(k, v) });
};
function fakeTransitions() {
  Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true });
  const start = vi.fn((cb: () => Promise<void>) => {
    const done = Promise.resolve().then(cb);
    return { ready: done, updateCallbackDone: done, finished: done.then(() => new Promise((r) => setTimeout(r, 0))) };
  });
  doc.startViewTransition = start;
  return start;
}
afterEach(() => {
  delete doc.startViewTransition;
  delete (document as unknown as { visibilityState?: string }).visibilityState;
  window.matchMedia = original;
  document.documentElement.removeAttribute('data-theme-fade');
});

describe('choosing a theme crossfades the window', () => {
  it('applies the choice inside one view transition, holds the accent glide off during it, and lets go after', async () => {
    const start = fakeTransitions();
    const t = store();
    const was = t.selectedId;
    t.select('light');
    expect(start).toHaveBeenCalledTimes(1);
    expect(document.documentElement.hasAttribute('data-theme-fade')).toBe(true);
    await vi.waitFor(() => expect(t.settings.themeId).toBe('light'));
    expect(t.selectedId).not.toBe(was);
    await vi.waitFor(() => expect(document.documentElement.hasAttribute('data-theme-fade')).toBe(false));
  });

  it('crossfades for following the system and the cover colour too, and not for a change that is no change', async () => {
    const start = fakeTransitions();
    const t = store();
    t.update({ dynamicColor: true });
    t.update({ followSystem: false });
    expect(start).toHaveBeenCalledTimes(2);
    // the settings are applied inside the transitions, a moment after update() returns
    await vi.waitFor(() => expect(t.settings).toMatchObject({ dynamicColor: true, followSystem: false }));
    t.update({ dynamicColor: true });
    expect(start).toHaveBeenCalledTimes(2);
  });

  it('is never used for the live preview in the editor or the palette that follows the cover', () => {
    const start = fakeTransitions();
    const t = store();
    t.preview = { id: '', name: 'x', scheme: 'dark', colors: t.active.colors };
    t.art = '#336699';
    expect(start).not.toHaveBeenCalled();
  });

  it('just changes the setting at once without view transitions or with less motion asked for', () => {
    const t = store();
    t.select('light');
    expect(t.settings.themeId).toBe('light');
    fakeTransitions();
    window.matchMedia = ((q: string) => ({ matches: q.includes('prefers-reduced-motion'), media: q })) as unknown as typeof window.matchMedia;
    const t2 = store();
    t2.select('light');
    expect(t2.settings.themeId).toBe('light');
    expect((doc.startViewTransition as ReturnType<typeof vi.fn>).mock.calls).toHaveLength(0);
    let ran = 0;
    expect(crossfadeTheme(() => ran++)).toBe(false);
    expect(ran).toBe(1);
  });
});
