import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/wishtory_api.dart';
import '../../domain/planning.dart';
import '../../state/providers.dart';
import '../../ui/banner_art.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets.dart';
import 'game_boards.dart';

/// The banners running right now, large and centered at the top of the Overview, each on the artwork of
/// what it features. A character banner that features two characters is two cards, one per character
/// (the game runs them as two banners that share one pity); the weapon banner is one card showing both
/// featured weapons. Tapping a card opens that banner's pity below.
class LiveHero extends ConsumerWidget {
  const LiveHero({super.key, required this.account, required this.pity, required this.onSelect});

  final Account account;
  final List<BannerPity> pity;
  final ValueChanged<String> onSelect;

  /// Character banners first, then weapon banners, then the rest; soonest ending first within each.
  static int _order(String pool) => switch (pool) {
        'character' => 0,
        'weapon' => 1,
        _ => 2,
      };

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final banners = ref.watch(bannersProvider).data;
    final catalog = ref.watch(catalogProvider).data;
    final live = liveBanners(banners, account.game, account.timezone, catalog);
    if (live.isEmpty) return const SizedBox.shrink();
    final sorted = [...live]..sort((a, b) {
        final byPool = _order(a.pool).compareTo(_order(b.pool));
        return byPool != 0 ? byPool : a.msLeft.compareTo(b.msLeft);
      });
    // Split after sorting, so the cards of one banner stay together and in the order the game lists them.
    final cards = liveCards(sorted);
    final c = context.wc;
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Column(children: [
        Semantics(
          header: true,
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            PulseDot(color: c.ok),
            const SizedBox(width: 2),
            Text(liveLabelFor(account.game), style: Ty.label.copyWith(color: c.muted)),
          ]),
        ),
        const SizedBox(height: 12),
        LayoutBuilder(builder: (context, box) {
          const gap = 16.0;
          const minCard = 300.0;
          final count = cards.length;
          // As many cards per row as fit at a sensible width, then balanced so no row ends with a lone card.
          var fit = ((box.maxWidth + gap) / (minCard + gap)).floor();
          if (fit < 1) fit = 1;
          if (fit > count) fit = count;
          final rows = (count / fit).ceil();
          final cols = (count / rows).ceil();
          var width = (box.maxWidth - (cols - 1) * gap) / cols;
          final cap = count == 1 ? 960.0 : 640.0;
          if (width > cap) width = cap;
          return Wrap(
            alignment: WrapAlignment.center,
            spacing: gap,
            runSpacing: gap,
            children: [
              for (final (i, card) in cards.indexed)
                PopIn(
                  index: i,
                  child: SizedBox(
                    width: width,
                    child: LiftOnHover(
                      child: _LiveCardView(
                        game: account.game,
                        card: card,
                        pity: pity.where((p) => p.banner == card.banner.pool).firstOrNull,
                        onSelect: onSelect,
                      ),
                    ),
                  ),
                ),
            ],
          );
        }),
      ]),
    );
  }
}

class _LiveCardView extends StatelessWidget {
  const _LiveCardView({required this.game, required this.card, required this.pity, required this.onSelect});

  final GameId game;
  final LiveCard card;
  final BannerPity? pity;
  final ValueChanged<String> onSelect;

  LiveBanner get _banner => card.banner;

