import { describe, expect, it } from 'vitest';
import { parseChangelog } from './changelog';

describe('parseChangelog', () => {
  it('reads headings, bullet lists and paragraphs', () => {
    const md = '## What\'s new\n\n- **Faster** search\n- Fixes [a bug](https://x.test/1)\n\nThanks for listening.\n';
    expect(parseChangelog(md)).toEqual([
      { kind: 'heading', text: "What's new" },
      { kind: 'list', items: ['Faster search', 'Fixes a bug'] },
      { kind: 'paragraph', text: 'Thanks for listening.' },
    ]);
  });

  it('keeps numbered and starred items as list items and joins wrapped lines', () => {
    expect(parseChangelog('1. first\n   continued\n* second')).toEqual([{ kind: 'list', items: ['first continued', 'second'] }]);
  });

  it('drops comments, rules and inline markup but never produces markup or empty blocks', () => {
    const md = '<!-- hidden\nstill hidden -->\n---\n### `Fixed`\n- <b>bold</b> and `code`\n\n\n';
    expect(parseChangelog(md)).toEqual([
      { kind: 'heading', text: 'Fixed' },
      { kind: 'list', items: ['bold and code'] },
    ]);
  });

  it('an empty description has no blocks', () => {
    expect(parseChangelog('')).toEqual([]);
    expect(parseChangelog('  \n\n')).toEqual([]);
  });
});
