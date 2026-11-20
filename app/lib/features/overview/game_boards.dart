import 'package:flutter/material.dart';

import '../../bridge/wishtory_api.dart';
import '../../domain/game_info.dart';
import '../../ui/density.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets.dart';

// Each game gets its own layout for the Overview, so switching games feels like switching games:
//   Genshin Impact     warm and rounded: gold-edged panels with diamond ornaments, a pity ring per banner.
//   Honkai: Star Rail  a HUD and a route map: bevelled corners, numbered stations joined by a line.
//   Zenless Zone Zero  loud and slanted: skewed blocks, huge numerals, the open channel inverted.
// What they show is identical; only the arrangement and the shapes differ.

/// The small header over the live banners, in the game's own words.
String liveLabelFor(GameId game) => switch (game) {
      GameId.genshin => 'WISHING NOW',
      GameId.starRail => 'WARPS ONLINE',
      GameId.zenless => 'ON AIR',
    };

/// The label over the banner cards, in the game's own words.
String bannersLabelFor(GameId game) => switch (game) {
      GameId.genshin => 'Wish banners',
      GameId.starRail => 'Warp route',
      GameId.zenless => 'Signal channels',
    };

/// How far the Zenless layouts lean.
const double _kSkew = -0.18;

const _bevel = BorderRadius.only(topLeft: Radius.circular(14), bottomRight: Radius.circular(14));

bool _inSoft(BannerPity p) => p.softPity != null && p.pity >= p.softPity!;

String _bannerLabel(BannerPity p) => '${p.label}: ${p.pity} of ${p.hardPity} pulls${_inSoft(p) ? ', soft pity reached' : ''}';

Duration _motion(BuildContext context, int ms) => MediaQuery.of(context).disableAnimations ? Duration.zero : Duration(milliseconds: ms);

List<List<T>> _rowsOf<T>(List<T> items, int perRow) => [
      for (var i = 0; i < items.length; i += perRow) items.sublist(i, i + perRow < items.length ? i + perRow : items.length),
    ];

/// The chips under a banner's pity: guaranteed or the odds, and soft pity. [ink] paints them all one colour, for
/// a card whose background is the accent.
Widget _bannerChips(BuildContext context, BannerPity p, GameId game, {Color? ink, WrapAlignment alignment = WrapAlignment.start}) {
  final c = context.wc;
  final info = GameInfo.of(game);
  final odds = p.guaranteeOdds;
  return Wrap(alignment: alignment, spacing: 6, runSpacing: 6, children: [
    if (odds != null && p.guaranteed == true) StatusChip('Guaranteed ${info.r5}', color: ink ?? c.blue, icon: Icons.verified_outlined),
    if (odds != null && p.guaranteed == false) StatusChip('$odds/${100 - odds}', color: ink ?? c.muted, icon: Icons.casino_outlined),
    if (_inSoft(p)) StatusChip('Soft pity', color: ink ?? c.accent, icon: Icons.trending_up),
    if (p.guaranteed == null && !_inSoft(p)) Text('${p.fiveStars} ${info.fives}', style: Ty.small.copyWith(color: ink ?? c.faint)),
  ]);
}

/// A small four-sided ornament, a square turned on its corner.
class _Diamond extends StatelessWidget {
  const _Diamond({required this.color, this.size = 8, this.filled = true});

  final Color color;
  final double size;
  final bool filled;

  @override
  Widget build(BuildContext context) => ExcludeSemantics(
        child: Transform.rotate(
          angle: 0.7853981633974483,
          child: SizedBox(
            width: size,
            height: size,
            child: DecoratedBox(
              decoration: BoxDecoration(color: filled ? color : null, border: Border.all(color: color, width: 1.2)),
            ),
          ),
        ),
      );
}

// ------------------------------------------------------------------ status strip

/// One figure of the status strip.
class StatData {
  const StatData(this.label, this.value, {this.sub, this.text = false});

  final String label;
  final String value;
  final String? sub;

  /// A name rather than a number: set in the heading face, not the numeric one.
  final bool text;
}