  String get _kind => switch (_banner.pool) {
        'character' => game == GameId.starRail ? 'Character warp' : 'Character event',
        'weapon' => game == GameId.starRail ? 'Light cone warp' : 'Weapon event',
        final other => other.isEmpty ? 'Event' : '${other[0].toUpperCase()}${other.substring(1)}',
      };

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final p = pity;
    final words = '${card.title}. $_kind. ${card.featured.join(', ')}. ${timeLeft(_banner.msLeft)}.${p == null ? '' : ' Your pity ${p.pity} of ${p.hardPity}.'}';
    final face = Container(
          height: 250,
          decoration: _frame(c),
          clipBehavior: Clip.antiAlias,
          child: Material(
            color: Colors.transparent,
            child: InkWell(
              onTap: p == null ? null : () => onSelect(_banner.pool),
              child: LayoutBuilder(builder: (context, box) {
                // A narrower card gives the artwork a larger share and the title a smaller size.
                final compact = box.maxWidth < 380;
                final artLeft = (box.maxWidth * 0.34).clamp(110.0, 230.0);
                return Stack(children: [
                  // A tinted base, so the card is never empty while its artwork downloads.
                  Positioned.fill(
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        gradient: LinearGradient(
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                          colors: [c.panel2, Color.alphaBlend(c.accent.withValues(alpha: 0.22), c.panel)],
                        ),
                      ),
                    ),
                  ),
                  // The artwork sits in the right part; the text has the left.
                  Positioned.fill(left: artLeft, child: BannerArt(game: game, names: card.featured)),
                  // A fade from the card colour keeps the text readable over any picture.
                  Positioned.fill(
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        gradient: LinearGradient(
                          begin: Alignment.centerLeft,
                          end: Alignment.centerRight,
                          stops: const [0, 0.42, 0.78],
                          colors: [c.panel.withValues(alpha: 0.97), c.panel.withValues(alpha: 0.72), c.panel.withValues(alpha: 0)],
                        ),
                      ),
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(22, 18, 22, 18),
                    child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 3),
                        decoration: BoxDecoration(color: c.accent, borderRadius: BorderRadius.circular(6)),
                        child: Text(_kind.toUpperCase(), style: Ty.label.copyWith(color: c.accentInk)),
                      ),
                      const Spacer(),
                      FractionallySizedBox(
                        widthFactor: 0.62,
                        alignment: Alignment.centerLeft,
                        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                          Text(
                            card.title,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(fontSize: compact ? 21 : 26, fontWeight: FontWeight.w700, height: 1.15, letterSpacing: -0.5, color: c.text),
                          ),
                          const SizedBox(height: 8),
                          for (final name in card.featured)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 2),
                              child: Row(mainAxisSize: MainAxisSize.min, children: [
                                Icon(Icons.star_rounded, size: 16, color: c.gold),
                                const SizedBox(width: 4),
                                Flexible(child: Text(name, maxLines: 1, overflow: TextOverflow.ellipsis, style: Ty.h3.copyWith(color: c.text))),
                              ]),
                            ),
                        ]),
                      ),
                      const SizedBox(height: 10),
                      Row(children: [
                        Icon(Icons.schedule, size: 15, color: c.muted),
                        const SizedBox(width: 5),
                        Flexible(child: Text(timeLeft(_banner.msLeft), maxLines: 1, overflow: TextOverflow.ellipsis, style: Ty.small.copyWith(color: c.muted))),
                        const Spacer(),
                        if (p != null)
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                            decoration: BoxDecoration(color: c.bg.withValues(alpha: 0.78), borderRadius: BorderRadius.circular(8), border: Border.all(color: c.line2)),
                            child: Text('Your pity ${p.pity} / ${p.hardPity}', style: Ty.num.copyWith(fontSize: 13, fontWeight: FontWeight.w600, color: c.text)),
                          ),
                      ]),
                    ]),
                  ),
                ]);
              }),
            ),
          ),
        );
    return Semantics(
      button: p != null,
      label: words,
      onTap: p == null ? null : () => onSelect(_banner.pool),
      child: ExcludeSemantics(
        child: game == GameId.zenless ? Transform(alignment: Alignment.center, transform: Matrix4.skewX(-0.1), child: card) : card,
      ),
    );
  }

  /// The card's outline: rounded and gold-edged for Genshin, bevelled for Star Rail, square and bright for Zenless.
  Decoration _frame(WishColors c) => switch (game) {
        GameId.genshin => BoxDecoration(borderRadius: BorderRadius.circular(22), border: Border.all(color: c.gold.withValues(alpha: 0.55), width: 1.5)),
        GameId.starRail => ShapeDecoration(
            shape: BeveledRectangleBorder(
              side: BorderSide(color: c.line2),
              borderRadius: const BorderRadius.only(topLeft: Radius.circular(28), bottomRight: Radius.circular(28)),
            ),
          ),
        GameId.zenless => BoxDecoration(borderRadius: BorderRadius.circular(4), border: Border.all(color: c.accent, width: 2)),
      };
}
