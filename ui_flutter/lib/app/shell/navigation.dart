import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../shared/icons.dart';
import '../../shared/logo.dart';
import '../../shared/widgets.dart';
import '../state/ui_state.dart';
import '../theme/px_theme.dart';

/// The four top-level places. The top tabs (wide windows) and the bottom bar (narrow ones) show the same list.
const destinations = <({PxRoute route, String label, String icon})>[
  (route: PxRoute.home, label: 'Home', icon: 'home'),
  (route: PxRoute.explore, label: 'Explore', icon: 'explore'),
  (route: PxRoute.library, label: 'Library', icon: 'library'),
  (route: PxRoute.queue, label: 'Queue', icon: 'queue'),
];

/// The top of a wide window: the wordmark (hidden under 1100 px) and the places as tabs, with an accent bar under the
/// chosen one.
class PxTabs extends ConsumerWidget {
  const PxTabs({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final route = ref.watch(navProvider.select((n) => n.route));
    final nav = ref.read(navProvider.notifier);
    final showName = MediaQuery.sizeOf(context).width >= 1100;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        const PxLogo(size: 24),
        if (showName) ...[const SizedBox(width: 10), Text('Pyxee', style: PxText.wordmark(p.fg))],
        const SizedBox(width: 24),
        for (final d in destinations)
          Padding(
            padding: const EdgeInsets.only(right: 2),
            child: PxPress(
              onTap: () => nav.go(d.route),
              semanticLabel: d.label,
              builder: (context, hovered, _) {
                final on = route == d.route;
                return SizedBox(
                  height: 60,
                  child: Stack(
                    alignment: Alignment.center,
                    children: [
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 14),
                        child: Text(d.label, style: PxText.tab(on || hovered ? p.fg : p.muted, on ? FontWeight.w600 : FontWeight.w400)),
                      ),
                      Positioned(
                        left: 14,
                        right: 14,
                        bottom: -1,
                        child: AnimatedContainer(
                          duration: const Duration(milliseconds: 220),
                          curve: Curves.easeOutCubic,
                          height: 3,
                          decoration: BoxDecoration(color: on ? p.accent : Colors.transparent, borderRadius: const BorderRadius.vertical(top: Radius.circular(2))),
                        ),
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
      ],
    );
  }
}

/// The bottom bar of a narrow window.
class PxBottomBar extends ConsumerWidget {
  const PxBottomBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final route = ref.watch(navProvider.select((n) => n.route));
    final nav = ref.read(navProvider.notifier);
    return Container(
      height: 56,
      decoration: BoxDecoration(color: p.surface, border: Border(top: BorderSide(color: p.line))),
      child: Row(
        children: [
          for (final d in destinations)
            Expanded(
              child: PxPress(
                onTap: () => nav.go(d.route),
                semanticLabel: d.label,
                builder: (context, _, _) {
                  final on = route == d.route;
                  return Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Container(
                        width: 56,
                        height: 28,
                        decoration: BoxDecoration(color: on ? p.selected : Colors.transparent, borderRadius: BorderRadius.circular(14)),
                        child: IconTheme(data: IconThemeData(color: on ? p.fg : p.muted), child: Center(child: PxIcon(d.icon, size: 22))),
                      ),
                      const SizedBox(height: 2),
                      Text(d.label, style: PxText.caption(on ? p.fg : p.muted).copyWith(fontWeight: on ? FontWeight.w600 : FontWeight.w400)),
                    ],
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}
