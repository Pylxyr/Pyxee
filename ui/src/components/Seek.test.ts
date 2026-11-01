import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import Seek from './Seek.svelte';
import { EMPTY_STATE, player } from '$lib/player.svelte';
import { seekMode } from '$lib/seekmode.svelte';
import { pixelPct } from '$lib/seekbar';

const DUR = 200_000;
const W = 420;

function show(transport: 'playing' | 'paused', positionMs: number) {
  player.state = {
    ...EMPTY_STATE,
    seq: 1,
    transport: transport === 'playing' ? { state: 'playing', id: 1 } : { state: 'paused', id: 1, loaded: true },
    audible: 1,
    current: 1,
    positionMs,
  };
}
const pctOf = (c: HTMLElement) => c.querySelector<HTMLInputElement>('input')!.style.getPropertyValue('--pct');
const mount = () => render(Seek, { positionMs: player.positionMs, durationMs: DUR, onseek: () => {} });
const posNow = () => player.estimatedPositionMs();

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} unobserve() {} });
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockReturnValue({ width: W, height: 20, x: 0, y: 0, top: 0, left: 0, right: W, bottom: 20, toJSON() {} });
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  seekMode.value = 'step';
  player.state = EMPTY_STATE;
});

describe('seek bar, pixel mode', () => {
  beforeEach(() => (seekMode.value = 'pixel'));

  it('moves the fill by one pixel at a time, between the once-a-second reports', async () => {
    show('playing', 10_000);
    const { container } = mount();
    flushSync();
    expect(parseFloat(pctOf(container))).toBeCloseTo(pixelPct(10_000, DUR, W), 6);
    const seen = new Set<string>();
    for (let i = 0; i < 10; i++) {
      await vi.advanceTimersByTimeAsync(100); // 1 s with no report: 2 pixel boundaries (at 476 and 952 ms)
      flushSync();
      seen.add(pctOf(container));
      expect(parseFloat(pctOf(container))).toBeCloseTo(pixelPct(posNow(), DUR, W), 6);
    }
    // one pixel is 100/420 %; the fill took 2 steps in 1 s, each exactly one pixel
    expect(seen.size).toBe(3);
  });

  it('arms nothing while paused, and nothing once the reports have stopped', async () => {
    show('paused', 10_000);
    mount();
    flushSync();
    expect(vi.getTimerCount()).toBe(0);
    show('playing', 10_000);
    flushSync();
    expect(vi.getTimerCount()).toBe(1);
    await vi.advanceTimersByTimeAsync(5_000); // no report: the clock is capped, then the timers stop
    expect(vi.getTimerCount()).toBe(0);
  });

  it('keeps the time label and the slider value at the reported second', async () => {
    show('playing', 10_000);
    const { container } = mount();
    await vi.advanceTimersByTimeAsync(900);
    flushSync();
    expect(container.querySelector('input')!.getAttribute('aria-valuetext')).toBe('0:10 of 3:20');
  });
});

describe('seek bar, glide and steps modes', () => {
  let calls: { frames: Keyframe[]; opts: KeyframeAnimationOptions; startedAt: number; cancel: ReturnType<typeof vi.fn>; pause: ReturnType<typeof vi.fn> }[];
  beforeEach(() => {
    calls = [];
    Element.prototype.animate = function (frames: Keyframe[], opts: KeyframeAnimationOptions) {
      const startedAt = Date.now();
      const a = {
        playState: 'running',
        get currentTime() {
          return Date.now() - startedAt;
        },
        cancel: vi.fn(() => (a.playState = 'idle')),
        pause: vi.fn(() => (a.playState = 'paused')),
      };
      calls.push({ frames, opts, startedAt, cancel: a.cancel, pause: a.pause });
      return a as unknown as Animation;
    } as never;
  });
  afterEach(() => delete (Element.prototype as { animate?: unknown }).animate);
  const fill = (c: HTMLElement) => c.querySelector<HTMLElement>('.seek-fill')!;

  it('starts one animation for the rest of the song and leaves it running across reports', async () => {
    seekMode.value = 'glide';
    show('playing', 10_000);
    const { container } = mount();
    flushSync();
    expect(calls).toHaveLength(1);
    expect(calls[0]!.opts.duration).toBe(DUR - 10_000);
    expect(calls[0]!.opts.easing).toBe('linear');
    expect(calls[0]!.frames).toEqual([{ transform: 'scaleX(0.05)' }, { transform: 'scaleX(1)' }]);
    for (let s = 1; s <= 5; s++) {
      await vi.advanceTimersByTimeAsync(1_000);
      show('playing', 10_000 + s * 1_000); // on time: the clock and the animation agree
      flushSync();
    }
    expect(calls).toHaveLength(1);
    expect(calls[0]!.cancel).not.toHaveBeenCalled();
    expect(fill(container)).toBeTruthy();
    expect(pctOf(container)).toBe('0%'); // the native track's own fill is off
  });

  it('starts again on a seek and on a drift over 300 ms, not on a small one', async () => {
    seekMode.value = 'glide';
    show('playing', 10_000);
    mount();
    flushSync();
    await vi.advanceTimersByTimeAsync(1_000);
    show('playing', 11_250); // 250 ms ahead of the animation: left alone
    flushSync();
    expect(calls).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1_000);
    show('playing', 12_000 + 1_000 + 400); // 400 ms off: restart
    flushSync();
    expect(calls).toHaveLength(2);
    expect(calls[0]!.cancel).toHaveBeenCalled();
    show('playing', 90_000); // a seek
    flushSync();
    expect(calls).toHaveLength(3);
    expect(calls[2]!.opts.duration).toBe(DUR - 90_000);
  });

  it('stops on pause (a still bar, no animation) and when the reports stop', async () => {
    seekMode.value = 'glide';
    show('playing', 10_000);
    const { container } = mount();
    flushSync();
    show('paused', 10_000);
    flushSync();
    expect(calls[0]!.cancel).toHaveBeenCalled();
    expect(fill(container).style.transform).toBe('scaleX(0.05)');
    show('playing', 20_000);
    flushSync();
    expect(calls).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(2_000); // no report for 2 s
    expect(calls[1]!.pause).toHaveBeenCalled();
  });

  it('divides the rest of the song into a few steps a second in steps mode', () => {
    seekMode.value = 'steps';
    show('playing', 10_000);
    mount();
    flushSync();
    expect(calls[0]!.opts.easing).toBe('steps(399, jump-end)'); // 190 s x 4 a second = 760, capped to the 399 px still to fill
  });
});
