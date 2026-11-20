import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../bridge/wishtory_api.dart';
import '../domain/game_info.dart';
import '../domain/icon_cache.dart';
import '../domain/recovery.dart';
import '../state/providers.dart';
import 'density.dart';
import 'tokens.dart';

/// A bordered surface. Structure comes from hairlines; a card that can be pressed lifts a little and glows in the
/// accent colour when the pointer is over it, and a selected card keeps a softer glow.
class Panel extends StatefulWidget {
  const Panel({super.key, required this.child, this.padding, this.tint, this.onTap, this.selected = false});

  final Widget child;
  /// Defaults to the density setting's card padding.
  final EdgeInsetsGeometry? padding;
  final Color? tint;
  final VoidCallback? onTap;
  final bool selected;

  @override
  State<Panel> createState() => _PanelState();
}

class _PanelState extends State<Panel> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final w = widget;
    final tappable = w.onTap != null;
    final lifted = tappable && _hover;
    final glow = lifted || w.selected;
    final box = AnimatedContainer(
      duration: MediaQuery.of(context).disableAnimations ? Duration.zero : const Duration(milliseconds: 160),
      curve: Curves.easeOut,
      padding: w.padding ?? EdgeInsets.all(Density.of(context).pad),
      decoration: BoxDecoration(
        color: w.tint ?? c.panel,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(
          color: w.selected ? c.accent : (lifted ? c.accent.withValues(alpha: 0.55) : c.line),
          width: w.selected ? 1.5 : 1,
        ),
        boxShadow: glow ? [BoxShadow(color: c.accent.withValues(alpha: w.selected ? 0.16 : 0.12), blurRadius: 16, offset: const Offset(0, 4))] : const [],
      ),
      child: w.child,
    );
    if (!tappable) return box;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(12),
        child: InkWell(borderRadius: BorderRadius.circular(12), onTap: w.onTap, child: box),
      ),
    );
  }
}

/// Slides its child up a few pixels while fading it in, once, when it first appears. [index] staggers siblings so a
/// row of cards arrives one after another. With reduced motion switched on the child simply appears.
class PopIn extends StatelessWidget {
  const PopIn({super.key, required this.child, this.index = 0});

  final Widget child;
  final int index;

  @override
  Widget build(BuildContext context) {
    if (MediaQuery.of(context).disableAnimations) return child;
    final steps = index < 8 ? index : 8;
    return TweenAnimationBuilder<double>(
      tween: Tween<double>(begin: 0, end: 1),
      duration: Duration(milliseconds: 320 + 70 * steps),
      curve: Curves.easeOutCubic,
      child: child,
      builder: (context, t, child) => Opacity(
        opacity: t,
        child: Transform.translate(offset: Offset(0, (1 - t) * 14), child: child),
      ),
    );
  }
}

/// Grows its child a touch while the pointer is over it.
class LiftOnHover extends StatefulWidget {
  const LiftOnHover({super.key, required this.child, this.scale = 1.012});

  final Widget child;
  final double scale;

  @override
  State<LiftOnHover> createState() => _LiftOnHoverState();
}

class _LiftOnHoverState extends State<LiftOnHover> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final still = MediaQuery.of(context).disableAnimations;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: AnimatedScale(
        scale: _hover && !still ? widget.scale : 1,
        duration: still ? Duration.zero : const Duration(milliseconds: 160),
        curve: Curves.easeOut,
        child: widget.child,
      ),
    );
  }
}

/// A dot with a ring that swells and fades around it, for something that is happening now. Still (no ring) when
/// reduced motion is switched on.
class PulseDot extends StatefulWidget {
  const PulseDot({super.key, required this.color, this.size = 8});

  final Color color;
  final double size;

  @override
  State<PulseDot> createState() => _PulseDotState();
}

