import 'dart:math' as math;
import 'dart:ui' show FontFeature, ImageFilter;

import 'package:flutter/gestures.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../app/theme/colors.dart';
import '../app/theme/px_theme.dart';
import '../core/state/providers.dart';
import '../core/theme/prefs.dart';
import 'icons.dart';

/// 83_000 -> "1:23", 3_725_000 -> "1:02:05". Unknown durations render as "–:––".
String formatTime(int? ms) {
  if (ms == null || ms < 0) return '–:––';
  final total = ms ~/ 1000;
  final h = total ~/ 3600;
  final m = (total % 3600) ~/ 60;
  final s = (total % 60).toString().padLeft(2, '0');
  return h > 0 ? '$h:${m.toString().padLeft(2, '0')}:$s' : '$m:$s';
}

String songsCount(int n) => '$n ${n == 1 ? 'song' : 'songs'}';

/// The page margin (`--page-pad`): 12 px on a narrow window, 24, then more from 1600 and 1920 px.
double pagePad(double width) => width < 900 ? 12 : (width >= 1920 ? 40 : (width >= 1600 ? 32 : 24));

/// Asks Google's image servers for a cover at about the size it is needed, instead of whatever small size the stored
/// address names. Other hosts are left alone.
String sizedCover(String url, int px) {
  if (!url.contains('googleusercontent.com') && !url.contains('ggpht.com')) return url;
  final m = RegExp(r'=(w\d+-h\d+|s\d+)[^/]*$').firstMatch(url);
  if (m == null) return url;
  final target = ((px + 63) ~/ 64) * 64;
  return '${url.substring(0, m.start)}=w$target-h$target-l90-rj';
}

// ------------------------------------------------------------------------------------------------------ pointer

/// Something to click on a desktop: a pointer cursor and a builder that learns whether the pointer is over it or
/// pressing it.
class PxPress extends StatefulWidget {
  const PxPress({super.key, required this.onTap, required this.builder, this.onSecondaryTap, this.semanticLabel});

  final VoidCallback? onTap;
  final VoidCallback? onSecondaryTap;
  final String? semanticLabel;
  final Widget Function(BuildContext context, bool hovered, bool pressed) builder;

  @override
  State<PxPress> createState() => _PxPressState();
}

class _PxPressState extends State<PxPress> {
  bool _hovered = false;
  bool _pressed = false;

  @override
  Widget build(BuildContext context) => MouseRegion(
    cursor: widget.onTap == null ? MouseCursor.defer : SystemMouseCursors.click,
    onEnter: (_) => setState(() => _hovered = true),
    onExit: (_) => setState(() => _hovered = false),
    child: Semantics(
      button: true,
      label: widget.semanticLabel,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onTap,
        onSecondaryTap: widget.onSecondaryTap,
        onTapDown: widget.onTap == null ? null : (_) => setState(() => _pressed = true),
        onTapUp: (_) => setState(() => _pressed = false),
        onTapCancel: () => setState(() => _pressed = false),
        child: widget.builder(context, _hovered, _pressed),
      ),
    ),
  );
}

// ------------------------------------------------------------------------------------------------------ buttons

enum PxButtonKind { secondary, primary, quiet }

/// `.btn`: 34 px high, 10 px radius, hairline frame; `primary` carries the accent (one per view); `quiet` has no frame
/// until it is pointed at. `small` is 28 px.
class PxButton extends StatelessWidget {
  const PxButton({super.key, required this.label, required this.onPressed, this.kind = PxButtonKind.secondary, this.small = false, this.icon, this.iconFill = false, this.trailing});