/// The four headline figures of a game, laid out the way that game looks.
class StatStrip extends StatelessWidget {
  const StatStrip({super.key, required this.game, required this.stats});

  final GameId game;
  final List<StatData> stats;

  @override
  Widget build(BuildContext context) => LayoutBuilder(builder: (context, box) {
        final perRow = box.maxWidth >= 900 ? 4 : 2;
        return switch (game) {
          GameId.genshin => _GenshinStats(stats: stats, perRow: perRow),
          GameId.starRail => _HsrStats(stats: stats, perRow: perRow),
          GameId.zenless => _ZzzStats(stats: stats, perRow: perRow),
        };
      });
}

class _Figure extends StatelessWidget {
  const _Figure(this.s, {this.align = CrossAxisAlignment.start, this.heading, this.valueColor, this.subColor, this.size = 26});

  final StatData s;
  final CrossAxisAlignment align;

  /// Replaces the label when a layout numbers or decorates it.
  final String? heading;
  final Color? valueColor;
  final Color? subColor;
  final double size;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final centered = align == CrossAxisAlignment.center;
    final textAlign = centered ? TextAlign.center : TextAlign.start;
    return Column(crossAxisAlignment: align, mainAxisSize: MainAxisSize.min, children: [
      Text(heading ?? s.label, textAlign: textAlign, maxLines: 1, overflow: TextOverflow.ellipsis, style: Ty.small.copyWith(color: valueColor == null ? c.muted : valueColor!.withValues(alpha: 0.8))),
      const SizedBox(height: 4),
      Text(
        s.value,
        textAlign: textAlign,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: (s.text ? Ty.h1 : Ty.bigNum.copyWith(fontSize: size)).copyWith(color: valueColor),
      ),
      if (s.sub != null) Text(s.sub!, textAlign: textAlign, maxLines: 1, overflow: TextOverflow.ellipsis, style: Ty.small.copyWith(color: subColor ?? c.faint)),
    ]);
  }
}

/// Genshin: one gold-edged panel, the figures centred and parted by diamonds.
class _GenshinStats extends StatelessWidget {
  const _GenshinStats({required this.stats, required this.perRow});

  final List<StatData> stats;
  final int perRow;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final gold = c.gold;
    final rows = _rowsOf(stats, perRow);
    return PopIn(
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 16, horizontal: 10),
        decoration: BoxDecoration(
          color: c.panel,
          borderRadius: BorderRadius.circular(18),
          border: Border.all(color: gold.withValues(alpha: 0.45)),
          gradient: LinearGradient(begin: Alignment.topCenter, end: Alignment.bottomCenter, colors: [gold.withValues(alpha: 0.08), c.panel]),
        ),
        child: Column(children: [
          for (var r = 0; r < rows.length; r++) ...[
            if (r > 0) const SizedBox(height: 14),
            Row(children: [
              for (var i = 0; i < rows[r].length; i++) ...[
                if (i > 0) _Diamond(color: gold.withValues(alpha: 0.75), size: 7),
                Expanded(child: _Figure(rows[r][i], align: CrossAxisAlignment.center)),
              ],
            ]),
          ],
        ]),
      ),
    );
  }
}

/// Star Rail: a HUD. Bevelled corners, numbered cells divided by hairlines.
class _HsrStats extends StatelessWidget {
  const _HsrStats({required this.stats, required this.perRow});

  final List<StatData> stats;
  final int perRow;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final rows = _rowsOf(stats, perRow);
    var n = 0;
    return PopIn(
      child: DecoratedBox(
        decoration: ShapeDecoration(
          color: c.panel,
          shape: BeveledRectangleBorder(side: BorderSide(color: c.line2), borderRadius: const BorderRadius.only(topLeft: Radius.circular(20), bottomRight: Radius.circular(20))),
        ),
        child: Column(children: [
          for (var r = 0; r < rows.length; r++) ...[
            if (r > 0) Divider(height: 1, thickness: 1, color: c.line),
            IntrinsicHeight(
              child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                for (var i = 0; i < rows[r].length; i++) ...[
                  if (i > 0) VerticalDivider(width: 1, thickness: 1, color: c.line),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(16, 12, 12, 12),
                      child: _Figure(rows[r][i], heading: '${(++n).toString().padLeft(2, '0')}  ${rows[r][i].label.toUpperCase()}', size: 28),
                    ),
                  ),
                ],
              ]),
            ),
          ],
        ]),
      ),
    );
  }
}

