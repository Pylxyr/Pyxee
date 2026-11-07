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
      padding: const EdgeInsets.symmetric(vertical: 16),
      children: [
        if (home != null && home.quickPicks.isNotEmpty)
          _Row(
            title: 'Quick picks',
            cards: [
              for (var i = 0; i < home.quickPicks.length; i++)
                _Card(
                  title: home.quickPicks[i].title,
                  subtitle: home.quickPicks[i].artist,
                  url: home.quickPicks[i].thumbnail,
                  onTap: () => actions.play(home.quickPicks, i),
                ),
            ],
          ),
        if (home != null && home.recent.isNotEmpty)
          _Row(
            title: 'Play it again',
            cards: [
              for (var i = 0; i < home.recent.length; i++)
                _Card(
                  title: home.recent[i].title,
                  subtitle: home.recent[i].artist,
                  url: home.recent[i].thumbnail,
                  onTap: () => actions.play(home.recent, i),
                ),
            ],
          ),
        if (genres.isNotEmpty)
          SizedBox(
            height: 48,
            child: ListView(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.symmetric(horizontal: 16),
              children: [
                for (final g in genres)
                  Padding(
                    padding: const EdgeInsets.only(right: 8),
                    child: ChoiceChip(
                      label: Text(g.title),
                      selected: _genre == g.params,
                      onSelected: (on) => setState(() => _genre = on ? g.params : null),
                    ),
                  ),
              ],
            ),
          ),
        PxAsyncList<Shelf>(
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
    return _Row(
      title: s.title,
      cards: [
        for (var i = 0; i < s.songs.length; i++)
          _Card(
            title: s.songs[i].title,
            subtitle: s.songs[i].artist,
            url: s.songs[i].thumbnail,
            onTap: () => actions.play(s.songs, i),
          ),
        for (final a in s.albums)
          _Card(
            title: a.title,
            subtitle: a.artist,
            url: a.thumbnail,
            onTap: () => context.go('/search?q=${Uri.encodeQueryComponent('${a.title} ${a.artist}')}'),
          ),
        for (final p in s.playlists)
          _Card(
            title: p.title,
            subtitle: p.subtitle,
            url: p.thumbnail,
            onTap: () => unawaited(api.playlistTracks(p.id).then(actions.play)),
          ),
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

class _Row extends StatelessWidget {
  const _Row({required this.title, required this.cards});

  final String title;
  final List<Widget> cards;

  @override
  Widget build(BuildContext context) {
    if (cards.isEmpty) return const SizedBox.shrink();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
          child: Text(title, style: Theme.of(context).textTheme.titleMedium),
        ),
        SizedBox(
          height: 196,
          child: ListView.builder(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            itemCount: cards.length,
            itemExtent: 148,
            itemBuilder: (context, i) => cards[i],
          ),
        ),
      ],
    );
  }
}

class _Card extends StatelessWidget {
  const _Card({required this.title, required this.subtitle, required this.url, required this.onTap});

  final String title;
  final String subtitle;
  final String? url;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.all(6),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            PxArt(url: url, size: 136, radius: 12),
            const SizedBox(height: 6),
            Text(title, maxLines: 1, overflow: TextOverflow.ellipsis, style: theme.textTheme.bodyMedium),
            Text(
              subtitle,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
            ),
          ],
        ),
      ),
    );
  }
}
