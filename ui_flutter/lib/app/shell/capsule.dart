import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/bridge/bridge_providers.dart';
import '../../features/orbit/orbit_state.dart';
import '../theme/metrics.dart';

/// The player as a floating pill over the page. Tapping it opens Orbit. For now it shows the core's transport state
/// and how many times the core has published, and the button sends it a toggle.
class Capsule extends ConsumerWidget {
  const Capsule({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final head = ref.watch(playerHeadProvider);
    final status = head.when(
      data: (h) => '${h.transport} - ${h.queueLength} in queue - #${h.seq}',
      error: (e, _) => 'bridge FAILED: $e',
      loading: () => 'starting',
    );
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
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Row(
                  children: [
                    CircleAvatar(
                      backgroundColor: scheme.primaryContainer,
                      child: Icon(
                        Icons.music_note,
                        color: scheme.onPrimaryContainer,
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Nothing playing',
                            style: theme.textTheme.titleSmall,
                          ),
                          Text(
                            status,
                            style: theme.textTheme.bodySmall,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ),
                    ),
                    IconButton(
                      onPressed: head.hasValue
                          ? () => ref
                                .read(bridgeProvider)
                                .dispatch({'type': 'toggle'})
                          : null,
                      icon: const Icon(Icons.play_arrow),
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
