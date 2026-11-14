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

/**
 * What the page holds right now: Chromium's JS heap (`performance.memory`, WebView2 has it; the numbers are 0 where a
 * WebView does not), the number of page elements, and the decoded size of the pictures and canvases (width x height;
 * a picture takes about 4 bytes a pixel). Reading it walks the page's pictures once, so it is only done for the log.
 */
export function pageMemory(): Pick<PerfReportDto, 'heapUsedKb' | 'heapTotalKb' | 'domNodes' | 'images' | 'imageKpx' | 'canvases' | 'canvasKpx'> {
  const m = (performance as unknown as { memory?: { usedJSHeapSize: number; totalJSHeapSize: number } }).memory;
  const kb = (n: number) => Math.round(n / 1024);
  let images = 0;
  let imagePx = 0;
  for (const img of Array.from(document.images)) {
    if (img.complete && img.naturalWidth > 0) {
      images++;
      imagePx += img.naturalWidth * img.naturalHeight;
    }
  }
  let canvases = 0;
  let canvasPx = 0;
  for (const c of Array.from(document.getElementsByTagName('canvas'))) {
    canvases++;
    canvasPx += c.width * c.height;
  }
  return {
    heapUsedKb: m ? kb(m.usedJSHeapSize) : 0,
    heapTotalKb: m ? kb(m.totalJSHeapSize) : 0,
    domNodes: document.getElementsByTagName('*').length,
    images,
    imageKpx: Math.round(imagePx / 1000),
    canvases,
    canvasKpx: Math.round(canvasPx / 1000),
  };
}

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
  // The moment the window is hidden or shown again: one line with the page's memory right then (counts as zero), so the
  // log shows what the page held just before it went to sleep and just after it woke.
  const onVisibility = () =>
    report({ seconds: 1, stateEvents: 0, positionEvents: 0, otherEvents: 0, domMutations: 0, runningAnimations: 0, longTasks: 0, longFrames: 0, hidden: document.hidden, ...pageMemory() });
  document.addEventListener('visibilitychange', onVisibility);
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
      ...pageMemory(),
    });
    last = { ...perfCounters };
    lastAt = now;
    mutations = longTasks = longFrames = 0;
  }, intervalMs);
  return () => {
    clearInterval(timer);
    document.removeEventListener('visibilitychange', onVisibility);
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
