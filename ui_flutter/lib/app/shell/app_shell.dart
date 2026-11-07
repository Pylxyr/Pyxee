import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../features/orbit/orbit_state.dart';
import '../../features/orbit/orbit_view.dart';
import '../places.dart';
import '../theme/metrics.dart';
import 'capsule.dart';
import 'spine.dart';

/// The frame around every page: the spine, the page on its canvas, the capsule floating over it, and Orbit over all.
class AppShell extends StatelessWidget {
  const AppShell({super.key, required this.path, required this.child});

  final String path;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final wide = MediaQuery.sizeOf(context).width >= Metrics.railBreakpoint;
    final place = Place.of(path);
    final canvas = Stack(
      children: [
        Positioned.fill(
          child: _Canvas(
            child: Padding(
              padding: const EdgeInsets.only(
                bottom: Metrics.capsuleHeight + 2 * Metrics.capsuleGap,
              ),
              child: child,
            ),
          ),
        ),
        const Positioned(
          left: 0,
          right: 0,
          bottom: Metrics.capsuleGap,
          child: Center(child: Capsule()),
        ),
      ],
    );
    return Scaffold(
      body: Stack(
        children: [
          Positioned.fill(
            child: wide
                ? Row(
                    children: [
                      Spine(axis: Axis.vertical, current: place),
                      Expanded(child: canvas),
                    ],
                  )
                : Column(
                    children: [
                      Expanded(child: canvas),
                      Spine(axis: Axis.horizontal, current: place),
                    ],
                  ),
          ),
          const Positioned.fill(child: _OrbitLayer()),
        ],
      ),
    );
  }
}

class _Canvas extends StatelessWidget {
  const _Canvas({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(Metrics.canvasInset),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(Metrics.canvasRadius),
        child: ColoredBox(
          color: Theme.of(context).colorScheme.surface,
          child: child,
        ),
      ),
    );
  }
}

/// Orbit slides up over the shell and is not built at all while it is closed.
class _OrbitLayer extends ConsumerWidget {
  const _OrbitLayer();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final open = ref.watch(orbitOpenProvider);
    return AnimatedSwitcher(
      duration: Metrics.orbitDuration,
      switchInCurve: Curves.easeOutCubic,
      switchOutCurve: Curves.easeInCubic,
      transitionBuilder: (child, animation) => SlideTransition(
        position: Tween<Offset>(
          begin: const Offset(0, 1),
          end: Offset.zero,
        ).animate(animation),
        child: child,
      ),
      child: open
          ? const OrbitView(key: ValueKey('orbit'))
          : const SizedBox.shrink(key: ValueKey('closed')),
    );
  }
}
