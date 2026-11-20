import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/wishtory_api.dart';
import '../../domain/game_info.dart';
import '../../domain/planning.dart';
import '../../domain/recovery.dart';
import '../../domain/time_text.dart';
import '../../state/loadable.dart';
import '../../state/nav.dart';
import '../../state/providers.dart';
import '../../state/refresh.dart';
import '../../ui/tokens.dart';
import '../../ui/view_controls.dart';
import '../../ui/widgets.dart';
import '../shell/shell.dart';
import 'banner_stats.dart';
import 'game_boards.dart';
import 'first_five_prompt.dart';
import 'live_hero.dart';

/// Which banner's detail is open; null means the first one.
class SelectedBanner extends Notifier<String?> {
  @override
  String? build() => null;
  void select(String key) => state = key;
}

final selectedBannerProvider = NotifierProvider<SelectedBanner, String?>(SelectedBanner.new);

class OverviewPage extends ConsumerWidget {
  const OverviewPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final nav = ref.watch(navProvider);
    final accounts = ref.watch(accountsProvider);
    final account = ref.watch(currentAccountProvider);
    final pity = ref.watch(pityProvider);
    final info = GameInfo.of(nav.game);

    if (accounts.data == null && accounts.loading) {
      return const PageBody(maxWidth: double.infinity, children: [_OverviewSkeleton()]);
    }
    if (account == null) {
      return PageBody(children: [
        const RefreshPanel(),
        SizedBox(
          height: 420,
          child: EmptyState(
            icon: Icons.inbox_outlined,
            title: 'No ${info.name} history yet',
            body: 'Two steps: ${info.howTo} Then come back here and press Refresh. '
                'Wishtory reads the link the game saved on this PC. Nothing is sent anywhere but HoYoverse.',
            actions: [
              FilledButton.icon(
                onPressed: () => ref.read(refreshProvider.notifier).run(),
                icon: const Icon(Icons.sync, size: 18),
                label: const Text('Refresh'),
              ),
              OutlinedButton(onPressed: () => ref.read(navProvider.notifier).goTo(AppPage.importExport), child: const Text('Import a file instead')),
            ],
          ),
        ),
      ]);
    }
    // The data views use the whole window: on a wide screen a capped column leaves wide empty margins.
    return PageBody(maxWidth: double.infinity, children: [
      const RefreshPanel(),
      _body(context, ref, account, pity),
    ]);
  }

  Widget _body(BuildContext context, WidgetRef ref, Account account, Loadable<List<BannerPity>> pity) {
    if (pity.failed && pity.data == null) {
      return RecoveryCard(
        recovery: recoveryFor(pity.error!, game: account.game),
        onRetry: () => ref.read(dataTickProvider.notifier).bump(),
      );
    }
    final list = pity.data;
    if (list == null) return const _OverviewSkeleton();
    if (list.isEmpty || list.every((p) => p.total == 0)) {
      return SizedBox(
        height: 360,
        child: EmptyState(
          icon: Icons.hourglass_empty,
          title: 'A clean slate',
          body: 'This account has no pulls yet. Open the history page in the game, then press Refresh and your first wishes will land here.',
          actions: [FilledButton(onPressed: () => ref.read(refreshProvider.notifier).run(), child: const Text('Refresh'))],
        ),
      );
    }
    final selectedKey = ref.watch(selectedBannerProvider);
    final selected = list.firstWhere((p) => p.banner == selectedKey, orElse: () => list.first);
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      LiveHero(account: account, pity: list, onSelect: (key) => ref.read(selectedBannerProvider.notifier).select(key)),
      _StatusStrip(game: account.game, list: list),
      const SizedBox(height: 20),
      SectionLabel(bannersLabelFor(account.game)),
      BannerBoard(game: account.game, list: list, selected: selected.banner, onSelect: (key) => ref.read(selectedBannerProvider.notifier).select(key)),
      const SizedBox(height: 20),
      _BannerDetail(p: selected, game: account.game),
    ]);
  }
}

class _OverviewSkeleton extends StatelessWidget {
  const _OverviewSkeleton();

  @override
  Widget build(BuildContext context) => Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Row(children: [for (var i = 0; i < 4; i++) const Expanded(child: Padding(padding: EdgeInsets.only(right: 12), child: Skeleton(height: 72, radius: 12)))]),
        const SizedBox(height: 24),
        Row(children: [for (var i = 0; i < 3; i++) const Expanded(child: Padding(padding: EdgeInsets.only(right: 12), child: Skeleton(height: 150, radius: 12)))]),
        const SizedBox(height: 24),
        const Skeleton(height: 240, radius: 12),
      ]);
}

// ------------------------------------------------------------------ status strip

