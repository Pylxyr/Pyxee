import { describe, expect, it } from 'vitest';
import { isVersionHeading, parseChangelog } from './changelog';

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

describe('isVersionHeading', () => {
  it('tells the version sections of a multi-release changelog from ordinary headings', () => {
    for (const v of ['0.4.6', 'v0.4.6', '0.4.2-rc1', '1.0.0+build.5']) expect(isVersionHeading(v), v).toBe(true);
    for (const t of ['New', 'Fixes in 0.4.6', '0.4', 'Earlier releases', '']) expect(isVersionHeading(t), t).toBe(false);
  });

  it('a changelog that spans releases parses into a version heading before each release\'s own sections', () => {
    const blocks = parseChangelog('## 0.4.1\n\n### Fixed\n- a\n\n## 0.4.0\n\n- b');
    const versions = blocks.filter((b) => b.kind === 'heading' && isVersionHeading(b.text));
    expect(versions.map((b) => (b.kind === 'heading' ? b.text : ''))).toEqual(['0.4.1', '0.4.0']);
  });
});
