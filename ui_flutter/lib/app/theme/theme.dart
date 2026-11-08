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
  final radius = BorderRadius.circular(8);
  return ThemeData(
    colorScheme: scheme,
    scaffoldBackgroundColor: clear ? const Color(0x00000000) : (dark ? Palette.base : Palette.baseLight),
    // A desktop feel: dense, no ripples, quiet hover states, square-ish buttons, thin controls.
    visualDensity: VisualDensity.compact,
    splashFactory: NoSplash.splashFactory,
    highlightColor: const Color(0x00000000),
    hoverColor: scheme.onSurface.withValues(alpha: 0.07),
    iconButtonTheme: IconButtonThemeData(style: IconButton.styleFrom(shape: RoundedRectangleBorder(borderRadius: radius))),
    listTileTheme: ListTileThemeData(
      shape: RoundedRectangleBorder(borderRadius: radius),
      contentPadding: const EdgeInsets.symmetric(horizontal: 12),
      horizontalTitleGap: 12,
    ),
    chipTheme: ChipThemeData(
      showCheckmark: false,
      backgroundColor: const Color(0x00000000),
      selectedColor: scheme.primary.withValues(alpha: 0.18),
      side: BorderSide(color: scheme.outlineVariant),
      shape: RoundedRectangleBorder(borderRadius: radius),
      labelStyle: const TextStyle(fontSize: 13),
      padding: const EdgeInsets.symmetric(horizontal: 4),
    ),
    sliderTheme: const SliderThemeData(
      trackHeight: 3,
      thumbShape: RoundSliderThumbShape(enabledThumbRadius: 6),
      overlayShape: RoundSliderOverlayShape(overlayRadius: 14),
    ),
    scrollbarTheme: ScrollbarThemeData(
      thickness: const WidgetStatePropertyAll(6),
      radius: const Radius.circular(3),
    ),
    snackBarTheme: SnackBarThemeData(
      behavior: SnackBarBehavior.floating,
      width: 420,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
    ),
    popupMenuTheme: PopupMenuThemeData(shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10))),
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