class _StatusStrip extends StatelessWidget {
  const _StatusStrip({required this.game, required this.list});
  final GameId game;
  final List<BannerPity> list;

  @override
  Widget build(BuildContext context) {
    final info = GameInfo.of(game);
    final total = list.fold<int>(0, (a, p) => a + p.total);
    final fives = list.fold<int>(0, (a, p) => a + p.fiveStars);
    var weighted = 0.0;
    var weight = 0;
    FiveStar? latest;
    for (final p in list) {
      if (p.avgFivePity != null && p.fiveStars > 0) {
        weighted += p.avgFivePity! * p.fiveStars;
        weight += p.fiveStars;
      }
      for (final f in p.recentFives) {
        if (latest == null || f.time.compareTo(latest.time) > 0) latest = f;
      }
    }
    final avg = weight == 0 ? null : (weighted / weight).toStringAsFixed(1);
    final stats = [
      StatData('Total pulls', groupDigits(total), sub: '≈ ${groupDigits(total * info.pullCost)} ${info.currency}'),
      StatData('${info.r5} pulls', groupDigits(fives), sub: total == 0 ? null : '${(fives * 100 / total).toStringAsFixed(2)}% of pulls'),
      StatData('Average ${info.r5} pity', avg ?? '–', sub: avg == null ? 'No ${info.r5} yet' : 'pulls per ${info.r5}'),
      StatData('Latest ${info.r5}', latest?.name ?? '–', sub: latest == null ? null : '${latest.pity} pulls · ${dayOf(latest.time)}', text: true),
    ];
    return StatStrip(game: game, stats: stats);
  }
}

// ------------------------------------------------------------------ banner detail

class _BannerDetail extends StatelessWidget {
  const _BannerDetail({required this.p, required this.game});
  final BannerPity p;
  final GameId game;

  @override
  Widget build(BuildContext context) {
    final c = context.wc;
    final info = GameInfo.of(game);
    final plan = p.total > 0 ? pullsToGuarantee(p) : null;
    String worth(int pulls) => '${groupDigits(pulls * info.pullCost)} ${info.currency.toLowerCase()}';
    final odds = p.guaranteeOdds;
    final inSoft = p.softPity != null && p.pity >= p.softPity!;

    String guarantee() {
      if (odds == null) return 'The featured item cannot be told from the history on this banner, so no guarantee is tracked.';
      return switch (p.guaranteed) {
        true => 'Next ${info.r5} is guaranteed to be featured. Your last ${info.r5} was not the featured one.',
        false => 'Not guaranteed: next ${info.r5} is a $odds/${100 - odds}. Your last ${info.r5} was the featured one.',
        null => 'Guarantee status unknown. Not enough history to tell.',
      };
    }

    String planText() {
      if (plan == null) return '';
      if (plan.featured == null) return 'Up to ${plan.fiveStar} more pulls (${worth(plan.fiveStar)}) to a guaranteed ${info.r5}.';
      if (p.guaranteed == true) return 'Up to ${plan.fiveStar} more pulls (${worth(plan.fiveStar)}) to the featured ${info.r5}.';
      return 'Worst case ${plan.featured} more pulls (${worth(plan.featured!)}) to a guaranteed featured ${info.r5}: '
          '${plan.fiveStar} to the next ${info.r5}, then up to ${p.hardPity} more if the $odds/${100 - odds!} is lost.';
    }

    return Panel(
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        LayoutBuilder(builder: (context, box) {
          final title = Row(mainAxisSize: MainAxisSize.min, children: [
            Flexible(child: Text(p.label, style: Ty.h2, overflow: TextOverflow.ellipsis)),
            if (inSoft) ...[const SizedBox(width: 10), const StatusChip('Past soft pity', icon: Icons.trending_up)],
          ]);
          if (box.maxWidth < 1000) {
            return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [title, const SizedBox(height: 8), const ViewControls()]);
          }
          return Row(children: [Expanded(child: title), const SizedBox(width: 16), const ViewControls()]);
        }),
        const SizedBox(height: 6),
        Text(guarantee(), style: Ty.body),
        if (plan != null) ...[
          const SizedBox(height: 4),
          Text(planText(), style: Ty.body.copyWith(color: c.muted)),
        ],
        FirstFivePrompt(p: p, game: game),
        const SizedBox(height: 14),
        LayoutBuilder(builder: (context, box) {
          final stats = StatsTable(p: p, game: game);
          final chips = PullChips(key: ValueKey(p.banner), p: p, game: game);
          if (box.maxWidth < 820) {
            return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [stats, const SizedBox(height: 22), chips]);
          }
          return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            SizedBox(width: 380, child: stats),
            const SizedBox(width: 28),
            Expanded(child: chips),
          ]);
        }),
      ]),
    );
  }
}
