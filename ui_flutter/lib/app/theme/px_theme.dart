import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/theme/art_color.dart';
import '../../core/theme/prefs.dart';
import 'colors.dart';

const fontText = 'Segoe UI Variable Text';
const fontDisplay = 'Bahnschrift';
const fontTextFallback = ['Segoe UI', 'Inter', 'Noto Sans', 'Cantarell', 'Roboto', 'DejaVu Sans'];
const fontDisplayFallback = ['Segoe UI Variable Display', 'Segoe UI', 'Inter', 'Noto Sans', 'Cantarell', 'Roboto', 'DejaVu Sans'];

/// Radii of the Tauri UI (`--r-sm/md/lg`).
const rSm = 6.0;
const rMd = 10.0;
const rLg = 16.0;

/// The colours of the app: the seven of a theme plus the ones `app.css` derives from them (`--layer`, `--hover`,
/// `--selected`, ...). Read it with `context.px`.
@immutable
class PxColors extends ThemeExtension<PxColors> {
  const PxColors({
    required this.dark,
    required this.bg,
    required this.surface,
    required this.surface2,
    required this.line,
    required this.fg,
    required this.muted,
    required this.accent,
    required this.onAccent,
    required this.onFg,
    required this.solid,
    required this.solid2,
    required this.hover,
    required this.press,
    required this.selected,
    required this.stageWash,
    required this.rule,
    required this.glow,
  });

  /// [mica]: the page background is transparent so the window's Mica material shows through, and panels are
  /// translucent tints of the text colour. Three tokens stay opaque on purpose: [solid], [solid2] and [onFg].
  factory PxColors.from(ThemeColors c, {required bool dark, required bool mica}) {
    final tint = _chroma(c.bg) < 8 ? 0.0 : 0.7;
    return PxColors(
      dark: dark,
      bg: mica ? c.bg.withValues(alpha: tint) : c.bg,
      surface: mica ? c.fg.withValues(alpha: dark ? 0.05 : 0.04) : c.surface,
      surface2: mica ? c.fg.withValues(alpha: dark ? 0.10 : 0.08) : c.surface2,
      line: mica ? c.fg.withValues(alpha: dark ? 0.12 : 0.10) : c.line,
      fg: c.fg,
      muted: c.muted,
      accent: c.accent,
      onAccent: readableOn(c.accent),
      onFg: readableOn(c.fg),
      solid: c.bg,
      solid2: c.surface2,
      hover: c.fg.withValues(alpha: dark ? 0.07 : 0.06),
      press: c.fg.withValues(alpha: dark ? 0.11 : 0.10),
      selected: c.accent.withValues(alpha: 0.16),
      stageWash: c.accent.withValues(alpha: 0.22),
      rule: c.fg.withValues(alpha: 0.10),
      glow: c.accent.withValues(alpha: 0.38),
    );
  }

  final bool dark;
  final Color bg;
  final Color surface;
  final Color surface2;
  final Color line;
  final Color fg;
  final Color muted;
  final Color accent;
  final Color onAccent;
  final Color onFg;
  final Color solid;
  final Color solid2;
  final Color hover;
  final Color press;
  final Color selected;
  final Color stageWash;
  final Color rule;
  final Color glow;

  /// `--layer`: what pages and bars are painted on.
  Color get layer => surface;

  @override
  PxColors copyWith({Color? accent}) => PxColors(
    dark: dark,
    bg: bg,
    surface: surface,
    surface2: surface2,
    line: line,
    fg: fg,
    muted: muted,
    accent: accent ?? this.accent,
    onAccent: onAccent,
    onFg: onFg,
    solid: solid,
    solid2: solid2,
    hover: hover,
    press: press,
    selected: selected,
    stageWash: stageWash,
    rule: rule,
    glow: glow,
  );

  @override
  PxColors lerp(ThemeExtension<PxColors>? other, double t) {
    if (other is! PxColors) return this;
    Color l(Color a, Color b) => Color.lerp(a, b, t)!;
    return PxColors(
      dark: t < 0.5 ? dark : other.dark,
      bg: l(bg, other.bg),
      surface: l(surface, other.surface),
      surface2: l(surface2, other.surface2),
      line: l(line, other.line),
      fg: l(fg, other.fg),
      muted: l(muted, other.muted),
      accent: l(accent, other.accent),
      onAccent: l(onAccent, other.onAccent),
      onFg: l(onFg, other.onFg),
      solid: l(solid, other.solid),
      solid2: l(solid2, other.solid2),
      hover: l(hover, other.hover),
      press: l(press, other.press),
      selected: l(selected, other.selected),
      stageWash: l(stageWash, other.stageWash),
      rule: l(rule, other.rule),
      glow: l(glow, other.glow),
    );
  }
}

double _chroma(Color c) {
  final v = [c.r, c.g, c.b].map((x) => x * 255).toList();
  return v.reduce((a, b) => a > b ? a : b) - v.reduce((a, b) => a < b ? a : b);
}

extension PxContext on BuildContext {
  PxColors get px => Theme.of(this).extension<PxColors>()!;
}

/// The type ramp of the Tauri UI (Windows 11: 12/16, 14/20, 18/24, 20/28, 30/36, 40/52).
abstract final class PxText {
  static TextStyle _text(double size, double line, {FontWeight weight = FontWeight.w400, Color? color}) => TextStyle(
    fontFamily: fontText,
    fontFamilyFallback: fontTextFallback,
    fontSize: size,
    height: line / size,
    fontWeight: weight,
    color: color,
    decoration: TextDecoration.none,
  );

