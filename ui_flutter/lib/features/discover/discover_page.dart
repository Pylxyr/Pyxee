import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/widgets/basics.dart';

/// Home (quick picks, recently played) and Explore (YouTube Music's shelves and genres).
class DiscoverPage extends ConsumerStatefulWidget {
  const DiscoverPage({super.key});

  @override
  ConsumerState<DiscoverPage> createState() => _DiscoverPageState();
}

class _DiscoverPageState extends ConsumerState<DiscoverPage> {
  String? _genre;

  @override
  Widget build(BuildContext context) {
    final home = ref.watch(homeProvider).value;
    final genres = ref.watch(genresProvider).value ?? const <Genre>[];
    final shelves = _genre == null ? ref.watch(exploreProvider) : ref.watch(genreProvider(_genre!));
    final actions = ref.read(actionsProvider);
    return ListView(
      padding: const EdgeInsets.fromLTRB(28, 8, 28, 24),
      children: [
        if (home != null && home.quickPicks.isNotEmpty) ...[
          const _Title('Quick picks'),
          _PickGrid(songs: home.quickPicks, onPlay: (i) => actions.play(home.quickPicks, i)),
        ],
        if (home != null && home.recent.isNotEmpty)
          _ShelfRow(
            title: 'Play it again',
            cards: [
              for (var i = 0; i < home.recent.length; i++)
                _Item(home.recent[i].title, home.recent[i].artist, home.recent[i].thumbnail, () => actions.play(home.recent, i)),
            ],
          ),
        if (genres.isNotEmpty) ...[
          const _Title('Moods and genres'),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              for (final g in genres)
                ChoiceChip(
                  label: Text(g.title),
                  selected: _genre == g.params,
                  onSelected: (on) => setState(() => _genre = on ? g.params : null),
                ),
            ],
          ),
        ],
        PxAsyncList<List<Shelf>>(
          value: shelves,
          onRetry: () => ref.invalidate(_genre == null ? exploreProvider : genreProvider(_genre!)),
          builder: (list) => [for (final s in list) _shelf(context, s)],
        ),
      ],
    );
  }

  Widget _shelf(BuildContext context, Shelf s) {
    final actions = ref.read(actionsProvider);
    final api = ref.read(apiProvider);
    return _ShelfRow(
      title: s.title,
      cards: [
        for (var i = 0; i < s.songs.length; i++)
          _Item(s.songs[i].title, s.songs[i].artist, s.songs[i].thumbnail, () => actions.play(s.songs, i)),
        for (final a in s.albums)
          _Item(
            a.title,
            a.artist,
            a.thumbnail,
            () => context.go('/search?q=${Uri.encodeQueryComponent('${a.title} ${a.artist}')}'),
          ),
        for (final p in s.playlists)
          _Item(p.title, p.subtitle, p.thumbnail, () => unawaited(api.playlistTracks(p.id).then(actions.play))),
      ],
    );
  }
}

/// Like [PxAsync], for a screen that is a list of sections: spinner, error with retry, or the sections.
class PxAsyncList<T> extends StatelessWidget {
  const PxAsyncList({super.key, required this.value, required this.onRetry, required this.builder});

  final AsyncValue<T> value;
  final VoidCallback onRetry;
  final List<Widget> Function(T data) builder;

  @override
  Widget build(BuildContext context) => value.when(
    data: (d) => Column(children: builder(d)),
    error: (e, _) => SizedBox(height: 240, child: PxMessage(errorText(e), onRetry: onRetry)),
    loading: () => const SizedBox(height: 240, child: Center(child: CircularProgressIndicator())),
  );
}

class _Item {
  const _Item(this.title, this.subtitle, this.url, this.onTap);

  final String title;
  final String subtitle;
  final String? url;
  final VoidCallback onTap;
}

class _Title extends StatelessWidget {
  const _Title(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 24, bottom: 12),
    child: Text(text, style: Theme.of(context).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w700, fontSize: 20)),
  );
}

/// One row of covers: as many as fit across the page, so nothing needs scrolling sideways.
class _ShelfRow extends StatelessWidget {
  const _ShelfRow({required this.title, required this.cards});

  final String title;
  final List<_Item> cards;

  @override
  Widget build(BuildContext context) {
    if (cards.isEmpty) return const SizedBox.shrink();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _Title(title),
        LayoutBuilder(
          builder: (context, box) {
            const gap = 16.0;
            final count = ((box.maxWidth + gap) / (160 + gap)).floor().clamp(2, 10);
            final width = (box.maxWidth - gap * (count - 1)) / count;
            return Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              spacing: gap,
              children: [
                for (final c in cards.take(count))
                  SizedBox(
                    width: width,
                    child: PxCoverCard(title: c.title, subtitle: c.subtitle, url: c.url, width: width, onTap: c.onTap),
                  ),
              ],
            );
          },
        ),
      ],
    );
  }
}

/// Quick picks as compact rows in a grid: cover, title, artist.
class _PickGrid extends StatelessWidget {
  const _PickGrid({required this.songs, required this.onPlay});

  final List<Song> songs;
  final void Function(int index) onPlay;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, box) {
        const gap = 8.0;
        final columns = (box.maxWidth / 300).floor().clamp(1, 4);
        final width = (box.maxWidth - gap * (columns - 1)) / columns;
        return Wrap(
          spacing: gap,
          runSpacing: gap,
          children: [
            for (var i = 0; i < songs.length && i < columns * 3; i++)
              SizedBox(width: width, height: 56, child: _PickTile(song: songs[i], onTap: () => onPlay(i))),
          ],
        );
      },
    );
  }
}

class _PickTile extends StatelessWidget {
  const _PickTile({required this.song, required this.onTap});

  final Song song;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return PxPress(
      onTap: onTap,
      builder: (context, hovered) => DecoratedBox(
        decoration: BoxDecoration(
          color: scheme.onSurface.withValues(alpha: hovered ? 0.1 : 0.05),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          children: [
            PxArt(url: song.thumbnail, size: 56, radius: 10),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    song.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
                  ),
                  Text(
                    song.artist,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodySmall?.copyWith(color: scheme.onSurfaceVariant),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 12),
          ],
        ),
      ),
    );
  }
}
