import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';

import '../app/theme/px_theme.dart';

/// The icon set of the Tauri UI (`components/Icon.svelte`): 24 x 24 outlines drawn with a 2 px round stroke, a few of
/// them filled. The path data is the same, parsed here, so the icons look exactly as they do there.
const Map<String, String> _paths = {
  'home': 'M3 11l9-8 9 8v10h-6v-6H9v6H3z',
  'explore': 'M12 3a9 9 0 100 18 9 9 0 000-18zm3.5 5.5l-2 5-5 2 2-5z',
  'settings':
      'M18.7 10.0 L21.7 10.3 L21.7 13.7 L18.7 14.0 L18.2 15.3 L20.0 17.6 L17.6 20.0 L15.3 18.2 L14.0 18.7 L13.7 21.7 L10.3 21.7 L10.0 18.7 L8.7 18.2 L6.4 20.0 L4.0 17.6 L5.8 15.3 L5.3 14.0 L2.3 13.7 L2.3 10.3 L5.3 10.0 L5.8 8.7 L4.0 6.4 L6.4 4.0 L8.7 5.8 L10.0 5.3 L10.3 2.3 L13.7 2.3 L14.0 5.3 L15.3 5.8 L17.6 4.0 L20.0 6.4 L18.2 8.7z M15 12a3 3 0 11-6 0 3 3 0 016 0z',
  'library': 'M4 4v16M9 4v16M14 6l6 14',
  'search': 'M11 4a7 7 0 100 14 7 7 0 000-14zm9 16l-4-4',
  'play': 'M7 4l13 8-13 8z',
  'pause': 'M8 5v14M16 5v14',
  'prev': 'M6 5v14M19 5L9 12l10 7z',
  'next': 'M18 5v14M5 5l10 7-10 7z',
  'shuffle': 'M3 7h4l10 10h4M3 17h4l3-3M14 10l3-3h4M18 4l3 3-3 3M18 14l3 3-3 3',
  'repeat': 'M17 2l3 3-3 3M4 11V9a4 4 0 014-4h12M7 22l-3-3 3-3M20 13v2a4 4 0 01-4 4H4',
  'heart': 'M12 20s-8-5-8-11a4.5 4.5 0 018-2.8A4.5 4.5 0 0120 9c0 6-8 11-8 11z',
  'volume': 'M4 9v6h4l5 4V5L8 9zM17 9a4 4 0 010 6',
  'up': 'M6 15l6-6 6 6',
  'down': 'M6 9l6 6 6-6',
  'more': 'M12 5h.01M12 12h.01M12 19h.01',
  'drag': 'M9 6h.01M9 12h.01M9 18h.01M15 6h.01M15 12h.01M15 18h.01',
  'close': 'M6 6l12 12M18 6L6 18',
  'minimize': 'M5 12h14',
  'maximize': 'M5.5 5.5h13v13h-13z',
  'restore': 'M8.5 8.5h10v10h-10zM5.5 15.5v-10h10',
  'top': 'M5 4h14M12 20V9M6 14l6-6 6 6',
  'expand': 'M14 4h6v6M10 20H4v-6M20 4l-7 7M4 20l7-7',
  'moon': 'M20.5 13.5A8.5 8.5 0 1110.5 3.5a6.5 6.5 0 0010 10z',
  'plus': 'M12 5v14M5 12h14',
  'queue': 'M4 6h12M4 12h12M4 18h7M17 14v6l5-3z',
  'music': 'M9 18V5l11-2v13M9 18a3 3 0 11-6 0 3 3 0 016 0zm11-2a3 3 0 11-6 0 3 3 0 016 0z',
};

final Map<String, ui.Path> _cache = {};

ui.Path _iconPath(String name) => _cache.putIfAbsent(name, () => parseSvgPath(_paths[name] ?? ''));