class _PulseDotState extends State<PulseDot> with SingleTickerProviderStateMixin {
  late final AnimationController _ctl = AnimationController(vsync: this, duration: const Duration(milliseconds: 1600));

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.of(context).disableAnimations) {
      _ctl.stop();
      _ctl.value = 1;
    } else if (!_ctl.isAnimating) {
      _ctl.repeat();
    }
  }

  @override
  void dispose() {
    _ctl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final size = widget.size;
    return ExcludeSemantics(
      child: SizedBox(
        width: size * 3,
        height: size * 3,
        child: AnimatedBuilder(
          animation: _ctl,
          builder: (context, _) {
            final t = _ctl.value;
            final ring = size * (1 + t * 1.8);
            return Center(
              child: Stack(alignment: Alignment.center, clipBehavior: Clip.none, children: [
                SizedBox(
                  width: ring,
                  height: ring,
                  child: DecoratedBox(decoration: BoxDecoration(shape: BoxShape.circle, color: widget.color.withValues(alpha: (1 - t) * 0.45))),
                ),
                SizedBox(width: size, height: size, child: DecoratedBox(decoration: BoxDecoration(shape: BoxShape.circle, color: widget.color))),
              ]),
            );
          },
        ),
      ),
    );
  }
}

class SectionLabel extends StatelessWidget {
  const SectionLabel(this.text, {super.key});
  final String text;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(bottom: 8),
        child: Semantics(
          header: true,
          child: Text(text.toUpperCase(), style: Ty.label.copyWith(color: context.wc.faint)),
        ),
      );
}

/// "5★" / "S-rank" in the rarity colour. The text carries the meaning, the colour only reinforces it.
class RarityChip extends StatelessWidget {
  const RarityChip({super.key, required this.game, required this.rank});
  final GameId game;
  final int rank;

  @override
  Widget build(BuildContext context) {
    final color = context.wc.rarity(rank);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.14),
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: color.withValues(alpha: 0.5)),
        boxShadow: rank >= 5 ? [BoxShadow(color: color.withValues(alpha: 0.35), blurRadius: 8)] : null,
      ),
      child: Text(rankLabel(game, rank), style: Ty.small.copyWith(color: color, fontWeight: FontWeight.w600)),
    );
  }
}

class StatusChip extends StatelessWidget {
  const StatusChip(this.label, {super.key, this.color, this.icon});
  final String label;
  final Color? color;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final c = color ?? context.wc.muted;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: c.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(99),
        border: Border.all(color: c.withValues(alpha: 0.45)),
      ),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        if (icon != null) ...[Icon(icon, size: 13, color: c), const SizedBox(width: 4)],
        Flexible(child: Text(label, style: Ty.small.copyWith(color: c, fontWeight: FontWeight.w600), overflow: TextOverflow.ellipsis)),
      ]),
    );
  }
}

/// Won / Lost / Guaranteed: always text plus an icon, never colour alone.
class OutcomeChip extends StatelessWidget {
  const OutcomeChip({super.key, required this.outcome, this.odds});
  final PullOutcome? outcome;
  final int? odds;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final label = odds == null ? '' : ' $odds/${100 - odds!}';
    return switch (outcome) {
      PullOutcome.win => StatusChip('Won$label', color: c.ok, icon: Icons.check_circle_outline),
      PullOutcome.loss => StatusChip('Lost$label', color: c.err, icon: Icons.highlight_off),
      PullOutcome.guaranteed => StatusChip('Guaranteed', color: c.blue, icon: Icons.verified_outlined),
      null => StatusChip('Result unknown', color: c.faint, icon: Icons.help_outline),
    };
  }
}

/// Item icon from the public Enka host (or Paimon.moe's repository when Enka has none yet), with a rarity-coloured
/// star as the offline/missing fallback.
class ItemIcon extends ConsumerWidget {
  const ItemIcon({super.key, required this.game, required this.name, required this.rank, this.size = 36});

