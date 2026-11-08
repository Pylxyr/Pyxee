/// Sizes and timings the shell is laid out with.
abstract final class Metrics {
  /// From this width the places stand in a spine on the left; below it they sit along the bottom.
  static const railBreakpoint = 720.0;

  /// From this width Orbit puts the cover beside its lists instead of above them.
  static const orbitSplit = 900.0;

  static const titleBarHeight = 36.0;
  static const spineWidth = 208.0;
  static const spineHeight = 64.0;
  static const canvasRadius = 28.0;
  static const canvasInset = 8.0;
  static const capsuleHeight = 72.0;
  static const capsuleGap = 16.0;
  static const capsuleMaxWidth = 760.0;
  static const orbitDuration = Duration(milliseconds: 360);
}
