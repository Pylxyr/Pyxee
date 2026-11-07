import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/models/models.dart';
import '../../core/state/providers.dart';

/// A cover. Decoded at the size it is shown at, so a screen of covers stays small in memory.
class PxArt extends StatelessWidget {
  const PxArt({super.key, required this.url, required this.size, this.radius = 10});

  final String? url;
  final double size;
  final double radius;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final placeholder = ColoredBox(
      color: scheme.surfaceContainerHigh,
      child: Icon(Icons.music_note, size: size * 0.4, color: scheme.onSurfaceVariant),
    );
    final src = url;
    return SizedBox.square(
      dimension: size,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(radius),
        child: src == null
            ? placeholder
            : Image.network(
                src,
                cacheWidth: (size * MediaQuery.devicePixelRatioOf(context)).round(),
                fit: BoxFit.cover,
                gaplessPlayback: true,
                filterQuality: FilterQuality.low,
                errorBuilder: (_, _, _) => placeholder,
              ),
      ),
    );
  }
}

class PxMessage extends StatelessWidget {
  const PxMessage(this.text, {super.key, this.onRetry});

  final String text;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        spacing: 12,
        children: [
          Text(text, textAlign: TextAlign.center, style: Theme.of(context).textTheme.bodyLarge),
          if (onRetry != null) FilledButton.tonal(onPressed: onRetry, child: const Text('Try again')),
        ],
      ),
    ),
  );
}

/// Shows a loading spinner, the error with a retry, or the data.
class PxAsync<T> extends StatelessWidget {
  const PxAsync({super.key, required this.value, required this.onRetry, required this.builder});

  final AsyncValue<T> value;
  final VoidCallback onRetry;
  final Widget Function(T data) builder;

  @override
  Widget build(BuildContext context) => value.when(
    data: builder,
    error: (e, _) => PxMessage(errorText(e), onRetry: onRetry),
    loading: () => const Center(child: CircularProgressIndicator()),
  );
}

String clock(int? ms) {
  if (ms == null) return '';
  final s = ms ~/ 1000;
  return '${s ~/ 60}:${(s % 60).toString().padLeft(2, '0')}';
}

Future<void> showAddToPlaylist(BuildContext context, WidgetRef ref, List<Song> songs) async {
  final api = ref.read(apiProvider);
  final playlists = ref.read(libraryProvider).playlists;
  final messenger = ScaffoldMessenger.of(context);
  final choice = await showDialog<int>(
    context: context,
    builder: (context) => SimpleDialog(
      title: const Text('Add to playlist'),
      children: [
        SimpleDialogOption(
          onPressed: () => Navigator.pop(context, -1),
          child: const Text('New playlist...'),
        ),
        for (final p in playlists)
          SimpleDialogOption(
            onPressed: () => Navigator.pop(context, p.id),
            child: Text(p.name),
          ),
      ],
    ),
  );
  if (choice == null || !context.mounted) return;
  if (choice >= 0) {
    await api.addToPlaylist(choice, songs);
    messenger.showSnackBar(const SnackBar(content: Text('Added to the playlist')));
    return;
  }
  final name = await askName(context, 'New playlist');
  if (name == null || name.trim().isEmpty) return;
  await api.createPlaylist(name.trim(), songs);
  messenger.showSnackBar(SnackBar(content: Text('Created "${name.trim()}"')));
}

Future<String?> askName(BuildContext context, String title, [String initial = '']) {
  final controller = TextEditingController(text: initial);
  return showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(title),
      content: TextField(
        controller: controller,
        autofocus: true,
        decoration: const InputDecoration(hintText: 'Name'),
        onSubmitted: (v) => Navigator.pop(context, v),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
        FilledButton(onPressed: () => Navigator.pop(context, controller.text), child: const Text('Save')),
      ],
    ),
  ).whenComplete(controller.dispose);
}

/// One song in a list: cover, title, artist, length, a like button and a menu.
class PxSongTile extends ConsumerWidget {
  const PxSongTile({super.key, required this.song, required this.onTap, this.trailing});

  final Song song;
  final VoidCallback onTap;
  final Widget? trailing;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final liked = ref.watch(libraryProvider.select((l) => l.likedKeys.contains(song.key)));
    final actions = ref.read(actionsProvider);
    return ListTile(
      onTap: onTap,
      leading: PxArt(url: song.thumbnail, size: 44, radius: 8),
      title: Text(song.title, maxLines: 1, overflow: TextOverflow.ellipsis),
      subtitle: Text(
        song.durationMs == null ? song.artist : '${song.artist} - ${clock(song.durationMs)}',
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          ?trailing,
          IconButton(
            tooltip: liked ? 'Remove from liked' : 'Like',
            onPressed: () => ref.read(apiProvider).setLiked(song, !liked),
            icon: Icon(liked ? Icons.favorite : Icons.favorite_border),
          ),
          PopupMenuButton<String>(
            tooltip: 'More',
            onSelected: (v) => switch (v) {
              'next' => actions.playNext(song),
              'queue' => actions.addToQueue(song),
              'search' => context.go('/search?q=${Uri.encodeQueryComponent(song.artist)}'),
              _ => showAddToPlaylist(context, ref, [song]),
            },
            itemBuilder: (_) => const [
              PopupMenuItem(value: 'next', child: Text('Play next')),
              PopupMenuItem(value: 'queue', child: Text('Add to queue')),
              PopupMenuItem(value: 'playlist', child: Text('Add to playlist')),
              PopupMenuItem(value: 'search', child: Text('More by this artist')),
            ],
          ),
        ],
      ),
    );
  }
}

/// A scrolling list of songs, played from the tapped one.
class PxSongList extends StatelessWidget {
  const PxSongList({super.key, required this.songs, required this.onPlay, this.empty = 'Nothing here yet.'});

  final List<Song> songs;
  final void Function(int index) onPlay;
  final String empty;

  @override
  Widget build(BuildContext context) {
    if (songs.isEmpty) return PxMessage(empty);
    return ListView.builder(
      itemCount: songs.length,
      itemExtent: 64,
      itemBuilder: (context, i) => PxSongTile(song: songs[i], onTap: () => onPlay(i)),
    );
  }
}
