import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Backend } from './backend';
import type { PerfReportDto } from './bindings/PerfReportDto';
import { perfCounters, startPerf, startPerfIfEnabled } from './perf';

beforeEach(() => {
  vi.useFakeTimers();
  perfCounters.state = perfCounters.position = perfCounters.other = 0;
});
afterEach(() => vi.useRealTimers());

describe('the performance log (PYXEE_PERF)', () => {
  it('reports what the window did in each interval, then starts again from zero', async () => {
    const reports: PerfReportDto[] = [];
    const stop = startPerf((r) => reports.push(r), 10_000);
    perfCounters.state += 2;
    perfCounters.position += 10;
    perfCounters.other += 1;
    const el = document.createElement('div');
    document.body.append(el); // a page change the observer sees
    await vi.advanceTimersByTimeAsync(10_000);
    expect(reports).toHaveLength(1);
    expect(reports[0]).toMatchObject({ seconds: 10, stateEvents: 2, positionEvents: 10, otherEvents: 1, longTasks: 0, longFrames: 0, hidden: false });
    expect(reports[0]!.domMutations).toBeGreaterThanOrEqual(1);
    perfCounters.position += 3;
    await vi.advanceTimersByTimeAsync(10_000);
    expect(reports[1]).toMatchObject({ stateEvents: 0, positionEvents: 3, otherEvents: 0, domMutations: 0 });
    stop();
    el.remove();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(reports).toHaveLength(2); // stopped: nothing more, no timer left
    expect(vi.getTimerCount()).toBe(0);
  });

  it('does nothing at all while the app was not started with PYXEE_PERF=1', async () => {
    const log = vi.fn(async () => {});
    const off = { perfEnabled: async () => false, perfLog: log } as unknown as Backend;
    expect(await startPerfIfEnabled(off)).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
    const broken = { perfEnabled: async () => Promise.reject(new Error('no ipc')), perfLog: log } as unknown as Backend;
    expect(await startPerfIfEnabled(broken)).toBeNull();
    const on = { perfEnabled: async () => true, perfLog: log } as unknown as Backend;
    const stop = await startPerfIfEnabled(on);
    expect(stop).toBeTypeOf('function');
    await vi.advanceTimersByTimeAsync(10_000);
    expect(log).toHaveBeenCalledTimes(1);
    stop!();
  });
});