  final String label;
  final VoidCallback? onPressed;
  final PxButtonKind kind;
  final bool small;
  final String? icon;
  final bool iconFill;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final enabled = onPressed != null;
    return PxPress(
      onTap: onPressed,
      semanticLabel: label,
      builder: (context, hovered, pressed) {
        final primary = kind == PxButtonKind.primary;
        final quiet = kind == PxButtonKind.quiet;
        Color bg = Colors.transparent;
        Color border = quiet ? Colors.transparent : p.rule;
        if (primary) {
          bg = p.accent;
          border = Colors.transparent;
        } else if (enabled && pressed) {
          bg = p.press;
        } else if (enabled && hovered) {
          bg = p.hover;
          if (!quiet) border = p.muted;
        }
        final fg = primary ? p.onAccent : p.fg;
        Widget child = AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          constraints: BoxConstraints(minHeight: small ? 28 : 34),
          padding: EdgeInsets.symmetric(horizontal: small ? 12 : 16),
          decoration: BoxDecoration(
            color: bg,
            border: Border.all(color: border),
            borderRadius: BorderRadius.circular(rMd),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              if (icon != null) ...[
                IconTheme(data: IconThemeData(color: fg), child: PxIcon(icon!, size: 16, fill: iconFill)),
                const SizedBox(width: 8),
              ],
              Text(label, style: PxText.button(fg, primary ? FontWeight.w600 : FontWeight.w400, small ? 13 : 14), maxLines: 1),
              if (trailing != null) ...[const SizedBox(width: 8), trailing!],
            ],
          ),
        );
        if (primary && enabled && hovered) child = ColorFiltered(colorFilter: const ColorFilter.matrix(_brighten), child: child);
        return AnimatedScale(
          scale: enabled && pressed ? 0.96 : 1,
          duration: const Duration(milliseconds: 120),
          child: Opacity(opacity: enabled ? 1 : 0.4, child: child),
        );
      },
    );
  }
}

const _brighten = <double>[1.08, 0, 0, 0, 0, 0, 1.08, 0, 0, 0, 0, 0, 1.08, 0, 0, 0, 0, 0, 1, 0];

/// `.icon-btn`: 32 px (40 px `large`), quiet icon that turns to the text colour when pointed at, and to the accent
/// when it is a switch that is on.
class PxIconButton extends StatelessWidget {
  const PxIconButton(this.icon, {super.key, required this.label, required this.onPressed, this.large = false, this.strong = false, this.iconSize = 20, this.fill = false, this.pressed = false, this.color, this.child, this.width, this.quarterTurns = 0});

  final String icon;
  final String label;
  final VoidCallback? onPressed;
  final bool large;
  final bool strong;
  final double iconSize;
  final bool fill;