/// A small SVG path parser: M L H V C S Q A Z, absolute and relative, with the compact number and flag forms
/// (`a9 9 0 100 18`) the icons are written in.
ui.Path parseSvgPath(String d) {
  final path = ui.Path();
  final r = _PathReader(d);
  var x = 0.0, y = 0.0, sx = 0.0, sy = 0.0;
  double? cx, cy; // the last control point, for the smooth curves
  String? cmd;
  while (true) {
    r.skipSeparators();
    if (r.done) break;
    if (r.atCommand) {
      cmd = r.command();
    } else if (cmd == null) {
      break;
    }
    final String c = cmd;
    final rel = c == c.toLowerCase();
    final up = c.toUpperCase();
    switch (up) {
      case 'M':
        final nx = r.number() + (rel ? x : 0);
        final ny = r.number() + (rel ? y : 0);
        path.moveTo(nx, ny);
        x = sx = nx;
        y = sy = ny;
        cmd = rel ? 'l' : 'L';
        cx = cy = null;
      case 'L':
        final nx = r.number() + (rel ? x : 0);
        final ny = r.number() + (rel ? y : 0);
        path.lineTo(nx, ny);
        x = nx;
        y = ny;
        cx = cy = null;
      case 'H':
        final nx = r.number() + (rel ? x : 0);
        path.lineTo(nx, y);
        x = nx;
        cx = cy = null;
      case 'V':
        final ny = r.number() + (rel ? y : 0);
        path.lineTo(x, ny);
        y = ny;
        cx = cy = null;
      case 'C':
        final x1 = r.number() + (rel ? x : 0), y1 = r.number() + (rel ? y : 0);
        final x2 = r.number() + (rel ? x : 0), y2 = r.number() + (rel ? y : 0);
        final nx = r.number() + (rel ? x : 0), ny = r.number() + (rel ? y : 0);
        path.cubicTo(x1, y1, x2, y2, nx, ny);
        cx = x2;
        cy = y2;
        x = nx;
        y = ny;
      case 'S':
        final x1 = cx == null ? x : 2 * x - cx, y1 = cy == null ? y : 2 * y - cy;
        final x2 = r.number() + (rel ? x : 0), y2 = r.number() + (rel ? y : 0);
        final nx = r.number() + (rel ? x : 0), ny = r.number() + (rel ? y : 0);
        path.cubicTo(x1, y1, x2, y2, nx, ny);
        cx = x2;
        cy = y2;
        x = nx;
        y = ny;
      case 'Q':
        final x1 = r.number() + (rel ? x : 0), y1 = r.number() + (rel ? y : 0);
        final nx = r.number() + (rel ? x : 0), ny = r.number() + (rel ? y : 0);
        path.quadraticBezierTo(x1, y1, nx, ny);
        x = nx;
        y = ny;
        cx = cy = null;
      case 'A':
        final rx = r.number(), ry = r.number(), rot = r.number();
        final large = r.flag(), sweep = r.flag();
        final nx = r.number() + (rel ? x : 0), ny = r.number() + (rel ? y : 0);
        path.arcToPoint(
          Offset(nx, ny),
          radius: Radius.elliptical(rx, ry),
          rotation: rot,
          largeArc: large,
          clockwise: sweep,
        );
        x = nx;
        y = ny;
        cx = cy = null;
      case 'Z':
        path.close();
        x = sx;
        y = sy;
        cx = cy = null;
      default:
        return path;
    }
  }
  return path;
}

class _PathReader {
  _PathReader(this.s);

  final String s;
  int i = 0;
  static final _num = RegExp(r'[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?');

  bool get done => i >= s.length;

  bool get atCommand => RegExp('[MmLlHhVvCcSsQqAaZz]').hasMatch(s[i]);

  void skipSeparators() {
    while (i < s.length && (s[i] == ' ' || s[i] == ',' || s[i] == '\n' || s[i] == '\t')) {
      i++;
    }
  }

  String command() => s[i++];

  double number() {
    skipSeparators();
    final m = _num.matchAsPrefix(s, i);
    if (m == null) throw FormatException('number expected at $i in "$s"');
    i = m.end;
    return double.parse(m.group(0)!);
  }

  /// An arc flag is one character, `0` or `1`, with or without a separator after it.
  bool flag() {
    skipSeparators();
    final c = s[i++];
    return c == '1';
  }
}

/// One icon, drawn in the surrounding [IconTheme]'s colour unless [color] is given. [fill] also fills the shape
/// (the play triangle, a liked heart).
class PxIcon extends StatelessWidget {
  const PxIcon(this.name, {super.key, this.size = 22, this.fill = false, this.color});

  final String name;
  final double size;
  final bool fill;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final c = color ?? IconTheme.of(context).color ?? context.px.fg;
    return SizedBox.square(
      dimension: size,
      child: CustomPaint(painter: _IconPainter(_iconPath(name), c, fill)),
    );
  }
}

class _IconPainter extends CustomPainter {
  _IconPainter(this.path, this.color, this.fill);

  final ui.Path path;
  final Color color;
  final bool fill;

  @override
  void paint(Canvas canvas, Size size) {
    canvas.save();
    canvas.scale(size.width / 24, size.height / 24);
    if (fill) canvas.drawPath(path, Paint()..color = color..style = PaintingStyle.fill);
    canvas.drawPath(
      path,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round,
    );
    canvas.restore();
  }

  @override
  bool shouldRepaint(_IconPainter old) => old.color != color || old.fill != fill || old.path != path;
}