/// Zenless: slanted blocks, the first one solid accent, the rest dark with a bright edge.
class _ZzzStats extends StatelessWidget {
  const _ZzzStats({required this.stats, required this.perRow});

  final List<StatData> stats;
  final int perRow;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final rows = _rowsOf(stats, perRow);
    var n = 0;
    return PopIn(
      child: Padding(
        // The lean pushes the blocks sideways by half their height; this keeps them inside the page.
        padding: const EdgeInsets.symmetric(horizontal: 14),
        child: Transform(
          alignment: Alignment.center,
          transform: Matrix4.skewX(_kSkew),
          child: Column(children: [
            for (var r = 0; r < rows.length; r++) ...[
              if (r > 0) const SizedBox(height: 6),
              IntrinsicHeight(
                child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                  for (var i = 0; i < rows[r].length; i++) ...[
                    if (i > 0) const SizedBox(width: 6),
                    Expanded(child: _zzzBlock(c, rows[r][i], solid: n++ == 0)),
                  ],
                ]),
              ),
            ],
          ]),
        ),
      ),
    );
  }

  Widget _zzzBlock(WishColors c, StatData s, {required bool solid}) {
    final ink = solid ? c.accentInk : null;
    return Container(
      padding: const EdgeInsets.fromLTRB(16, 10, 12, 10),
      decoration: BoxDecoration(
        color: solid ? c.accent : c.panel2,
        border: Border(left: BorderSide(color: solid ? c.accentInk : c.accent, width: 5)),
      ),
      child: _Figure(s, valueColor: ink, subColor: ink?.withValues(alpha: 0.75), size: 32),
    );
  }
}

// ------------------------------------------------------------------ banner board

/// The banners of a game with their pity, laid out the way that game looks. Tapping one calls [onSelect].
class BannerBoard extends StatelessWidget {
  const BannerBoard({super.key, required this.game, required this.list, required this.selected, required this.onSelect});

  final GameId game;
  final List<BannerPity> list;

  /// The key of the banner whose detail is open.
  final String selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) => switch (game) {
        GameId.genshin => _GenshinBoard(game: game, list: list, selected: selected, onSelect: onSelect),
        GameId.starRail => _HsrBoard(game: game, list: list, selected: selected, onSelect: onSelect),
        GameId.zenless => _ZzzBoard(game: game, list: list, selected: selected, onSelect: onSelect),
      };
}

/// Genshin: a grid of cards, each with its pity in a ring that fills as it climbs and turns gold in soft pity.
class _GenshinBoard extends StatelessWidget {
  const _GenshinBoard({required this.game, required this.list, required this.selected, required this.onSelect});

  final GameId game;
  final List<BannerPity> list;
  final String selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) => LayoutBuilder(builder: (context, box) {
        final gap = Density.of(context).gap;
        // As many cards per row as fit at a sensible width, balanced so no row ends with a lone card.
        var fit = ((box.maxWidth + gap) / (240 + gap)).floor();
        if (fit < 1) fit = 1;
        if (fit > 6) fit = 6;
        final rows = (list.length / fit).ceil();
        final cols = (list.length / rows).ceil();
        final w = (box.maxWidth - (cols - 1) * gap) / cols;
        return Wrap(spacing: gap, runSpacing: gap, alignment: WrapAlignment.center, children: [
          for (final (i, p) in list.indexed)
            PopIn(
              index: i,
              child: SizedBox(width: w, child: _GenshinCard(p: p, game: game, selected: p.banner == selected, onTap: () => onSelect(p.banner))),
            ),
        ]);
      });
}