  /// A button that switches something on and off reads as pressed and turns accent.
  final bool pressed;
  final Color? color;
  final Widget? child;
  final double? width;
  final int quarterTurns;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final side = large ? 40.0 : 32.0;
    final enabled = onPressed != null;
    return Tooltip(
      message: label,
      waitDuration: const Duration(milliseconds: 600),
      child: PxPress(
        onTap: onPressed,
        semanticLabel: label,
        builder: (context, hovered, down) {
          final base = color ?? (pressed ? p.accent : (strong ? p.fg : p.muted));
          final fg = (enabled && hovered && !pressed && color == null) ? p.fg : base;
          return Opacity(
            opacity: enabled ? 1 : 0.3,
            child: AnimatedScale(
              scale: enabled && down ? 0.88 : 1,
              duration: const Duration(milliseconds: 100),
              child: Container(
                width: width ?? side,
                height: side,
                decoration: BoxDecoration(
                  color: enabled && down ? p.press : (enabled && hovered ? p.hover : Colors.transparent),
                  borderRadius: BorderRadius.circular(rSm),
                ),
                alignment: Alignment.center,
                child: IconTheme(
                  data: IconThemeData(color: fg),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      quarterTurns == 0 ? PxIcon(icon, size: iconSize, fill: fill) : RotatedBox(quarterTurns: quarterTurns, child: PxIcon(icon, size: iconSize, fill: fill)),
                      ?child,
                    ],
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// `.play-btn`: the accent button with a rounded-square (36 %) shape; 44 px, 56 px (`large`) or 36 px (`small`).
class PxPlayButton extends ConsumerWidget {
  const PxPlayButton({super.key, this.size = PxPlaySize.medium, this.announceLoading = true});

  final PxPlaySize size;
  final bool announceLoading;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final transport = ref.watch(playerProvider.select((s) => s.transport));
    final playing = transport.playing;
    final dim = switch (size) { PxPlaySize.small => 36.0, PxPlaySize.medium => 44.0, PxPlaySize.large => 56.0 };
    final iconSize = switch (size) { PxPlaySize.small => 18.0, PxPlaySize.medium => 22.0, PxPlaySize.large => 28.0 };
    final label = playing ? 'Pause' : (announceLoading && transport.busy ? 'Loading, press to pause' : 'Play');
    return PxPress(
      onTap: ref.read(actionsProvider).toggle,
      semanticLabel: label,
      builder: (context, hovered, pressed) => AnimatedScale(
        scale: pressed ? 0.93 : 1,
        duration: const Duration(milliseconds: 100),
        child: Container(
          width: dim,
          height: dim,
          decoration: BoxDecoration(color: p.accent, borderRadius: BorderRadius.circular(dim * 0.36)),
          alignment: Alignment.center,
          child: transport.state == 'loading'
              ? SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2, color: p.onAccent, backgroundColor: p.onAccent.withValues(alpha: 0.3)),
                )
              : IconTheme(
                  data: IconThemeData(color: p.onAccent),
                  child: PxIcon(playing ? 'pause' : 'play', size: iconSize, fill: !playing),
                ),
        ),
      ),
    );
  }
}

enum PxPlaySize { small, medium, large }

/// `.chip`: a genre. Pressed chips are filled with the text colour.
class PxChip extends StatelessWidget {
  const PxChip({super.key, required this.label, required this.selected, required this.onTap, this.dashed = false});

  final String label;
  final bool selected;
  final VoidCallback onTap;
  final bool dashed;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return PxPress(
      onTap: onTap,
      semanticLabel: label,
      builder: (context, hovered, pressed) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
        decoration: BoxDecoration(
          color: selected ? p.fg : (hovered ? p.hover : Colors.transparent),
          borderRadius: BorderRadius.circular(rMd),
          border: selected ? null : Border.all(color: p.line),
        ),
        child: Text(
          label,
          style: PxText.body(selected ? p.onFg : (dashed ? p.muted : p.fg)).copyWith(fontWeight: selected ? FontWeight.w600 : FontWeight.w400),
        ),
      ),
    );
  }
}

/// `.segmented`: plain tabs on a hairline, the chosen one with an accent underline. One control for two jobs
/// (tabs that switch panels, a toggle group that picks a filter).
class PxSegmented<T> extends StatelessWidget {
  const PxSegmented({super.key, required this.items, required this.value, required this.onSelect});

  final List<({T id, String label, bool disabled, String? tooltip})> items;
  final T value;
  final ValueChanged<T> onSelect;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Container(
      decoration: BoxDecoration(border: Border(bottom: BorderSide(color: p.rule))),
      child: Wrap(
        spacing: 4,
        children: [
          for (final it in items)
            Opacity(
              opacity: it.disabled ? 0.4 : 1,
              child: _wrapTip(
                it.disabled ? it.tooltip : null,
                PxPress(
                  onTap: it.disabled ? null : () => onSelect(it.id),
                  semanticLabel: it.label,
                  builder: (context, hovered, pressed) {
                    final on = value == it.id;
                    // No `alignment` on the Container: it would stretch the tab to the full width of the row.
                    return Container(
                      height: 32,
                      padding: const EdgeInsets.symmetric(horizontal: 10),
                      decoration: BoxDecoration(
                        color: hovered && !it.disabled ? p.hover : Colors.transparent,
                        borderRadius: const BorderRadius.vertical(top: Radius.circular(rSm)),
                        border: on ? Border(bottom: BorderSide(color: p.accent, width: 2)) : null,
                      ),
                      child: Center(
                        widthFactor: 1,
                        child: Text(
                          it.label,
                          style: PxText.body(on || (hovered && !it.disabled) ? p.fg : p.muted).copyWith(fontWeight: on ? FontWeight.w600 : FontWeight.w400),
                          maxLines: 1,
                        ),
                      ),
                    );
                  },
                ),
              ),
            ),
        ],
      ),
    );
  }
}

Widget _wrapTip(String? message, Widget child) => message == null ? child : Tooltip(message: message, child: child);

/// A settings row with a switch (`.setting` and `input.switch`), with an optional hint under it.
class PxToggle extends StatelessWidget {
  const PxToggle({super.key, required this.label, required this.checked, required this.onChanged, this.hint});

