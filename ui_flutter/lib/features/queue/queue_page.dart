import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/state/providers.dart';
import '../../shared/lists.dart';
import '../../shared/sheets.dart';
import '../../shared/widgets.dart';
import '../now_playing/now_playing.dart' show removeFromQueue;

/// The queue: every song, the one that plays marked, in an order you can change by dragging.
class QueuePage extends ConsumerWidget {
  const QueuePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final player = ref.watch(playerProvider);
    final queue = player.queue;
    final actions = ref.read(actionsProvider);
    final api = ref.read(apiProvider);
    final toast = ref.read(toastProvider.notifier);
    final p = context.px;
    final pad = pagePad(MediaQuery.sizeOf(context).width);
    final firstRadio = queue.where((e) => e.radio).map((e) => e.id).firstOrNull;
    final nowId = player.current;

    void saveAsPlaylist() => unawaited(
      showNameSheet(
        context,
        title: 'Save queue as playlist',
        confirm: 'Save',
        run: (name) async {
          final id = await api.createPlaylist(name.trim(), [for (final e in queue) e.song]);
          toast.push('Saved ${queue.length} songs as ${name.trim()}', actionLabel: 'Open', onAction: () => ref.read(navProvider.notifier).openPlaylist(id));
        },
      ),
    );

    void clear() {
      actions.clear();
      toast.push('Queue cleared', actionLabel: 'Undo', onAction: actions.undo);
    }

    return Column(
      children: [
        Padding(
          padding: EdgeInsets.symmetric(horizontal: pad),
          child: PxPageHeader(
            title: 'Queue',
            meta: songsCount(queue.length),
            actions: [
              PxButton(label: 'Save as playlist', onPressed: queue.isEmpty ? null : saveAsPlaylist),
              PxButton(label: 'Clear queue', onPressed: queue.isEmpty ? null : clear),
            ],
          ),
        ),
        Expanded(
          child: queue.isEmpty
              ? const PxEmptyState(icon: 'queue', title: 'Your queue is empty', text: 'Search for a song and choose Play next or Add to queue.')
              : ReorderableListView.builder(
                  padding: EdgeInsets.fromLTRB(pad, 0, pad, 32),
                  buildDefaultDragHandles: false,
                  itemCount: queue.length,
                  proxyDecorator: (child, _, _) => Material(color: Colors.transparent, child: child),
                  onReorder: (from, to) {
                    final dest = to > from ? to - 1 : to;
                    if (dest != from) actions.move(queue[from].id, dest);
                  },
                  itemBuilder: (context, i) {
                    final e = queue[i];
                    final isNow = e.id == nowId;
                    final loading = e.id == player.transport.id && player.transport.state == 'loading';
                    return Column(
                      key: ValueKey(e.id),
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        if (e.id == firstRadio)
                          Padding(padding: const EdgeInsets.fromLTRB(12, 20, 12, 4), child: Text('Autoplay · similar songs', style: PxText.caption(p.muted))),
                        PxEditableRow(
                          song: e.song,
                          index: i,
                          current: isNow || loading,
                          status: isNow ? 'Now playing' : (loading ? 'Loading' : ''),
                          onPlay: () => actions.playEntry(e.id),
                          handle: ReorderableDragStartListener(index: i, child: const PxHandle()),
                          actions: [
                            PxIconButton('up', iconSize: 18, label: 'Move ${e.song.title} up', onPressed: i == 0 ? null : () => actions.move(e.id, i - 1)),
                            PxIconButton('down', iconSize: 18, label: 'Move ${e.song.title} down', onPressed: i == queue.length - 1 ? null : () => actions.move(e.id, i + 1)),
                            PxIconButton('close', iconSize: 18, label: 'Remove ${e.song.title} from queue', onPressed: () => removeFromQueue(ref, e)),
                          ],
                        ),
                      ],
                    );
                  },
                ),
        ),
      ],
    );
  }
}
