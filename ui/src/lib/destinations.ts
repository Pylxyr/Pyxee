import type { Route } from './nav.svelte';

export interface Destination {
  route: Route;
  label: string;
  icon: string;
}

/** The four top-level places. The side rail (wide windows) and the bottom bar (narrow ones) show the same list. */
export const DESTINATIONS: readonly Destination[] = [
  { route: 'home', label: 'Home', icon: 'home' },
  { route: 'explore', label: 'Explore', icon: 'explore' },
  { route: 'library', label: 'Library', icon: 'library' },
  { route: 'queue', label: 'Queue', icon: 'queue' },
];