  final String label;
  final String? hint;
  final bool checked;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        PxPress(
          onTap: () => onChanged(!checked),
          semanticLabel: label,
          builder: (context, hovered, pressed) => Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: hovered ? p.hover : Colors.transparent,
              border: Border(bottom: BorderSide(color: p.rule)),
            ),
            child: Row(
              children: [
                Expanded(child: Text(label, style: PxText.body(p.fg))),
                const SizedBox(width: 16),
                AnimatedContainer(
                  duration: const Duration(milliseconds: 160),
                  width: 40,
                  height: 20,
                  decoration: BoxDecoration(
                    color: checked ? p.accent : Colors.transparent,
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: checked ? p.accent : p.muted),
                  ),
                  child: AnimatedAlign(
                    duration: const Duration(milliseconds: 160),
                    alignment: checked ? Alignment.centerRight : Alignment.centerLeft,
                    child: Container(
                      margin: const EdgeInsets.symmetric(horizontal: 3),
                      width: 12,
                      height: 12,
                      decoration: BoxDecoration(color: checked ? p.onAccent : p.muted, shape: BoxShape.circle),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        if (hint != null) Padding(padding: const EdgeInsets.fromLTRB(12, 4, 12, 8), child: Text(hint!, style: PxText.caption(p.muted))),
      ],
    );
  }
}

// ---------------------------------------------------------------------------------------------------- seek bar

/// The seek bar (`input.slider`): a 4 px track filled up to the position, with a thumb that shows when the pointer is
/// over it. The position is watched here, so only this widget rebuilds four times a second. With [accent] the fill is
/// the accent colour; the volume slider fills with the text colour.
class PxSeek extends ConsumerStatefulWidget {
  const PxSeek({super.key, required this.durationMs, this.label = 'Seek'});

  final int? durationMs;
  final String label;

