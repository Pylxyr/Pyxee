import { describe, expect, it, vi } from 'vitest';
import type { Backend } from './backend';
import type { SleepStatusDto } from './bindings/SleepStatusDto';

/** Pretends the window is hidden (tray) or shown, and tells the page, like the browser does. */
function setHidden(hidden: boolean) {
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden });
  document.dispatchEvent(new Event('visibilitychange'));
}

describe('SleepStore', () => {
  it('takes the timer state again when the page wakes up, and not while it is hidden', async () => {
    const { sleep } = await import('./sleep.svelte');
    let current: SleepStatusDto | null = null;
    let reads = 0;
    const backend = {
      sleepGet: async () => (reads++, { status: current }),
      onSleep: async () => () => {},
    } as unknown as Backend;
    await sleep.init(backend);
    expect(reads).toBe(1);
    current = { kind: 'endOfTrack' };
    setHidden(true);
    await Promise.resolve();
    expect(reads, 'hiding asks nothing').toBe(1);
    expect(sleep.status).toBeNull();
    setHidden(false);
    await vi.waitFor(() => expect(sleep.status).toEqual({ kind: 'endOfTrack' }));
    expect(reads).toBe(2);
  });

  it('keeps what it shows when the timer state cannot be read on waking', async () => {
    const { sleep } = await import('./sleep.svelte');
    let fail = false;
    const backend = {
      sleepGet: async () => {
        if (fail) throw new Error('gone');
        return { status: { kind: 'tracks', left: 2 } as SleepStatusDto };
      },
      onSleep: async () => () => {},
    } as unknown as Backend;
    await sleep.init(backend);
    fail = true;
    setHidden(true);
    setHidden(false);
    await Promise.resolve();
    expect(sleep.status).toEqual({ kind: 'tracks', left: 2 });
  });
});