class _GenshinCard extends StatelessWidget {
  const _GenshinCard({required this.p, required this.game, required this.selected, required this.onTap});

  final BannerPity p;
  final GameId game;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final hard = p.hardPity <= 0 ? 1 : p.hardPity;
    final frac = (p.pity / hard).clamp(0.0, 1.0);
    final ring = _inSoft(p) ? c.gold : c.accent;
    return Semantics(
      button: true,
      selected: selected,
      label: _bannerLabel(p),
      child: Panel(
        selected: selected,
        onTap: onTap,
        padding: const EdgeInsets.fromLTRB(14, 14, 14, 16),
        child: ExcludeSemantics(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Row(mainAxisAlignment: MainAxisAlignment.center, children: [
              _Diamond(color: ring, size: 6),
              const SizedBox(width: 8),
              Flexible(child: Text(p.label, style: Ty.h3, maxLines: 1, overflow: TextOverflow.ellipsis, textAlign: TextAlign.center)),
              const SizedBox(width: 8),
              _Diamond(color: ring, size: 6),
            ]),
            const SizedBox(height: 12),
            SizedBox(
              width: 104,
              height: 104,
              child: Stack(alignment: Alignment.center, children: [
                TweenAnimationBuilder<double>(
                  tween: Tween<double>(begin: 0, end: frac),
                  duration: _motion(context, 700),
                  curve: Curves.easeOutCubic,
                  builder: (context, v, _) => SizedBox(
                    width: 104,
                    height: 104,
                    child: CircularProgressIndicator(value: v, strokeWidth: 8, backgroundColor: c.panel3, color: ring),
                  ),
                ),
                Column(mainAxisSize: MainAxisSize.min, children: [
                  Text('${p.pity}', style: Ty.bigNum.copyWith(fontSize: 30)),
                  Text('/ ${p.hardPity}', style: Ty.small.copyWith(color: c.faint)),
                ]),
              ]),
            ),
            const SizedBox(height: 10),
            Text('${groupDigits(p.total)} pulls', style: Ty.small.copyWith(color: c.faint)),
            const SizedBox(height: 8),
            _bannerChips(context, p, game, alignment: WrapAlignment.center),
          ]),
        ),
      ),
    );
  }
}

/// Star Rail: a route map. The banners are stations on one line, each a bevelled card.
class _HsrBoard extends StatelessWidget {
  const _HsrBoard({required this.game, required this.list, required this.selected, required this.onSelect});

  final GameId game;
  final List<BannerPity> list;
  final String selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final gap = Density.of(context).gap;
    return Column(children: [
      for (final (i, p) in list.indexed)
        PopIn(
          index: i,
          child: IntrinsicHeight(
            child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              SizedBox(
                width: 34,
                child: Column(children: [
                  const SizedBox(height: 20),
                  _station(c, p.banner == selected),
                  if (i < list.length - 1) Expanded(child: Center(child: SizedBox(width: 2, child: ColoredBox(color: c.line2)))) else const Spacer(),
                ]),
              ),
              Expanded(
                child: Padding(
                  padding: EdgeInsets.only(bottom: i < list.length - 1 ? gap : 0),
                  child: _HsrCard(p: p, game: game, index: i, selected: p.banner == selected, onTap: () => onSelect(p.banner)),
                ),
              ),
            ]),
          ),
        ),
    ]);
  }

  Widget _station(WishColors c, bool on) => ExcludeSemantics(
        child: Container(
          width: 16,
          height: 16,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: on ? c.accent : c.bg,
            border: Border.all(color: on ? c.accent : c.control, width: 2),
            boxShadow: on ? [BoxShadow(color: c.accent.withValues(alpha: 0.55), blurRadius: 10)] : null,
          ),
        ),
      );
}

class _HsrCard extends StatelessWidget {
  const _HsrCard({required this.p, required this.game, required this.index, required this.selected, required this.onTap});

