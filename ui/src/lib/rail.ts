import { prefersReducedMotion } from './motion';
import type { Rail } from './snackbar.svelte';

/** How full the rail is `elapsed` ms into a run that began at `from` (1 = full) and lasts `duration` ms. */
export function railScale(from: number, elapsed: number, duration: number): number {
  if (duration <= 0) return 0;
  return Math.min(1, Math.max(0, from * (1 - elapsed / duration)));
}

/**
 * Svelte action for the thin rail along the foot of a snack: it runs down from full to empty in step with the snack's
 * own timer, so an Undo window can be SEEN. One `scaleX` Web Animations animation (compositor only, linear, it ends
 * when the snack goes), held while the snack is held (pointer over it, focus inside it) and carried on from where it
 * stood when the timer runs again. Nothing under reduced motion or without `animate` (the rail is then hidden by CSS).
 */
export function toastRail(node: HTMLElement, rail: Rail) {
  let seq = rail.seq;
  let anim: Animation | null = null;
  let from = 1;
  let duration = rail.ms;

  const now = () => (anim ? railScale(from, Number(anim.currentTime ?? 0), duration) : 1);
  const start = (r: Rail) => {
    const fromScale = r.fresh ? 1 : now();
    anim?.cancel();
    anim = null;
    if (prefersReducedMotion() || typeof node.animate !== 'function') return;
    from = fromScale;
    duration = r.ms;
    anim = node.animate([{ transform: `scaleX(${from})` }, { transform: 'scaleX(0)' }], { duration: r.ms, easing: 'linear', fill: 'forwards' });
    if (r.held) anim.pause?.();
  };
  start(rail);

  return {
    update(r: Rail) {
      if (r.seq !== seq) {
        seq = r.seq;
        start(r);
      } else if (r.held) {
        anim?.pause?.();
      } else if (anim?.playState === 'paused') {
        anim.play();
      }
    },
    destroy() {
      anim?.cancel();
      anim = null;
    },
  };
}