  final GameId game;
  final String name;
  final int rank;
  final double size;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final c = context.wc;
    final color = c.rarity(rank);
    final item = ref.watch(catalogIndexProvider).find(game, name);
    final urls = ref.watch(iconTableProvider).iconUrls(item);
    final fallback = Center(child: Icon(Icons.star_rounded, size: size * 0.55, color: color));
    // The picture comes from the disk cache; until it is there (or when it never will be), the star shows.
    final file = urls.isEmpty
        ? null
        : switch (ref.watch(iconFileProvider(IconRequest(urls)))) {
            AsyncData(:final value) => value,
            _ => null,
          };
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.10),
        shape: BoxShape.circle,
        border: Border.all(color: color.withValues(alpha: 0.6)),
      ),
      clipBehavior: Clip.antiAlias,
      child: file == null
          ? fallback
          : Image.file(
              file,
              fit: BoxFit.cover,
              cacheWidth: (size * MediaQuery.devicePixelRatioOf(context)).round(),
              gaplessPlayback: true,
              excludeFromSemantics: true,
              errorBuilder: (_, _, _) => fallback,
            ),
    );
  }
}

/// A horizontal bar of [value] out of [hard], with the soft-pity zone marked and a text label for
/// assistive technology.
class PityBar extends StatelessWidget {
  const PityBar({super.key, required this.value, required this.hard, this.soft, this.color, this.height = 8, this.label});

  final int value;
  final int hard;
  final int? soft;
  final Color? color;
  final double height;
  final String? label;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final inSoft = soft != null && value >= soft!;
    final fill = color ?? (inSoft ? c.blue : c.accent);
    final h = hard <= 0 ? 1 : hard;
    final frac = (value / h).clamp(0.0, 1.0);
    return Semantics(
      label: label ?? '$value of $hard pulls${inSoft ? ', soft pity reached' : ''}',
      child: ExcludeSemantics(
        child: LayoutBuilder(builder: (context, box) {
          final w = box.maxWidth;
          return SizedBox(
            height: height,
            child: Stack(clipBehavior: Clip.none, children: [
              Container(decoration: BoxDecoration(color: c.panel3, borderRadius: BorderRadius.circular(height))),
              if (soft != null)
                Positioned(
                  left: w * (soft! / h).clamp(0.0, 1.0),
                  right: 0,
                  top: 0,
                  bottom: 0,
                  child: Container(
                    decoration: BoxDecoration(
                      color: c.blue.withValues(alpha: 0.16),
                      borderRadius: BorderRadius.horizontal(right: Radius.circular(height)),
                    ),
                  ),
                ),
              AnimatedContainer(
                duration: MediaQuery.of(context).disableAnimations ? Duration.zero : const Duration(milliseconds: 250),
                width: w * frac,
                decoration: BoxDecoration(
                  gradient: LinearGradient(colors: [fill, Color.alphaBlend(Colors.white.withValues(alpha: 0.28), fill)]),
                  borderRadius: BorderRadius.circular(height),
                  boxShadow: inSoft ? [BoxShadow(color: fill.withValues(alpha: 0.5), blurRadius: 8)] : null,
                ),
              ),
            ]),
          );
        }),
      ),
    );
  }
}

class EmptyState extends StatelessWidget {
  const EmptyState({super.key, required this.icon, required this.title, required this.body, this.actions = const []});

  final IconData icon;
  final String title;
  final String body;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460),
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Container(
              width: 76,
              height: 76,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                gradient: LinearGradient(
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                  colors: [c.accent.withValues(alpha: 0.22), c.gold.withValues(alpha: 0.12)],
                ),
                border: Border.all(color: c.accent.withValues(alpha: 0.35)),
              ),
              child: Icon(icon, size: 34, color: c.accent),
            ),
            const SizedBox(height: 16),
            Text(title, style: Ty.h2, textAlign: TextAlign.center),
            const SizedBox(height: 6),
            Text(body, style: Ty.body.copyWith(color: c.muted), textAlign: TextAlign.center),
            if (actions.isNotEmpty) ...[
              const SizedBox(height: 18),
              Wrap(spacing: 8, runSpacing: 8, alignment: WrapAlignment.center, children: actions),
            ],
          ]),
        ),
      ),
    );
  }
}

/// A placeholder block while data loads (never a spinner over empty space).
class Skeleton extends StatelessWidget {
  const Skeleton({super.key, this.width, this.height = 16, this.radius = 6});
  final double? width;
  final double height;
  final double radius;

  @override
  Widget build(BuildContext context) => Container(
        width: width,
        height: height,
        decoration: BoxDecoration(color: context.wc.panel2, borderRadius: BorderRadius.circular(radius)),
      );
}

