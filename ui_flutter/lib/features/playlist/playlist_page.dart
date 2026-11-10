import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/lists.dart';
import '../../shared/sheets.dart';
import '../../shared/widgets.dart';

/// One playlist: its cover, Play, Shuffle, Rename, Delete, and its songs in an order you can change by dragging.
class PlaylistPage extends ConsumerWidget {
  const PlaylistPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final id = ref.watch(navProvider.select((n) => n.playlistId));
    if (id == null) return _missing();
    final value = ref.watch(playlistProvider(id));
    final api = ref.read(apiProvider);
    final nav = ref.read(navProvider.notifier);
    final toast = ref.read(toastProvider.notifier);
    final actions = ref.read(actionsProvider);
    return value.when(
      loading: () => const PxScrollPage(header: null, children: [PxSkeleton(rows: 6)]),
      error: (_, _) => _missing(),
      data: (pl) {
        final tracks = [for (final e in pl.entries) e.song];
        void play(bool shuffle, [int start = 0]) {
          if (tracks.isEmpty) return;
          if (shuffle != ref.read(playerProvider).shuffle) actions.setShuffle(shuffle);
          actions.play(tracks, start);
        }

        Future<void> remove() async {
          try {
            final snap = await api.deletePlaylist(pl.id);
            nav.openLibrary(LibraryTab.playlists);
            toast.push('Deleted "${pl.name}"', actionLabel: 'Undo', onAction: () => unawaited(api.restorePlaylist(snap)));
          } on Object catch (e) {
            toast.push(errorText(e));
          }
        }

        Future<void> removeEntry(PlaylistEntry e) async {
          try {
            final removed = await api.removeEntry(e.entryId);
            toast.push('Removed "${e.song.title}"', actionLabel: 'Undo', onAction: () => unawaited(api.restoreEntry(removed)));
          } on Object catch (err) {
            toast.push(errorText(err));
          }
        }

        void move(int entryId, int to) {
          if (to < 0 || to >= pl.entries.length) return;
          unawaited(api.moveEntry(entryId, to).catchError((Object e) => toast.push(errorText(e))));
        }

        final pad = pagePad(MediaQuery.sizeOf(context).width);
        return Column(
          children: [
            Padding(
              padding: EdgeInsets.symmetric(horizontal: pad),
              child: PxPageHeader(
                title: pl.name,
                meta: songsCount(pl.entries.length),
                lead: PxThumb(url: pl.entries.firstOrNull?.song.thumbnail, seed: 'pl:${pl.id}', size: 160),
                actions: [
                  PxButton(label: 'Play', kind: PxButtonKind.primary, onPressed: tracks.isEmpty ? null : () => play(false)),
                  PxButton(label: 'Shuffle', onPressed: tracks.isEmpty ? null : () => play(true)),
                  PxButton(
                    label: 'Rename',
                    onPressed: () => unawaited(showNameSheet(context, title: 'Rename playlist', confirm: 'Save', initial: pl.name, run: (name) => api.renamePlaylist(pl.id, name.trim()))),
                  ),
                  PxButton(label: 'Delete', onPressed: () => unawaited(remove())),
                ],
              ),
            ),
            Expanded(
              child: pl.entries.isEmpty
                  ? const PxEmptyState(icon: 'music', title: 'This playlist is empty', text: 'Choose Add to playlist on any song to put it here.')
                  : ReorderableListView.builder(
                      padding: EdgeInsets.fromLTRB(pad, 0, pad, 32),
                      buildDefaultDragHandles: false,
                      itemCount: pl.entries.length,
                      proxyDecorator: (child, _, _) => Material(color: Colors.transparent, child: child),
                      onReorderItem: (from, to) {
                        if (to != from) move(pl.entries[from].entryId, to);
                      },
                      itemBuilder: (context, i) {
                        final e = pl.entries[i];
                        return KeyedSubtree(
                          key: ValueKey(e.entryId),
                          child: PxEditableRow(
                            song: e.song,
                            index: i,
                            onPlay: () => play(false, i),
                            handle: ReorderableDragStartListener(index: i, child: const PxHandle()),
                            actions: [
                              PxIconButton('up', iconSize: 18, label: 'Move ${e.song.title} up', onPressed: i == 0 ? null : () => move(e.entryId, i - 1)),
                              PxIconButton('down', iconSize: 18, label: 'Move ${e.song.title} down', onPressed: i == pl.entries.length - 1 ? null : () => move(e.entryId, i + 1)),
                              PxIconButton('close', iconSize: 18, label: 'Remove ${e.song.title} from playlist', onPressed: () => unawaited(removeEntry(e))),
                            ],
                          ),
                        );
                      },
                    ),
            ),
          ],
        );
      },
    );
  }

  Widget _missing() => const PxScrollPage(
    header: null,
    children: [PxEmptyState(icon: 'library', title: 'This playlist no longer exists', text: 'It may have been deleted. Your other playlists are in the Library.')],
  );
}
