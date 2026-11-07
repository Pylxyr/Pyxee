import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/theme/metrics.dart';
import '../../app/theme/theme.dart';
import 'orbit_state.dart';

/// The full-screen player: the cover, and under or beside it the queue, the lyrics and what is related.
class OrbitView extends ConsumerWidget {
  const OrbitView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    final split = MediaQuery.sizeOf(context).width >= Metrics.orbitSplit;
    final cover = AspectRatio(
      aspectRatio: 1,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: scheme.primaryContainer,
          borderRadius: BorderRadius.circular(Metrics.canvasRadius),
        ),
        child: Icon(
          Icons.music_note,
          size: 64,
          color: scheme.onPrimaryContainer,
        ),
      ),
    );
    return SizedBox.expand(
      child: Material(
        color: Palette.base,
        child: SafeArea(
          child: Column(
            children: [
              Align(
                alignment: Alignment.centerLeft,
                child: Padding(
                  padding: const EdgeInsets.all(8),
                  child: IconButton(
                    onPressed: () => ref.read(orbitOpenProvider.notifier).close(),
                    tooltip: 'Close',
                    icon: const Icon(Icons.keyboard_arrow_down),
                  ),
                ),
              ),
              Expanded(
                child: split
                    ? Row(
                        children: [
                          Expanded(
                            child: Padding(
                              padding: const EdgeInsets.all(32),
                              child: Center(
                                child: ConstrainedBox(
                                  constraints: const BoxConstraints(maxWidth: 420),
                                  child: cover,
                                ),
                              ),
                            ),
                          ),
                          const Expanded(child: _OrbitTabs()),
                        ],
                      )
                    : Column(
                        children: [
                          ConstrainedBox(
                            constraints: const BoxConstraints(maxHeight: 240),
                            child: Padding(
                              padding: const EdgeInsets.all(16),
                              child: cover,
                            ),
                          ),
                          const Expanded(child: _OrbitTabs()),
                        ],
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _OrbitTabs extends StatelessWidget {
  const _OrbitTabs();

  @override
  Widget build(BuildContext context) {
    return const DefaultTabController(
      length: 3,
      child: Column(
        children: [
          TabBar(
            tabs: [
              Tab(text: 'Queue'),
              Tab(text: 'Lyrics'),
              Tab(text: 'Related'),
            ],
          ),
          Expanded(
            child: TabBarView(
              children: [
                Center(child: Text('Queue')),
                Center(child: Text('Lyrics')),
                Center(child: Text('Related')),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