  @override
  ConsumerState<PxSeek> createState() => _PxSeekState();
}

class _PxSeekState extends ConsumerState<PxSeek> {
  double? _drag;
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final total = widget.durationMs;
    if (total == null || total <= 0) {
      // Length unknown: no fake scrubber, a still striped bar.
      return SizedBox(
        height: 20,
        child: Center(
          child: Container(
            height: 4,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(2),
              gradient: LinearGradient(
                tileMode: TileMode.repeated,
                begin: const Alignment(-1, -0.2),
                end: const Alignment(-0.9, 0.2),
                colors: [p.surface2, p.surface2, mixColors(p.surface2, p.accent, 0.55), mixColors(p.surface2, p.accent, 0.55)],
                stops: const [0, 0.5, 0.5, 1],
              ),
            ),
          ),
        ),
      );
    }
    final position = ref.watch(positionProvider);
    final value = (_drag ?? position / total).clamp(0.0, 1.0);
    return LayoutBuilder(
      builder: (context, box) {
        final width = box.maxWidth;
        double at(Offset local) => (local.dx / math.max(1, width)).clamp(0.0, 1.0);
        return MouseRegion(
          cursor: SystemMouseCursors.click,
          onEnter: (_) => setState(() => _hover = true),
          onExit: (_) => setState(() => _hover = false),
          child: Semantics(
            slider: true,
            label: widget.label,
            value: '${formatTime(position)} of ${formatTime(total)}',
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTapDown: (d) => setState(() => _drag = at(d.localPosition)),
              onTapUp: (d) {
                ref.read(actionsProvider).seek((at(d.localPosition) * total).round());
                setState(() => _drag = null);
              },
              onTapCancel: () => setState(() => _drag = null),
              onHorizontalDragStart: (d) => setState(() => _drag = at(d.localPosition)),
              onHorizontalDragUpdate: (d) => setState(() => _drag = at(d.localPosition)),
              onHorizontalDragEnd: (_) {
                final v = _drag;
                if (v != null) ref.read(actionsProvider).seek((v * total).round());
                setState(() => _drag = null);
              },
              child: SizedBox(
                height: 20,
                child: CustomPaint(
                  painter: _BarPainter(fraction: value, fill: p.accent, track: p.surface2, thumb: p.fg, thumbVisible: _hover || _drag != null, grow: _drag != null),
                  size: Size(width, 20),
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

class _BarPainter extends CustomPainter {
  _BarPainter({required this.fraction, required this.fill, required this.track, required this.thumb, required this.thumbVisible, this.grow = false});

  final double fraction;
  final Color fill;
  final Color track;
  final Color thumb;
  final bool thumbVisible;
  final bool grow;

  @override
  void paint(Canvas canvas, Size size) {
    final cy = size.height / 2;
    final bar = RRect.fromLTRBR(0, cy - 2, size.width, cy + 2, const Radius.circular(2));
    canvas.drawRRect(bar, Paint()..color = track);
    final x = size.width * fraction;
    if (x > 0) {
      canvas.drawRRect(RRect.fromLTRBR(0, cy - 2, x, cy + 2, const Radius.circular(2)), Paint()..color = fill);
    }
    if (thumbVisible) {
      final w = grow ? 12.0 : 10.0;
      canvas.drawRRect(
        RRect.fromLTRBR(x - w / 2, cy - 8, x + w / 2, cy + 8, const Radius.circular(4)),
        Paint()..color = thumb,
      );
    }
  }

  @override
  bool shouldRepaint(_BarPainter old) =>
      old.fraction != fraction || old.fill != fill || old.track != track || old.thumbVisible != thumbVisible || old.grow != grow;
}

/// The volume: a speaker and a slider filled with the text colour.
class PxVolume extends ConsumerStatefulWidget {
  const PxVolume({super.key});

  @override
  ConsumerState<PxVolume> createState() => _PxVolumeState();
}

class _PxVolumeState extends ConsumerState<PxVolume> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final volume = ref.watch(volumeProvider);
    return Row(
      children: [
        IconTheme(data: IconThemeData(color: p.muted), child: const PxIcon('volume', size: 20)),
        const SizedBox(width: 8),
        Expanded(
          child: LayoutBuilder(
            builder: (context, box) {
              void set(Offset local) => ref.read(volumeProvider.notifier).set((local.dx / math.max(1, box.maxWidth) * 100).round().clamp(0, 100));
              return MouseRegion(
                cursor: SystemMouseCursors.click,
                onEnter: (_) => setState(() => _hover = true),
                onExit: (_) => setState(() => _hover = false),
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTapDown: (d) => set(d.localPosition),
                  onHorizontalDragUpdate: (d) => set(d.localPosition),
                  child: SizedBox(
                    height: 20,
                    child: CustomPaint(
                      painter: _BarPainter(fraction: volume / 100, fill: p.fg, track: p.surface2, thumb: p.fg, thumbVisible: _hover),
                      size: Size(box.maxWidth, 20),
                    ),
                  ),
                ),
              );
            },
          ),
        ),
      ],
    );
  }
}

// ------------------------------------------------------------------------------------------------------ covers

/// A cover at the size it is shown at, decoded at twice that size and filtered down so it stays sharp. Without a
/// picture (or when it fails) a tint made from [seed] with a note, as in the Tauri UI.
class PxThumb extends StatelessWidget {
  const PxThumb({super.key, required this.url, required this.seed, this.size, this.radius = rSm});

  /// With a [size] the box is that big; without one it fills what it is given.
  final String? url;
  final String seed;
  final double? size;
  final double radius;

  @override
  Widget build(BuildContext context) {
    Widget placeholder(double box) => ColoredBox(
      color: placeholderColor(seed),
      child: Center(child: IconTheme(data: const IconThemeData(color: Color(0xCCFFFFFF)), child: PxIcon('music', size: math.max(12, box / 2.6)))),
    );

    Widget build(double box) {
      final src = url;
      final px = (box * MediaQuery.devicePixelRatioOf(context) * 1.5).round().clamp(64, 800);
      return ClipRRect(
        borderRadius: BorderRadius.circular(radius),
        child: src == null
            ? placeholder(box)
            : Image.network(
                sizedCover(src, px),
                width: box,
                height: box,
                cacheWidth: px,
                fit: BoxFit.cover,
                gaplessPlayback: true,
                filterQuality: FilterQuality.medium,
                errorBuilder: (_, _, _) => placeholder(box),
              ),
      );
    }

    final s = size;
    if (s != null) return SizedBox.square(dimension: s, child: build(s));
    return LayoutBuilder(builder: (context, box) {
      final side = box.biggest.shortestSide.isFinite ? box.biggest.shortestSide : 160.0;
      return SizedBox.square(dimension: side, child: build(side));
    });
  }
}

/// A square cover with the glow, the hairline and the shadow of the deck, the hero and Now Playing.
class PxArt extends StatelessWidget {
  const PxArt({super.key, required this.url, required this.seed, required this.size, this.radius = rSm, this.glow = true});

  final String? url;
  final String seed;
  final double size;
  final double radius;
  final bool glow;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(radius),
        boxShadow: [
          BoxShadow(color: Colors.black.withValues(alpha: p.dark ? 0.34 : 0.2), blurRadius: size > 100 ? 34 : 18, offset: Offset(0, size > 100 ? 14 : 8)),
          if (glow) BoxShadow(color: p.glow, blurRadius: size > 100 ? 56 : 28, spreadRadius: size > 100 ? -12 : -8),
        ],
      ),
      foregroundDecoration: BoxDecoration(
        borderRadius: BorderRadius.circular(radius),
        border: Border.all(color: p.fg.withValues(alpha: 0.14)),
      ),
      child: PxThumb(url: url, seed: seed, size: size, radius: radius),
    );
  }
}

/// The blurred wash of a cover behind the Home hero and Now Playing (`.backdrop`): the cover, small, stretched over the
/// whole area and blurred, so it is shapes of its colours and nothing else.
class PxBackdrop extends StatelessWidget {
  const PxBackdrop({super.key, required this.url, required this.seed, this.opacity = 0.55});