  final BannerPity p;
  final GameId game;
  final int index;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final shape = BeveledRectangleBorder(
      side: BorderSide(color: selected ? c.accent : c.line2, width: selected ? 1.5 : 1),
      borderRadius: _bevel,
    );
    return Semantics(
      button: true,
      selected: selected,
      label: _bannerLabel(p),
      child: Material(
        color: selected ? Color.alphaBlend(c.accent.withValues(alpha: 0.09), c.panel) : c.panel,
        shape: shape,
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onTap,
          customBorder: shape,
          child: ExcludeSemantics(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 14),
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text('STATION ${(index + 1).toString().padLeft(2, '0')}', style: Ty.label.copyWith(color: selected ? c.accent : c.faint)),
                const SizedBox(height: 4),
                Row(crossAxisAlignment: CrossAxisAlignment.baseline, textBaseline: TextBaseline.alphabetic, children: [
                  Expanded(child: Text(p.label, style: Ty.h2, maxLines: 1, overflow: TextOverflow.ellipsis)),
                  Text('${p.pity}', style: Ty.bigNum.copyWith(fontSize: 26)),
                  Text(' / ${p.hardPity}', style: Ty.num.copyWith(color: c.faint)),
                ]),
                const SizedBox(height: 8),
                PityBar(value: p.pity, hard: p.hardPity, soft: p.softPity),
                const SizedBox(height: 10),
                Row(children: [
                  Expanded(child: _bannerChips(context, p, game)),
                  Text('${groupDigits(p.total)} pulls', style: Ty.small.copyWith(color: c.faint)),
                ]),
              ]),
            ),
          ),
        ),
      ),
    );
  }
}

/// Zenless: full-width slanted channels with a huge pity number; the open one is inverted.
class _ZzzBoard extends StatelessWidget {
  const _ZzzBoard({required this.game, required this.list, required this.selected, required this.onSelect});

  final GameId game;
  final List<BannerPity> list;
  final String selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) => Padding(
        // The lean pushes each row sideways by half its height; this keeps them inside the page.
        padding: const EdgeInsets.symmetric(horizontal: 18),
        child: Transform(
          alignment: Alignment.center,
          transform: Matrix4.skewX(_kSkew),
          child: Column(children: [
            for (final (i, p) in list.indexed) ...[
              if (i > 0) const SizedBox(height: 8),
              PopIn(index: i, child: _ZzzRow(p: p, game: game, selected: p.banner == selected, onTap: () => onSelect(p.banner))),
            ],
          ]),
        ),
      );
}

class _ZzzRow extends StatelessWidget {
  const _ZzzRow({required this.p, required this.game, required this.selected, required this.onTap});

  final BannerPity p;
  final GameId game;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final ink = selected ? c.accentInk : null;
    return Semantics(
      button: true,
      selected: selected,
      label: _bannerLabel(p),
      child: Material(
        color: selected ? c.accent : c.panel2,
        child: InkWell(
          onTap: onTap,
          child: ExcludeSemantics(
            child: Container(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
              decoration: BoxDecoration(border: Border(left: BorderSide(color: selected ? c.accentInk : c.accent, width: 6))),
              child: Row(children: [
                SizedBox(
                  width: 110,
                  child: FittedBox(
                    fit: BoxFit.scaleDown,
                    alignment: Alignment.centerLeft,
                    child: Text('${p.pity}', style: Ty.bigNum.copyWith(fontSize: 50, color: ink)),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                    Row(children: [
                      Expanded(child: Text(p.label.toUpperCase(), style: Ty.h3.copyWith(letterSpacing: 0.8, color: ink), maxLines: 1, overflow: TextOverflow.ellipsis)),
                      Text('/ ${p.hardPity}  ·  ${groupDigits(p.total)} pulls', style: Ty.small.copyWith(color: ink ?? c.faint)),
                    ]),
                    const SizedBox(height: 8),
                    PityBar(value: p.pity, hard: p.hardPity, soft: p.softPity, color: selected ? c.accentInk : null),
                    const SizedBox(height: 8),
                    _bannerChips(context, p, game, ink: ink),
                  ]),
                ),
              ]),
            ),
          ),
        ),
      ),
    );
  }
}
