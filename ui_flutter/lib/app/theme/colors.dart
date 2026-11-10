import 'dart:convert';
import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

/// Colour helpers and the built-in themes. This is `ui/src/lib/color.ts` and `themes.ts` of the Tauri UI, ported
/// one to one, so a theme (and the palette made from a cover) looks the same in both.

Color hex(String h) => Color(0xFF000000 | int.parse(h.substring(1), radix: 16));

int _byte(double channel) => (channel * 255).round().clamp(0, 255);

Color _rgb(double r, double g, double b) => Color.fromARGB(255, r.round().clamp(0, 255), g.round().clamp(0, 255), b.round().clamp(0, 255));

/// WCAG relative luminance.
double luminance(Color c) {
  double lin(double v) => v <= 0.03928 ? v / 12.92 : math.pow((v + 0.055) / 1.055, 2.4).toDouble();
  return 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
}

double contrast(Color a, Color b) {
  final la = luminance(a);
  final lb = luminance(b);
  final hi = math.max(la, lb);
  final lo = math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

/// The text colour (near-black or white) that reads better on [bg].
Color readableOn(Color bg) {
  const white = Color(0xFFFFFFFF);
  const ink = Color(0xFF0F0F0F);
  return contrast(white, bg) >= contrast(ink, bg) ? white : ink;
}

/// [a] moved [t] (0-1) of the way to [b], in sRGB.
Color mixColors(Color a, Color b, double t) => _rgb(
  _byte(a.r) + (_byte(b.r) - _byte(a.r)) * t,
  _byte(a.g) + (_byte(b.g) - _byte(a.g)) * t,
  _byte(a.b) + (_byte(b.b) - _byte(a.b)) * t,
);

/// Nudges [c] toward white (dark scheme) or black (light scheme) until it reads against [bg].
Color ensureContrastOn(Color c, Color bg, bool dark, [double min = 3]) {
  final target = dark ? 255.0 : 0.0;
  var r = _byte(c.r).toDouble();
  var g = _byte(c.g).toDouble();
  var b = _byte(c.b).toDouble();
  for (var i = 0; i < 20 && contrast(_rgb(r, g, b), bg) < min; i++) {
    r += (target - r) * 0.12;
    g += (target - g) * 0.12;
    b += (target - b) * 0.12;
  }
  return _rgb(r, g, b);
}

/// (hue 0-360, saturation 0-1, lightness 0-1)
(double, double, double) toHsl(Color c) {
  final r = c.r, g = c.g, b = c.b;
  final max = math.max(r, math.max(g, b));
  final min = math.min(r, math.min(g, b));
  final l = (max + min) / 2;
  if (max == min) return (0, 0, l);
  final d = max - min;
  final s = d / (1 - (2 * l - 1).abs());
  final h = max == r ? ((g - b) / d + (g < b ? 6 : 0)) : (max == g ? (b - r) / d + 2 : (r - g) / d + 4);
  return (h * 60, s, l);
}

Color fromHsl(double h, double s, double l) {
  final a = s * math.min(l, 1 - l);
  double f(double n) {
    final k = (n + h / 30) % 12;
    return l - a * math.max(-1, math.min(math.min(k - 3, 9 - k), 1));
  }

  return _rgb(f(0) * 255, f(8) * 255, f(4) * 255);
}

double _clamp(double n, double lo, double hi) => math.max(lo, math.min(hi, n));

/// The seven colours a theme is made of. Everything else (button text, overlays) is derived from them.
@immutable
class ThemeColors {
  const ThemeColors({
    required this.bg,
    required this.surface,
    required this.surface2,
    required this.line,
    required this.fg,
    required this.muted,
    required this.accent,
  });

  final Color bg;
  final Color surface;
  final Color surface2;
  final Color line;
  final Color fg;
  final Color muted;
  final Color accent;

  Color get(String key) => switch (key) {
    'bg' => bg,
    'surface' => surface,
    'surface2' => surface2,
    'line' => line,
    'fg' => fg,
    'muted' => muted,
    _ => accent,
  };

  ThemeColors withKey(String key, Color c) => ThemeColors(
    bg: key == 'bg' ? c : bg,
    surface: key == 'surface' ? c : surface,
    surface2: key == 'surface2' ? c : surface2,
    line: key == 'line' ? c : line,
    fg: key == 'fg' ? c : fg,
    muted: key == 'muted' ? c : muted,
    accent: key == 'accent' ? c : accent,
  );

  Map<String, String> toJson() => {for (final f in colorFields) f.$1: colorToHex(get(f.$1))};
}

/// (key, label, hint) of the seven colours, as the editor lists them.
const colorFields = <(String, String, String)>[
  ('bg', 'Background', 'The page behind everything'),
  ('surface', 'Panels', 'Cards, the player bar, hovered rows'),
  ('surface2', 'Raised', 'Menus, selected rows, slider tracks'),
  ('line', 'Dividers', 'Thin lines between areas'),
  ('fg', 'Text', 'Titles and body text'),
  ('muted', 'Secondary text', 'Artists, durations, icons'),
  ('accent', 'Accent', 'Seek bar, likes, focus ring'),
];

String colorToHex(Color c) => '#${((_byte(c.r) << 16) | (_byte(c.g) << 8) | _byte(c.b)).toRadixString(16).padLeft(6, '0')}';

/// `#abc` and `ABCDEF` become colours; anything else is null.
Color? normalizeHex(String v) {
  var h = v.trim().replaceFirst('#', '').toLowerCase();
  if (RegExp(r'^[0-9a-f]{3}$').hasMatch(h)) h = h.split('').map((c) => c + c).join();
  return RegExp(r'^[0-9a-f]{6}$').hasMatch(h) ? hex('#$h') : null;
}

@immutable
class ThemeDef {
  const ThemeDef(this.id, this.name, this.dark, this.description, this.colors, {this.builtin = false});

  final String id;
  final String name;
  final bool dark;
  final String description;
  final ThemeColors colors;
  final bool builtin;

  ThemeDef copyWith({String? id, String? name, bool? dark, ThemeColors? colors}) =>
      ThemeDef(id ?? this.id, name ?? this.name, dark ?? this.dark, description, colors ?? this.colors);

  Map<String, Object?> toJson() => {'id': id, 'name': name, 'scheme': dark ? 'dark' : 'light', 'colors': colors.toJson()};
}

/// A theme from untrusted data (saved settings, pasted text); null if it is not usable.
ThemeDef? themeFromJson(Object? raw) {
  if (raw is! Map) return null;
  final name = raw['name'] is String ? (raw['name'] as String).trim() : '';
  final scheme = raw['scheme'];
  final colors = raw['colors'];
  if (name.isEmpty || (scheme != 'dark' && scheme != 'light') || colors is! Map) return null;
  final parsed = <String, Color>{};
  for (final f in colorFields) {
    final v = colors[f.$1];
    final c = v is String ? normalizeHex(v) : null;
    if (c == null) return null;
    parsed[f.$1] = c;
  }
  final id = raw['id'] is String && RegExp(r'^[a-z0-9-]{1,40}$').hasMatch(raw['id'] as String) ? raw['id'] as String : '';
  return ThemeDef(
    id,
    name.length > 40 ? name.substring(0, 40) : name,
    scheme == 'dark',
    'Custom theme',
    ThemeColors(bg: parsed['bg']!, surface: parsed['surface']!, surface2: parsed['surface2']!, line: parsed['line']!, fg: parsed['fg']!, muted: parsed['muted']!, accent: parsed['accent']!),
  );
}

String exportTheme(ThemeDef def) =>
    const JsonEncoder.withIndent('  ').convert({'pyxeeTheme': 1, 'name': def.name, 'scheme': def.dark ? 'dark' : 'light', 'colors': def.colors.toJson()});

/// Reads text produced by [exportTheme]; throws a [FormatException] with a sentence the UI can show.
ThemeDef parseThemeText(String text) {
  Object? raw;
  try {
    raw = jsonDecode(text);
  } on FormatException {
    throw const FormatException("That isn't theme text. Paste what Export copied.");
  }
  final def = themeFromJson(raw);
  if (def == null) throw const FormatException('That theme is missing a name, a dark/light setting or one of its colours.');
  return def.copyWith(id: '');
}

class ContrastCheck {
  const ContrastCheck(this.label, this.ratio, this.min);

  final String label;
  final double ratio;
  final double min;
  bool get ok => ratio >= min;
}

/// The pairings that decide whether a theme is readable (WCAG contrast ratios).
List<ContrastCheck> contrastChecks(ThemeColors c) => [
  ContrastCheck('Text on background', contrast(c.fg, c.bg), 4.5),
  ContrastCheck('Text on panels', contrast(c.fg, c.surface), 4.5),
  ContrastCheck('Secondary text on background', contrast(c.muted, c.bg), 4.5),
  ContrastCheck('Secondary text on raised', contrast(c.muted, c.surface2), 3),
  ContrastCheck('Accent on background', contrast(c.accent, c.bg), 3),
];

ThemeDef _t(String id, String name, String scheme, String description, String c) {
  final p = c.split(' ').map(hex).toList();
  return ThemeDef(
    id,
    name,
    scheme == 'dark',
    description,
    ThemeColors(bg: p[0], surface: p[1], surface2: p[2], line: p[3], fg: p[4], muted: p[5], accent: p[6]),
    builtin: true,
  );
}

const defaultDarkId = 'dark';
const defaultLightId = 'light';

final List<ThemeDef> builtinThemes = [
  _t('dark', 'Ink', 'dark', 'Blue-black ink with a signal-lime accent (the default)', '#0c1017 #141a22 #1e2631 #28313e #e9eef5 #92a0b3 #c6f135'),
  _t('light', 'Frost', 'light', 'Soft grey-white with a deep olive-lime accent', '#f2f3f0 #e8eae5 #dcdfd8 #c9cdc3 #12140f #565b50 #4d7c0f'),
  _t('oled', 'OLED black', 'dark', 'Pure black, easy on OLED screens', '#000000 #0a0a0a #1a1a1a #262626 #f2f2f2 #9a9a9a #ff5c6a'),
  _t('deepblue', 'Deep blue', 'dark', 'Blue with violet highlights', '#0b1426 #111d36 #1b2a4a #243558 #e8eefc #9fb0d3 #8b7bff'),
  _t('sky', 'Sky', 'light', 'Light blue with violet highlights', '#f3f8ff #e6f0fc #d5e5f8 #c3d7ee #14233b #50627f #6b4fd8'),
  _t('graphitegold', 'Graphite and gold', 'dark', 'Warm grey with cream text and gold', '#1c1c1a #252522 #33332e #3d3d37 #f1e7c6 #aaa58d #d4a72c'),
  _t('turquoise', 'Turquoise', 'dark', 'Dark slate with a turquoise player colour', '#101416 #181e21 #232b2f #2c363b #eef6f7 #9db1b6 #1fd1c1'),
  _t('ember', 'Ember', 'dark', 'Dark brown with an orange glow', '#17110e #211813 #30221a #3b2a20 #fbeee6 #bfa392 #ff7a2f'),
  _t('paper', 'Paper', 'light', 'Warm off-white with a burnt-orange accent', '#fffaf5 #f6ede4 #ebddd0 #e2d2c3 #2b1d14 #6b5648 #c2410c'),
  _t('amethyst', 'Amethyst', 'dark', 'Deep purple-black with a soft violet accent', '#120d1f #1a1429 #271e3d #33284d #f1ecfb #b0a3cc #b388ff'),
  _t('forest', 'Forest', 'dark', 'Dark green-black with a fresh green accent', '#0d1410 #141d17 #1e2b22 #283a2d #e9f3ec #9bb4a3 #4ade80'),
  _t('crimson', 'Crimson', 'dark', 'Dark wine with a crimson-pink accent', '#170c10 #211217 #301a22 #3d222c #fbeaee #c0a0a9 #ff4d6d'),
  _t('synthwave', 'Synthwave', 'dark', 'Night purple with a hot pink accent', '#130a24 #1c1133 #2a1b4a #37245d #f6eefe #b9a2d6 #ff3ea5'),
  _t('steel', 'Steel', 'dark', 'Cool blue-grey with a sky-blue accent', '#161a20 #1e242c #2a323d #343e4b #e8edf3 #9aa7b8 #5eb8ff'),
  _t('midnightsun', 'Midnight sun', 'dark', 'Navy with a golden-yellow accent', '#0c1320 #131d2f #1d2a42 #26364f #eaf0fa #9aaccb #ffcf4a'),
  _t('abyss', 'Abyss', 'dark', 'Deep sea green with a bright teal accent', '#071a1c #0d2427 #153238 #1d4046 #e6f6f6 #8fb6b9 #2dd4bf'),
  _t('mint', 'Mint', 'light', 'Pale green with a deep teal accent', '#f1faf5 #e3f3ea #d2e9dc #bfdcca #10241a #4a6556 #0f766e'),
  _t('lavender', 'Lavender', 'light', 'Soft lilac with a rich purple accent', '#f7f4fd #ede7f9 #e0d6f3 #d0c3e8 #1d1530 #5d5177 #6d28d9'),
  _t('rose', 'Rose', 'light', 'Blush pink with a raspberry accent', '#fff5f7 #fde8ed #f9d6df #efc3cf #2a1018 #6e4a56 #be185d'),
  _t('sandstone', 'Sandstone', 'light', 'Warm sand with a bronze accent', '#f8f4ec #efe8da #e3d9c5 #d6cab2 #2a2417 #645a44 #8a5a00'),
  _t('silver', 'Silver', 'light', 'Neutral light grey with a clear blue accent', '#f4f5f7 #e9ebee #dcdfe4 #cbd0d7 #16181d #565c68 #2563eb'),
  _t('lemonade', 'Lemonade', 'light', 'Pale lemon with a dark gold accent', '#fffdf0 #f7f3d9 #ede7bf #e0d9a6 #26230d #615c33 #926200'),
];

const maxCustomThemes = 30;

/// The built-in themes and the ones the user made or imported.
List<ThemeDef> allThemes(List<ThemeDef> custom) => [...builtinThemes, ...custom];

ThemeDef themeById(String id, [List<ThemeDef> custom = const []]) =>
    allThemes(custom).firstWhere((t) => t.id == id, orElse: () => builtinThemes.first);

/// How much of a cover's colour the surfaces take: a grey cover stays grey, a coloured one is clearly tinted.
double _surfaceTint(double s, double ceiling) => s < 0.08 ? s : _clamp(s * 0.8, 0.22, ceiling);

/// A whole theme from one colour (the cover's): background, panels, lines, text and accent all take its hue. Dark
/// or light follows [dark]. The text and the accent are nudged until they read, as the built-in themes do.
ThemeColors paletteFromArt(Color art, {required bool dark}) {
  final (h, s, l) = toHsl(art);
  if (dark) {
    final n = _surfaceTint(s, 0.6);
    final surface2 = fromHsl(h, n, 0.21);
    return ThemeColors(
      bg: fromHsl(h, n, 0.115),
      surface: fromHsl(h, n, 0.155),
      surface2: surface2,
      line: fromHsl(h, n * 0.9, 0.3),
      fg: fromHsl(h, n * 0.4, 0.95),
      muted: ensureContrastOn(fromHsl(h, n * 0.8, 0.7), surface2, true, 4.5),
      accent: ensureContrastOn(fromHsl(h, _clamp(s, 0.6, 0.95), _clamp(l, 0.58, 0.74)), surface2, true),
    );
  }
  final n = _surfaceTint(s, 0.5);
  final surface2 = fromHsl(h, n, 0.855);
  return ThemeColors(
    bg: fromHsl(h, n, 0.935),
    surface: fromHsl(h, n, 0.9),
    surface2: surface2,
    line: fromHsl(h, n * 0.9, 0.72),
    fg: fromHsl(h, n * 0.6, 0.1),
    muted: ensureContrastOn(fromHsl(h, n * 0.7, 0.34), surface2, false, 4.5),
    accent: ensureContrastOn(fromHsl(h, _clamp(s, 0.6, 0.95), _clamp(l, 0.28, 0.4)), surface2, false),
  );
}

/// A placeholder cover colour from a string, as `hueFor` in the Tauri UI: stable per song.
Color placeholderColor(String key) {
  var h = 0;
  for (final unit in key.codeUnits) {
    h = (h * 31 + unit) & 0xFFFFFFFF;
  }
  return fromHsl((h % 360).toDouble(), 0.38, 0.32);
}