  final String? url;
  final String seed;
  final double opacity;

  @override
  Widget build(BuildContext context) {
    final src = url;
    return Positioned.fill(
      child: IgnorePointer(
        child: RepaintBoundary(
          child: Opacity(
            opacity: opacity,
            child: ClipRect(
              child: ImageFiltered(
                imageFilter: ImageFilter.blur(sigmaX: 36, sigmaY: 36, tileMode: TileMode.mirror),
                child: src == null
                    ? ColoredBox(color: placeholderColor(seed))
                    : Image.network(
                        sizedCover(src, 96),
                        fit: BoxFit.cover,
                        cacheWidth: 64,
                        filterQuality: FilterQuality.low,
                        gaplessPlayback: true,
                        errorBuilder: (_, _, _) => ColoredBox(color: placeholderColor(seed)),
                      ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// ------------------------------------------------------------------------------------------- page furniture

/// `.skeleton`: placeholder rows with the geometry of a track row, so nothing jumps when the real ones arrive.
class PxSkeleton extends StatefulWidget {
  const PxSkeleton({super.key, this.rows = 6});

  final int rows;

  @override
  State<PxSkeleton> createState() => _PxSkeletonState();
}

class _PxSkeletonState extends State<PxSkeleton> with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(vsync: this, duration: const Duration(milliseconds: 1400))..repeat();

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    Widget bar(double? w, double h) => AnimatedBuilder(
      animation: _c,
      builder: (context, _) => Container(
        width: w,
        height: h,
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(rSm),
          gradient: LinearGradient(
            colors: [p.surface2, mixColors(p.surface2, p.fg, 0.09), p.surface2],
            begin: Alignment(-1 + _c.value * 3, 0),
            end: Alignment(0 + _c.value * 3, 0),
          ),
        ),
      ),
    );
    return Semantics(
      label: 'Loading',
      child: Column(
        children: [
          for (var i = 0; i < widget.rows; i++)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              child: Row(
                children: [
                  const SizedBox(width: 44),
                  bar(40, 40),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        FractionallySizedBox(widthFactor: 0.66, child: bar(null, 14)),
                        const SizedBox(height: 8),
                        FractionallySizedBox(widthFactor: 0.33, child: bar(null, 12)),
                      ],
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class PxEmptyState extends StatelessWidget {
  const PxEmptyState({super.key, required this.icon, required this.title, required this.text});

  final String icon;
  final String title;
  final String text;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 64),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 384),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 56,
                height: 56,
                margin: const EdgeInsets.only(bottom: 8),
                decoration: BoxDecoration(color: p.surface2, shape: BoxShape.circle),
                child: IconTheme(data: IconThemeData(color: p.muted), child: Center(child: PxIcon(icon, size: 26))),
              ),
              const SizedBox(height: 8),
              Text(title, style: PxText.subtitle(p.fg), textAlign: TextAlign.center),
              const SizedBox(height: 8),
              Text(text, style: PxText.body(p.muted), textAlign: TextAlign.center),
            ],
          ),
        ),
      ),
    );
  }
}

/// `.notice`: a note with an accent bar on its left edge.
class PxNotice extends StatelessWidget {
  const PxNotice({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: p.surface,
        borderRadius: BorderRadius.circular(rMd),
        border: Border(left: BorderSide(color: p.accent, width: 3), top: BorderSide(color: p.rule), right: BorderSide(color: p.rule), bottom: BorderSide(color: p.rule)),
      ),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: children),
    );
  }
}

/// An underlined, bold link inside a sentence (`.btn-link`).
class PxLink extends StatelessWidget {
  const PxLink(this.label, {super.key, required this.onTap, this.color});

