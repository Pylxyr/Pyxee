import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/widgets/basics.dart';

class PlaylistPage extends ConsumerWidget {
  const PlaylistPage({super.key, required this.id});

  final int? id;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final id = this.id;
    if (id == null) return const PxMessage('That playlist does not exist.');
    return PxAsync<PlaylistDetail>(
      value: ref.watch(playlistProvider(id)),
      onRetry: () => ref.invalidate(playlistProvider(id)),
      builder: (playlist) => _Body(playlist: playlist),
    );
  }
}

class _Body extends ConsumerWidget {
  const _Body({required this.playlist});

  final PlaylistDetail playlist;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final api = ref.read(apiProvider);
    final songs = [for (final e in playlist.entries) e.song];
    final entries = playlist.entries;
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(8, 8, 8, 0),
          child: Row(
            children: [
              IconButton(
                onPressed: () => context.go('/collection'),
                tooltip: 'Back',
                icon: const Icon(Icons.arrow_back),
              ),
              Expanded(
                child: Text(
                  playlist.name,
                  style: Theme.of(context).textTheme.titleLarge,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              IconButton(
                onPressed: songs.isEmpty ? null : () => ref.read(actionsProvider).play(songs),
                tooltip: 'Play',
                icon: const Icon(Icons.play_arrow),
              ),
              IconButton(
                onPressed: () async {
                  final name = await askName(context, 'Rename playlist', playlist.name);
                  if (name != null && name.trim().isNotEmpty) await api.renamePlaylist(playlist.id, name.trim());
                },
                tooltip: 'Rename',
                icon: const Icon(Icons.edit),
              ),
              IconButton(
                onPressed: () async {
                  final messenger = ScaffoldMessenger.of(context);
                  final router = GoRouter.of(context);
                  final snapshot = await api.deletePlaylist(playlist.id);
                  router.go('/collection');
                  messenger.showSnackBar(
                    SnackBar(
                      content: Text('Deleted "${playlist.name}"'),
                      action: SnackBarAction(label: 'Undo', onPressed: () => unawaited(api.restorePlaylist(snapshot))),
                    ),
                  );
                },
                tooltip: 'Delete',
                icon: const Icon(Icons.delete_outline),
              ),
            ],
          ),
        ),
        Expanded(
          child: entries.isEmpty
              ? const PxMessage('This playlist is empty. Add songs from any list.')
              : ReorderableListView.builder(
                  buildDefaultDragHandles: false,
                  itemCount: entries.length,
                  itemExtent: 64,
                  onReorderItem: (oldIndex, newIndex) => unawaited(api.moveEntry(entries[oldIndex].entryId, newIndex)),
                  itemBuilder: (context, i) {
                    final e = entries[i];
                    return KeyedSubtree(
                      key: ValueKey(e.entryId),
                      child: PxSongTile(
                        song: e.song,
                        onTap: () => ref.read(actionsProvider).play(songs, i),
                        trailing: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            IconButton(
                              tooltip: 'Remove from playlist',
                              icon: const Icon(Icons.remove_circle_outline),
                              onPressed: () async {
                                final messenger = ScaffoldMessenger.of(context);
                                final removed = await api.removeEntry(e.entryId);
                                messenger.showSnackBar(
                                  SnackBar(
                                    content: const Text('Removed from the playlist'),
                                    action: SnackBarAction(
                                      label: 'Undo',
                                      onPressed: () => unawaited(api.restoreEntry(removed)),
                                    ),
                                  ),
                                );
                              },
                            ),
                            ReorderableDragStartListener(index: i, child: const Icon(Icons.drag_handle)),
                          ],
                        ),
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}
