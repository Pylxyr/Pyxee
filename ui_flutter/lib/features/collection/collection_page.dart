import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/widgets/basics.dart';

/// What is kept: liked songs, playlists and the listening history.
class CollectionPage extends StatelessWidget {
  const CollectionPage({super.key});

  @override
  Widget build(BuildContext context) {
    return const DefaultTabController(
      length: 3,
      child: Column(
        children: [
          TabBar(
            isScrollable: true,
            tabAlignment: TabAlignment.start,
            dividerColor: Color(0x00000000),
            tabs: [Tab(text: 'Liked'), Tab(text: 'Playlists'), Tab(text: 'History')],
          ),
          Expanded(child: TabBarView(children: [_Liked(), _Playlists(), _History()])),
        ],
      ),
    );
  }
}

class _Liked extends ConsumerWidget {
  const _Liked();

  @override
  Widget build(BuildContext context, WidgetRef ref) => PxAsync<List<Song>>(
    value: ref.watch(likedProvider),
    onRetry: () => ref.invalidate(likedProvider),
    builder: (songs) => PxSongList(
      songs: songs,
      empty: 'Songs you like show up here.',
      onPlay: (i) => ref.read(actionsProvider).play(songs, i),
    ),
  );
}

class _Playlists extends ConsumerWidget {
  const _Playlists();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final playlists = ref.watch(libraryProvider.select((l) => l.playlists));
    return Column(
      children: [
        Align(
          alignment: Alignment.centerRight,
          child: Padding(
            padding: const EdgeInsets.all(8),
            child: FilledButton.tonalIcon(
              onPressed: () async {
                final api = ref.read(apiProvider);
                final name = await askName(context, 'New playlist');
                if (name != null && name.trim().isNotEmpty) await api.createPlaylist(name.trim(), const []);
              },
              icon: const Icon(Icons.add),
              label: const Text('New playlist'),
            ),
          ),
        ),
        Expanded(
          child: playlists.isEmpty
              ? const PxMessage('No playlists yet.')
              : ListView.builder(
                  itemCount: playlists.length,
                  itemExtent: 64,
                  itemBuilder: (context, i) {
                    final p = playlists[i];
                    return ListTile(
                      leading: PxArt(url: p.thumbnail, size: 44, radius: 8),
                      title: Text(p.name, maxLines: 1, overflow: TextOverflow.ellipsis),
                      subtitle: Text('${p.trackCount} songs'),
                      onTap: () => context.go('/collection/playlist/${p.id}'),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _History extends ConsumerWidget {
  const _History();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return PxAsync<List<HistoryItem>>(
      value: ref.watch(historyProvider),
      onRetry: () => ref.invalidate(historyProvider),
      builder: (items) {
        final songs = [for (final h in items) h.song];
        return Column(
          children: [
            Align(
              alignment: Alignment.centerRight,
              child: TextButton(
                onPressed: items.isEmpty
                    ? null
                    : () async {
                        final api = ref.read(apiProvider);
                        final messenger = ScaffoldMessenger.of(context);
                        final cleared = await api.clearHistory();
                        messenger.showSnackBar(
                          SnackBar(
                            content: const Text('History cleared'),
                            action: SnackBarAction(
                              label: 'Undo',
                              onPressed: () => unawaited(api.restoreHistory(cleared)),
                            ),
                          ),
                        );
                      },
                child: const Text('Clear history'),
              ),
            ),
            Expanded(
              child: PxSongList(
                songs: songs,
                empty: 'Nothing played yet.',
                onPlay: (i) => ref.read(actionsProvider).play(songs, i),
              ),
            ),
          ],
        );
      },
    );
  }
}
