import 'package:material_ui/material_ui.dart';

/// Pyxee's palette: a near-black ink, a slightly lighter canvas that the pages sit on, and one citrus accent.
abstract final class Ink {
  static const base = Color(0xFF0D0E11);
  static const canvas = Color(0xFF16181D);
  static const accent = Color(0xFFC8F169);
}

ThemeData pyxeeTheme() {
  final scheme = ColorScheme.fromSeed(
    seedColor: Ink.accent,
    brightness: Brightness.dark,
    surface: Ink.canvas,
  );
  return ThemeData(
    colorScheme: scheme,
    scaffoldBackgroundColor: Ink.base,
  );
}