  final String label;
  final VoidCallback onTap;
  final Color? color;

  @override
  Widget build(BuildContext context) => PxPress(
    onTap: onTap,
    builder: (context, _, _) => Text(label, style: PxText.body(color ?? context.px.fg).copyWith(fontWeight: FontWeight.w600, decoration: TextDecoration.underline)),
  );
}

/// The top of a page: a poster-sized title over a hairline, a quiet line under it, the page's actions on the right and
/// tabs or filters below.
class PxPageHeader extends StatelessWidget {
  const PxPageHeader({super.key, required this.title, this.meta, this.lead, this.actions = const [], this.below});

  final String title;
  final String? meta;
  final Widget? lead;
  final List<Widget> actions;
  final Widget? below;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final width = MediaQuery.sizeOf(context).width;
    return Container(
      padding: EdgeInsets.only(top: 16, bottom: below == null ? 20 : 0),
      margin: const EdgeInsets.only(bottom: 8),
      decoration: below == null ? BoxDecoration(border: Border(bottom: BorderSide(color: p.rule))) : null,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          LayoutBuilder(
            builder: (context, box) {
              final titleBlock = Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(title, style: PxText.pageTitle(width, p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
                  if (meta != null) Text(meta!, style: PxText.body(p.muted)),
                ],
              );
              final actionRow = Wrap(spacing: 8, runSpacing: 8, crossAxisAlignment: WrapCrossAlignment.center, children: actions);
              // Wide enough: the title takes the row and the page's actions sit at the far right (`.page-head-main`
              // grows, `.page-head-actions` stays at its width). Narrow: they wrap under the title.
              if (box.hasBoundedWidth && box.maxWidth >= 640) {
                return Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    if (lead != null) ...[lead!, const SizedBox(width: 16)],
                    Expanded(child: titleBlock),
                    if (actions.isNotEmpty) ...[const SizedBox(width: 16), actionRow],
                  ],
                );
              }
              return Wrap(
                crossAxisAlignment: WrapCrossAlignment.end,
                spacing: 16,
                runSpacing: 8,
                children: [
                  ?lead,
                  ConstrainedBox(constraints: BoxConstraints(minWidth: 192, maxWidth: box.maxWidth), child: titleBlock),
                  if (actions.isNotEmpty) actionRow,
                ],
              );
            },
          ),
          if (below != null) ...[const SizedBox(height: 8), SizedBox(width: double.infinity, child: below!)],
        ],
      ),
    );
  }
}