/// What to do when something failed: why, numbered steps, and the buttons that help.
class RecoveryCard extends StatelessWidget {
  const RecoveryCard({super.key, required this.recovery, this.onRetry, this.onHelp, this.onLocations, this.onDismiss});

  final Recovery recovery;
  final VoidCallback? onRetry;
  final void Function(String topic)? onHelp;
  final VoidCallback? onLocations;
  final VoidCallback? onDismiss;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    return Semantics(
      liveRegion: true,
      child: Panel(
        tint: c.err.withValues(alpha: 0.07),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Icon(Icons.error_outline, color: c.err, size: 20),
            const SizedBox(width: 8),
            Expanded(child: Text(recovery.title, style: Ty.h2)),
            if (onDismiss != null) IconButton(tooltip: 'Dismiss', onPressed: onDismiss, icon: const Icon(Icons.close, size: 18)),
          ]),
          const SizedBox(height: 4),
          Text(recovery.why, style: Ty.body.copyWith(color: c.muted)),
          if (recovery.steps.isNotEmpty) ...[
            const SizedBox(height: 10),
            for (var i = 0; i < recovery.steps.length; i++)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  SizedBox(width: 22, child: Text('${i + 1}.', style: Ty.num.copyWith(color: c.faint))),
                  Expanded(child: Text(recovery.steps[i], style: Ty.body)),
                ]),
              ),
          ],
          const SizedBox(height: 12),
          Wrap(spacing: 8, runSpacing: 8, children: [
            if (recovery.canRetry && onRetry != null) FilledButton.icon(onPressed: onRetry, icon: const Icon(Icons.refresh, size: 18), label: const Text('Try again')),
            if (recovery.offerGameLocations && onLocations != null) OutlinedButton(onPressed: onLocations, child: const Text('Game locations')),
            if (recovery.helpTopic != null && onHelp != null) TextButton(onPressed: () => onHelp!(recovery.helpTopic!), child: const Text('Open help')),
          ]),
          const SizedBox(height: 8),
          SelectableText(recovery.detail, style: Ty.small.copyWith(color: c.faint)),
        ]),
      ),
    );
  }
}

InputDecoration fieldDecoration(BuildContext context, {String? hint, Widget? prefix, Widget? suffix}) {
  final c = context.wc;
  OutlineInputBorder border(Color color) => OutlineInputBorder(borderRadius: BorderRadius.circular(8), borderSide: BorderSide(color: color));
  return InputDecoration(
    isDense: true,
    hintText: hint,
    hintStyle: TextStyle(color: c.faint),
    prefixIcon: prefix,
    suffixIcon: suffix,
    filled: true,
    fillColor: c.panel2,
    contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
    enabledBorder: border(c.control),
    focusedBorder: border(c.accent),
    border: border(c.control),
  );
}

Future<bool> confirm(
  BuildContext context, {
  required String title,
  required String body,
  required String action,
  bool danger = false,
}) async {
  final c = context.wc;
  final ok = await showDialog<bool>(
    context: context,
    builder: (ctx) => AlertDialog(
      backgroundColor: c.panel,
      title: Text(title, style: Ty.h2),
      content: ConstrainedBox(constraints: const BoxConstraints(maxWidth: 440), child: Text(body, style: Ty.body)),
      actions: [
        TextButton(autofocus: true, onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
        FilledButton(
          style: danger ? FilledButton.styleFrom(backgroundColor: c.err, foregroundColor: c.onErr) : null,
          onPressed: () => Navigator.pop(ctx, true),
          child: Text(action),
        ),
      ],
    ),
  );
  return ok ?? false;
}

/// A page's scrolling body with the standard gutter and a readable maximum width.
class PageBody extends StatelessWidget {
  const PageBody({super.key, required this.children, this.maxWidth = 1200});
  final List<Widget> children;
  final double maxWidth;

  @override
  Widget build(BuildContext context) => Scrollbar(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 32),
          children: [
            Center(
              child: ConstrainedBox(
                constraints: BoxConstraints(maxWidth: maxWidth),
                child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: children),
              ),
            ),
          ],
        ),
      );
}
