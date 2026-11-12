/// A release description (GitHub Markdown) reduced to what a small panel shows: headings, lists and paragraphs.
sealed class ChangelogBlock {
  const ChangelogBlock();
}

class ChangelogHeading extends ChangelogBlock {
  const ChangelogHeading(this.text);
  final String text;
}

class ChangelogList extends ChangelogBlock {
  const ChangelogList(this.items);
  final List<String> items;
}

class ChangelogParagraph extends ChangelogBlock {
  const ChangelogParagraph(this.text);
  final String text;
}

final _image = RegExp(r'!\[([^\]]*)\]\([^)]*\)');
final _link = RegExp(r'\[([^\]]+)\]\([^)]*\)');
final _tag = RegExp(r'<[^>]+>');
final _bold = RegExp(r'(\*\*|__)(.+?)\1');
final _italic = RegExp(r'(\*|_)(.+?)\1');
final _code = RegExp(r'`([^`]+)`');
final _headingLine = RegExp(r'^#{1,6}\s+(.*?)\s*#*$');
final _itemLine = RegExp(r'^(?:[-*+]|\d+[.)])\s+(.*)$');
final _ruleLine = RegExp(r'^(-{3,}|\*{3,}|_{3,})$');
final _versionHeading = RegExp(r'^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$');

/// Drops inline Markup (`**bold**`, `` `code` ``, links, images) and keeps the words.
String _plain(String text) => text
    .replaceAllMapped(_image, (m) => m[1]!)
    .replaceAllMapped(_link, (m) => m[1]!)
    .replaceAll(_tag, '')
    .replaceAllMapped(_bold, (m) => m[2]!)
    .replaceAllMapped(_italic, (m) => m[2]!)
    .replaceAllMapped(_code, (m) => m[1]!)
    .trim();

List<ChangelogBlock> parseChangelog(String markdown) {
  final blocks = <ChangelogBlock>[];
  var paragraph = <String>[];
  List<String>? list;
  var hidden = false; // inside an HTML comment

  void endParagraph() {
    if (paragraph.isNotEmpty) blocks.add(ChangelogParagraph(_plain(paragraph.join(' '))));
    paragraph = [];
  }

  void endList() {
    final items = list;
    if (items != null && items.isNotEmpty) blocks.add(ChangelogList(items));
    list = null;
  }

  for (final raw in markdown.replaceAll(RegExp(r'\r\n?'), '\n').split('\n')) {
    var line = raw.trim();
    if (hidden) {
      if (line.contains('-->')) hidden = false;
      continue;
    }
    if (line.startsWith('<!--')) {
      if (!line.contains('-->')) hidden = true;
      continue;
    }
    final heading = _headingLine.firstMatch(line);
    final item = _itemLine.firstMatch(line);
    if (line.isEmpty || _ruleLine.hasMatch(line)) {
      endParagraph();
      endList();
    } else if (heading != null) {
      endParagraph();
      endList();
      final text = _plain(heading[1] ?? '');
      if (text.isNotEmpty) blocks.add(ChangelogHeading(text));
    } else if (item != null) {
      endParagraph();
      (list ??= []).add(_plain(item[1] ?? ''));
    } else if (list != null && raw.startsWith(' ')) {
      // A wrapped line of the previous list item.
      final items = list!;
      items[items.length - 1] = '${items.last} ${_plain(line)}';
    } else {
      endList();
      line = line.replaceFirst(RegExp(r'^>\s?'), '');
      paragraph.add(line);
    }
  }
  endParagraph();
  endList();
  return [
    for (final b in blocks)
      if (switch (b) {
        ChangelogList(:final items) => items.any((i) => i.isNotEmpty),
        ChangelogHeading(:final text) => text.isNotEmpty,
        ChangelogParagraph(:final text) => text.isNotEmpty,
      })
        b,
  ];
}

/// A heading that is only a version number: the sections `Pyxee` adds when an update spans several releases.
bool isVersionHeading(String text) => _versionHeading.hasMatch(text.trim());
