import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/theme/art_color.dart';
import '../../core/theme/prefs.dart';

/// Pyxee's palette: a near-black ink (or a soft grey in the light theme), a canvas that the pages sit on, and one
/// accent that comes from the chosen theme or from the album art.
abstract final class Palette {
  static const base = Color(0xFF0D0E11);
  static const canvas = Color(0xFF16181D);
  static const baseLight = Color(0xFFE9EBEF);
  static const canvasLight = Color(0xFFF7F8FA);
  static const accent = Color(0xFFC8F169);
}

/// With [clear] the page background is transparent so that the window's Mica material shows through the margins;
/// the canvas the pages sit on stays opaque.
ThemeData buildTheme(Color seed, Brightness brightness, {required bool clear}) {
  final dark = brightness == Brightness.dark;
  final scheme = ColorScheme.fromSeed(
    seedColor: seed,
    brightness: brightness,
    surface: dark ? Palette.canvas : Palette.canvasLight,
  );
  return ThemeData(
    colorScheme: scheme,
    scaffoldBackgroundColor: clear ? const Color(0x00000000) : (dark ? Palette.base : Palette.baseLight),
  );
}

typedef AppThemes = ({ThemeData light, ThemeData dark, ThemeMode mode});

final appThemeProvider = Provider<AppThemes>((ref) {
  final prefs = ref.watch(prefsProvider);
  final art = prefs.artColors ? ref.watch(artSeedProvider) : null;
  final seed = art ?? Color(prefs.theme.seedValue);
  final clear = prefs.mica && micaSupported;
  return (
    light: buildTheme(seed, Brightness.light, clear: clear),
    dark: buildTheme(seed, Brightness.dark, clear: clear),
    mode: switch (prefs.mode) {
      ModeChoice.system => ThemeMode.system,
      ModeChoice.dark => ThemeMode.dark,
      ModeChoice.light => ThemeMode.light,
    },
  );
});