/// A heading inside a page: sentence case, an optional count and the section's own actions on the right, then a
/// hairline that runs to the edge.
class PxSectionHead extends StatelessWidget {
  const PxSectionHead(this.title, {super.key, this.count, this.actions = const [], this.first = false});

  final String title;
  final Object? count;
  final List<Widget> actions;
  final bool first;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Padding(
      padding: EdgeInsets.only(top: first ? 0 : 32, bottom: 8),
      child: Row(
        children: [
          Flexible(child: Text(title, style: PxText.lead(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis)),
          if (count != null) ...[
            const SizedBox(width: 12),
            Text('$count', style: PxText.caption(p.muted).copyWith(fontFeatures: const [FontFeature.tabularFigures()])),
          ],
          const SizedBox(width: 12),
          Expanded(child: Container(height: 1, color: p.rule)),
          if (actions.isNotEmpty) ...[
            const SizedBox(width: 12),
            ...actions,
          ],
        ],
      ),
    );
  }
}

/// Marks a widget that runs out to the page's edges instead of sitting inside its margin (a cover shelf).
abstract interface class PxFullBleed {}

/// A page whose header stays put while only the body scrolls (`ScrollPage`).
class PxScrollPage extends StatelessWidget {
  const PxScrollPage({super.key, required this.header, required this.children, this.controller});

  final Widget? header;
  final List<Widget> children;
  final ScrollController? controller;

  @override
  Widget build(BuildContext context) {
    final pad = pagePad(MediaQuery.sizeOf(context).width);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (header != null) Padding(padding: EdgeInsets.symmetric(horizontal: pad), child: header),
        Expanded(
          child: ListView(
            controller: controller,
            padding: const EdgeInsets.only(bottom: 32),
            children: [
              for (final c in children)
                if (c is PxFullBleed) c else Padding(padding: EdgeInsets.symmetric(horizontal: pad), child: c),
            ],
          ),
        ),
      ],
    );
  }
}

/// The three bars of the song that is playing in a list: different heights, standing still.
class PxPlayingMark extends StatelessWidget {
  const PxPlayingMark({super.key, required this.playing});

  final bool playing;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final heights = playing ? const [1.0, 0.55, 0.8] : const [0.4, 0.4, 0.4];
    return SizedBox(
      height: 14,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          for (var i = 0; i < 3; i++) ...[
            if (i > 0) const SizedBox(width: 2),
            AnimatedContainer(
              duration: const Duration(milliseconds: 200),
              width: 3,
              height: 14 * heights[i],
              decoration: BoxDecoration(color: p.accent, borderRadius: BorderRadius.circular(1)),
            ),
          ],
        ],
      ),
    );
  }
}

/// Shows the data, a skeleton while it loads, or the error with a way to try again.
class PxAsync<T> extends StatelessWidget {
  const PxAsync({super.key, required this.value, required this.onRetry, required this.builder, this.skeletonRows = 6});

  final AsyncValue<T> value;
  final VoidCallback onRetry;
  final Widget Function(T data) builder;
  final int skeletonRows;

  @override
  Widget build(BuildContext context) => value.when(
    data: builder,
    loading: () => PxSkeleton(rows: skeletonRows),
    error: (e, _) => PxNotice(
      children: [
        Row(
          children: [
            Flexible(child: Text("Couldn't load this. ${errorText(e)}", style: PxText.body(context.px.fg))),
            const SizedBox(width: 8),
            PxLink('Try again', onTap: onRetry),
          ],
        ),
      ],
    ),
  );
}

/// Drag support for a desktop mouse on lists inside a scroll view (the default drag devices leave the mouse out).
class PxScrollBehavior extends MaterialScrollBehavior {
  const PxScrollBehavior();

  @override
  Set<PointerDeviceKind> get dragDevices => {PointerDeviceKind.touch, PointerDeviceKind.mouse, PointerDeviceKind.trackpad, PointerDeviceKind.stylus};
}

/// Whether the platform draws the window's own frame (set from `main`): used by the shell to decide on a title bar.
bool get ownTitleBar => !featureOff('chrome');
