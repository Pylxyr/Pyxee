import type { Backend } from './backend';
import type { PerfReportDto } from './bindings/PerfReportDto';

/**
 * Opt-in diagnostics (`PYXEE_PERF=1`, see Settings > Advanced): every `intervalMs` the window reports, to the log file,
 * what it did in that time. Off by default, and then nothing here runs except the three integer additions below.
 *
 * It counts what makes a window busy, not frames: counting frames needs a `requestAnimationFrame` loop, which makes the
 * page draw sixty frames a second by itself. The real CPU and GPU figures are Task Manager's (see docs/performance.md).
 */
export const perfCounters = { state: 0, position: 0, other: 0 };

type Entry = { entryType: string };

export function startPerf(report: (r: PerfReportDto) => void, intervalMs = 10_000): () => void {
  let mutations = 0;
  let longTasks = 0;
  let longFrames = 0;
  const mo = new MutationObserver((records) => (mutations += records.length));
  mo.observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
  const observers: PerformanceObserver[] = [];
  for (const type of ['longtask', 'long-animation-frame']) {
    try {
      if (!PerformanceObserver.supportedEntryTypes?.includes(type)) continue;
      const po = new PerformanceObserver((list) => {
        for (const e of list.getEntries() as Entry[]) {
          if (e.entryType === 'longtask') longTasks++;
          else longFrames++;
        }
      });
      po.observe({ type, buffered: false });
      observers.push(po);
    } catch {
      // not supported by this WebView: the counts stay 0
    }
  }
  let last = { ...perfCounters };
  let lastAt = Date.now();
  const timer = setInterval(() => {
    const now = Date.now();
    const seconds = Math.max(1, Math.round((now - lastAt) / 1000));
    const running = document.getAnimations ? document.getAnimations().filter((a) => a.playState === 'running').length : 0;
    report({
      seconds,
      stateEvents: perfCounters.state - last.state,
      positionEvents: perfCounters.position - last.position,
      otherEvents: perfCounters.other - last.other,
      domMutations: mutations,
      runningAnimations: running,
      longTasks,
      longFrames,
      hidden: document.hidden,
    });
    last = { ...perfCounters };
    lastAt = now;
    mutations = longTasks = longFrames = 0;
  }, intervalMs);
  return () => {
    clearInterval(timer);
    mo.disconnect();
    for (const o of observers) o.disconnect();
  };
}

/** Starts the probe if the app was started with `PYXEE_PERF=1`. */
export async function startPerfIfEnabled(backend: Backend): Promise<(() => void) | null> {
  try {
    if (!(await backend.perfEnabled())) return null;
  } catch {
    return null;
  }
  return startPerf((r) => void backend.perfLog(r).catch(() => {}));
}
