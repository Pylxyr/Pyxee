import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/state/providers.dart';
import '../../features/orbit/orbit_state.dart';
import '../../shared/widgets/basics.dart';
import '../theme/metrics.dart';

/// The player as a floating pill over the page. Tapping it opens Orbit. It rebuilds when the song or the transport
/// changes; the progress line under the title is the only part that follows the position.
class Capsule extends ConsumerWidget {
  const Capsule({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final song = ref.watch(currentSongProvider);
    final transport = ref.watch(playerProvider.select((p) => p.transport));
    final actions = ref.read(actionsProvider);
    final subtitle = transport.failed
        ? (transport.failure ?? 'Playback failed')
        : transport.busy
        ? 'Loading...'
        : (song?.artist ?? 'Pick something to play');
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: Metrics.capsuleMaxWidth),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12),
        child: Material(
          color: scheme.surfaceContainerHighest,
          shape: StadiumBorder(side: BorderSide(color: scheme.outlineVariant)),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: () => ref.read(orbitOpenProvider.notifier).toggle(),
            child: SizedBox(
              height: Metrics.capsuleHeight,
              child: Padding(
                padding: const EdgeInsets.only(left: 12, right: 8),
                child: Row(
                  children: [
                    PxArt(url: song?.thumbnail, size: 44, radius: 22),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            song?.title ?? 'Nothing playing',
                            style: theme.textTheme.titleSmall,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          Text(
                            subtitle,
                            style: theme.textTheme.bodySmall,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          const SizedBox(height: 4),
                          RepaintBoundary(child: _Progress(durationMs: song?.durationMs)),
                        ],
                      ),
                    ),
                    IconButton(
                      onPressed: song == null ? null : actions.previous,
                      tooltip: 'Previous',
                      icon: const Icon(Icons.skip_previous),
                    ),
                    IconButton.filled(
                      onPressed: song == null ? null : actions.toggle,
                      tooltip: transport.playing ? 'Pause' : 'Play',
                      icon: Icon(transport.playing ? Icons.pause : Icons.play_arrow),
                    ),
                    IconButton(
                      onPressed: song == null ? null : actions.next,
                      tooltip: 'Next',
                      icon: const Icon(Icons.skip_next),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _Progress extends ConsumerWidget {
  const _Progress({required this.durationMs});

  final int? durationMs;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final position = ref.watch(positionProvider);
    final total = durationMs;
    final value = total == null || total <= 0 ? 0.0 : (position / total).clamp(0.0, 1.0);
    return LinearProgressIndicator(value: value, minHeight: 3, borderRadius: BorderRadius.circular(2));
  }
}
