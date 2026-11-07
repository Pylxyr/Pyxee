import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/bridge/bridge_providers.dart';
import '../destinations.dart';

/// From this width the places are tabs in a masthead on top; below it they are a bar along the bottom.
const double wideBreakpoint = 900;

int? _selectedIndex(String path) {
  if (path.startsWith('/playlist')) return 2;
  final i = destinations.indexWhere((d) => d.path == path);
  return i < 0 ? null : i;
}

/// The frame around every page: masthead (wide windows), the page, the deck, and the bottom bar (narrow windows).
class AppShell extends StatelessWidget {
  const AppShell({super.key, required this.path, required this.child});

  final String path;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final wide = MediaQuery.sizeOf(context).width >= wideBreakpoint;
    final selected = _selectedIndex(path);
    return Scaffold(
      body: Column(
        children: [
          if (wide) _Masthead(selected: selected),
          Expanded(child: child),
          const _Deck(),
        ],
      ),
      bottomNavigationBar: wide
          ? null
          : NavigationBar(
              selectedIndex: selected ?? 0,
              onDestinationSelected: (i) => context.go(destinations[i].path),
              destinations: [
                for (final d in destinations)
                  NavigationDestination(icon: Icon(d.icon), label: d.label),
              ],
            ),
    );
  }
}

class _Masthead extends StatelessWidget {
  const _Masthead({required this.selected});

  final int? selected;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SizedBox(
      height: 56,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 24),
        child: Row(
          children: [
            Text('Pyxee', style: theme.textTheme.titleLarge),
            const SizedBox(width: 32),
            for (var i = 0; i < destinations.length; i++)
              TextButton(
                onPressed: () => context.go(destinations[i].path),
                style: TextButton.styleFrom(
                  foregroundColor: i == selected
                      ? theme.colorScheme.primary
                      : theme.colorScheme.onSurfaceVariant,
                ),
                child: Text(destinations[i].label),
              ),
            const Spacer(),
            IconButton(
              onPressed: () => context.go('/search'),
              tooltip: 'Search',
              icon: const Icon(Icons.search),
            ),
          ],
        ),
      ),
    );
  }
}

/// Where the player will sit. For now it only shows whether the Rust library answered.
class _Deck extends ConsumerWidget {
  const _Deck();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return SizedBox(
      height: 56,
      width: double.infinity,
      child: Center(child: Text(ref.watch(bridgeStatusProvider))),
    );
  }
}