  static TextStyle _display(double size, double line, {FontWeight weight = FontWeight.w600, double em = 0, Color? color}) => TextStyle(
    fontFamily: fontDisplay,
    fontFamilyFallback: fontDisplayFallback,
    fontSize: size,
    height: line / size,
    fontWeight: weight,
    letterSpacing: em * size,
    color: color,
    decoration: TextDecoration.none,
  );

  static TextStyle caption([Color? color]) => _text(12, 16, color: color);
  static TextStyle body([Color? color]) => _text(14, 20, color: color);
  static TextStyle strong([Color? color]) => _text(14, 20, weight: FontWeight.w600, color: color);
  static TextStyle lead([Color? color]) => _display(18, 24, color: color);
  static TextStyle subtitle([Color? color]) => _display(20, 28, color: color);
  static TextStyle title([Color? color]) => _display(30, 36, em: -0.01, color: color);
  static TextStyle button([Color? color, FontWeight weight = FontWeight.w400, double size = 14]) =>
      _display(size, 20, weight: weight, color: color);
  static TextStyle tab([Color? color, FontWeight weight = FontWeight.w400]) => _display(15, 20, weight: weight, color: color);

  /// Poster-sized: `clamp(36px, 3.6vw, 60px)`, line height 1.04.
  static TextStyle pageTitle(double width, [Color? color]) {
    final size = (width * 0.036).clamp(36.0, 60.0);
    return _display(size, size * 1.04, em: -0.01, color: color);
  }

  /// `clamp(30px, 3.4vw, 56px)` on the Home hero.
  static TextStyle heroTitle(double width, [Color? color]) {
    final size = (width * 0.034).clamp(30.0, 56.0);
    return _display(size, size * 1.05, em: -0.01, color: color);
  }

  /// `clamp(28px, 2.8vw, 48px)` on Now Playing.
  static TextStyle npTitle(double width, [Color? color]) {
    final size = (width * 0.028).clamp(28.0, 48.0);
    return _display(size, size * 1.05, em: -0.01, color: color);
  }

  static TextStyle wordmark([Color? color]) => _display(18, 24, em: 0.01, color: color);
}

ThemeData buildPxTheme(PxColors p, {required bool mica}) {
  final scheme = ColorScheme(
    brightness: p.dark ? Brightness.dark : Brightness.light,
    primary: p.accent,
    onPrimary: p.onAccent,
    secondary: p.accent,
    onSecondary: p.onAccent,
    error: const Color(0xFFD13438),
    onError: const Color(0xFFFFFFFF),
    surface: p.solid,
    onSurface: p.fg,
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    brightness: scheme.brightness,
    scaffoldBackgroundColor: mica ? const Color(0x00000000) : p.bg,
    canvasColor: p.solid,
    fontFamily: fontText,
    fontFamilyFallback: fontTextFallback,
    splashFactory: NoSplash.splashFactory,
    highlightColor: const Color(0x00000000),
    hoverColor: const Color(0x00000000),
    focusColor: const Color(0x00000000),
    dividerColor: p.rule,
    textSelectionTheme: TextSelectionThemeData(cursorColor: p.accent, selectionColor: p.accent.withValues(alpha: 0.3)),
    scrollbarTheme: ScrollbarThemeData(
      thickness: const WidgetStatePropertyAll(8),
      radius: const Radius.circular(8),
      thumbColor: WidgetStatePropertyAll(p.fg.withValues(alpha: 0.24)),
    ),
    extensions: [p],
  );
}

typedef AppThemes = ({ThemeData light, ThemeData dark, ThemeMode mode});

/// A theme being edited: shown live, not yet saved.
class ThemePreviewNotifier extends Notifier<ThemeDef?> {
  @override
  ThemeDef? build() => null;

  void set(ThemeDef? def) => state = def;
}

final themePreviewProvider = NotifierProvider<ThemePreviewNotifier, ThemeDef?>(ThemePreviewNotifier.new);

/// The theme the app wears: the chosen one, one for light and one for dark when following the system, or (with
/// "Color from album art" on and a cover to take it from) a palette made from the cover.
final appThemeProvider = Provider<AppThemes>((ref) {
  final prefs = ref.watch(prefsProvider);
  final preview = ref.watch(themePreviewProvider);
  final art = prefs.artColors && preview == null ? ref.watch(artSeedProvider) : null;
  final mica = prefs.mica && micaSupported;

  ThemeData make(ThemeDef def) {
    final colors = art != null ? paletteFromArt(art, dark: def.dark) : def.colors;
    return buildPxTheme(PxColors.from(colors, dark: def.dark, mica: mica), mica: mica);
  }

  if (preview != null) {
    final one = make(preview);
    return (light: one, dark: one, mode: preview.dark ? ThemeMode.dark : ThemeMode.light);
  }
  if (prefs.followSystem) {
    return (light: make(themeById(prefs.lightId, prefs.custom)), dark: make(themeById(prefs.darkId, prefs.custom)), mode: ThemeMode.system);
  }
  final def = themeById(prefs.themeId, prefs.custom);
  final one = make(def);
  return (light: one, dark: one, mode: def.dark ? ThemeMode.dark : ThemeMode.light);
});
