import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/state/providers.dart';
import '../../features/orbit/orbit_state.dart';
import '../../shared/widgets/basics.dart';
import '../theme/metrics.dart';

/// The player, as a bar floating over the page. Clicking it opens Orbit. It rebuilds when the song or the transport
/// changes; the progress line and the time are the only parts that follow the position.
class Capsule extends ConsumerWidget {
  const Capsule({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final song = ref.watch(currentSongProvider);
    final transport = ref.watch(playerProvider.select((p) => p.transport));
    final a = ref.read(actionsProvider);
    final subtitle = transport.failed
        ? (transport.failure ?? 'Playback failed')
        : transport.busy
        ? 'Loading...'
        : (song?.artist ?? 'Pick something to play');
    final play = IconButton.filled(
      onPressed: song == null ? null : a.toggle,
      tooltip: transport.playing ? 'Pause' : 'Play',
      iconSize: 22,
      style: IconButton.styleFrom(shape: const CircleBorder(), minimumSize: const Size(40, 40), padding: EdgeInsets.zero),
      icon: Icon(transport.playing ? Icons.pause : Icons.play_arrow),
    );
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: Metrics.capsuleMaxWidth),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: PxPress(
          onTap: () => ref.read(orbitOpenProvider.notifier).toggle(),
          builder: (context, hovered) => DecoratedBox(
            decoration: BoxDecoration(
              color: Color.alphaBlend(scheme.onSurface.withValues(alpha: hovered ? 0.1 : 0.07), scheme.surface),
              borderRadius: BorderRadius.circular(16),
              border: Border.all(color: scheme.outlineVariant.withValues(alpha: 0.6)),
            ),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(15),
              child: SizedBox(
                height: Metrics.capsuleHeight,
                child: Stack(
                  children: [
                    Padding(
                      padding: const EdgeInsets.fromLTRB(12, 0, 12, 2),
                      child: Row(
                        children: [
                          PxArt(url: song?.thumbnail, size: 48, radius: 8),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                              mainAxisAlignment: MainAxisAlignment.center,
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  song?.title ?? 'Nothing playing',
                                  style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                Text(
                                  subtitle,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: transport.failed ? scheme.error : scheme.onSurfaceVariant,
                                  ),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ],
                            ),
                          ),
                          RepaintBoundary(child: _Time(durationMs: song?.durationMs)),
                          const SizedBox(width: 8),
                          IconButton(
                            onPressed: song == null ? null : a.previous,
                            tooltip: 'Previous',
                            icon: const Icon(Icons.skip_previous),
                          ),
                          play,
                          IconButton(
                            onPressed: song == null ? null : a.next,
                            tooltip: 'Next',
                            icon: const Icon(Icons.skip_next),
                          ),
                        ],
                      ),
                    ),
                    Positioned(
                      left: 0,
                      right: 0,
                      bottom: 0,
                      child: RepaintBoundary(child: _Progress(durationMs: song?.durationMs)),
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

class _Time extends ConsumerWidget {
  const _Time({required this.durationMs});

  final int? durationMs;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final position = ref.watch(positionProvider);
    if (durationMs == null) return const SizedBox.shrink();
    return Text(
      '${clock(position)} / ${clock(durationMs)}',
      style: Theme.of(context).textTheme.bodySmall?.copyWith(
        color: Theme.of(context).colorScheme.onSurfaceVariant,
        fontFeatures: const [FontFeature.tabularFigures()],
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
    return LinearProgressIndicator(value: value, minHeight: 3, backgroundColor: const Color(0x00000000));
  }
}
