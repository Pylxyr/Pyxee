import { describe, expect, it } from 'vitest';
import { viewFromSearch } from './windows';

describe('viewFromSearch', () => {
  it('picks the flyout and the mini player, and anything else is the main window', () => {
    expect(viewFromSearch('?view=flyout')).toBe('flyout');
    expect(viewFromSearch('?view=mini')).toBe('mini');
    expect(viewFromSearch('')).toBe('main');
    expect(viewFromSearch('?view=settings')).toBe('main');
    expect(viewFromSearch('?x=1&view=mini')).toBe('mini');
  });
});
