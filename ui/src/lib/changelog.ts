/** A release description (GitHub Markdown) reduced to what a small panel shows: headings, lists and paragraphs. */
export type ChangelogBlock =
  | { kind: 'heading'; text: string }
  | { kind: 'list'; items: string[] }
  | { kind: 'paragraph'; text: string };

/** Drops inline Markup (`**bold**`, `` `code` ``, links, images) and keeps the words. */
function plain(text: string): string {
  return text
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/<[^>]+>/g, '')
    .replace(/(\*\*|__)(.+?)\1/g, '$2')
    .replace(/(\*|_)(.+?)\1/g, '$2')
    .replace(/`([^`]+)`/g, '$1')
    .trim();
}

export function parseChangelog(markdown: string): ChangelogBlock[] {
  const blocks: ChangelogBlock[] = [];
  let paragraph: string[] = [];
  let list: string[] | null = null;
  let hidden = false; // inside an HTML comment

  const endParagraph = () => {
    if (paragraph.length) blocks.push({ kind: 'paragraph', text: plain(paragraph.join(' ')) });
    paragraph = [];
  };
  const endList = () => {
    if (list?.length) blocks.push({ kind: 'list', items: list });
    list = null;
  };

  for (const raw of markdown.replace(/\r\n?/g, '\n').split('\n')) {
    let line = raw.trim();
    if (hidden) {
      if (line.includes('-->')) hidden = false;
      continue;
    }
    if (line.startsWith('<!--')) {
      if (!line.includes('-->')) hidden = true;
      continue;
    }
    const heading = /^#{1,6}\s+(.*?)\s*#*$/.exec(line);
    const item = /^(?:[-*+]|\d+[.)])\s+(.*)$/.exec(line);
    if (!line || /^(-{3,}|\*{3,}|_{3,})$/.test(line)) {
      endParagraph();
      endList();
    } else if (heading) {
      endParagraph();
      endList();
      const text = plain(heading[1] ?? '');
      if (text) blocks.push({ kind: 'heading', text });
    } else if (item) {
      endParagraph();
      (list ??= []).push(plain(item[1] ?? ''));
    } else if (list && raw.startsWith(' ')) {
      // A wrapped line of the previous list item.
      list[list.length - 1] += ' ' + plain(line);
    } else {
      endList();
      line = line.replace(/^>\s?/, '');
      paragraph.push(line);
    }
  }
  endParagraph();
  endList();
  return blocks.filter((b) => (b.kind === 'list' ? b.items.some(Boolean) : b.text !== ''));
}
