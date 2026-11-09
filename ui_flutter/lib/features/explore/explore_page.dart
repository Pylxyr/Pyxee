import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/lists.dart';
import '../../shared/widgets.dart';

/// Explore: genre chips, "Jump back in" (recent plays), then a shelf of similar songs per seed. Picking a genre swaps
/// the shelves for that genre's.
class ExplorePage extends ConsumerStatefulWidget {
  const ExplorePage({super.key});

  @override
  ConsumerState<ExplorePage> createState() => _ExplorePageState();
}

class _ExplorePageState extends ConsumerState<ExplorePage> {
  Genre? _genre;
  bool _more = false;
  String? _opening;
  static const _shown = 12;

  Future<void> _playPlaylist(PlaylistCover pl) async {
    if (_opening != null) return;
    setState(() => _opening = pl.id);
    final toast = ref.read(toastProvider.notifier);
    try {
      final tracks = await ref.read(apiProvider).playlistTracks(pl.id);
      if (tracks.isEmpty) {
        toast.push('${pl.title} has nothing to play right now');
      } else {
        ref.read(actionsProvider).play(tracks);
      }
    } on Object catch (e) {
      toast.push(errorText(e));
    } finally {
      if (mounted) setState(() => _opening = null);
    }
  }

  void _refresh() {
    ref.invalidate(exploreProvider);
    ref.invalidate(genresProvider);
    final g = _genre;
    if (g != null) ref.invalidate(genreProvider(g.params));
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final genres = ref.watch(genresProvider).value ?? const <Genre>[];
    final actions = ref.read(actionsProvider);
    final genre = _genre;
    final shelves = genre == null ? ref.watch(exploreProvider) : ref.watch(genreProvider(genre.params));
    final home = ref.watch(homeProvider).value;
    final loading = shelves.isLoading;

    final hiddenPicked = genre != null && genres.indexWhere((g) => g.params == genre.params) >= _shown;
    final visible = (_more || hiddenPicked) ? genres : genres.take(_shown).toList();

    final children = <Widget>[
      if (genres.isNotEmpty)
        Padding(
          padding: const EdgeInsets.only(top: 16),
          child: Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              PxChip(label: 'For you', selected: genre == null, onTap: () => setState(() => _genre = null)),
              for (final g in visible) PxChip(label: g.title, selected: genre?.params == g.params, onTap: () => setState(() => _genre = g)),
              if (genres.length > _shown && !(_more || hiddenPicked)) PxChip(label: 'More genres', selected: false, dashed: true, onTap: () => setState(() => _more = true)),
            ],
          ),
        ),
      const SizedBox(height: 8),
      ...shelves.when(
        loading: () => <Widget>[const SizedBox(height: 24), const PxSkeleton(rows: 4)],
        error: (e, _) => <Widget>[
          const SizedBox(height: 24),
          PxNotice(children: [
            Row(children: [
              Flexible(child: Text("Couldn't load this. ${errorText(e)}", style: PxText.body(p.fg))),
              const SizedBox(width: 8),
              PxLink('Try again', onTap: _refresh),
            ]),
          ]),
        ],
        data: (list) => _shelves(context, list, genre == null ? (home?.recent ?? const <Song>[]) : const <Song>[], actions),
      ),
    ];
    return PxScrollPage(
      header: PxPageHeader(
        title: 'Explore',
        actions: [PxButton(label: 'Refresh', icon: 'repeat', onPressed: loading ? null : _refresh)],
      ),
      children: children,
    );
  }

  List<Widget> _shelves(BuildContext context, List<Shelf> list, List<Song> jump, PlayerActions actions) {
    final p = context.px;
    final out = <Widget>[];
    var first = true;
    if (jump.isNotEmpty) {
      out.add(PxShelf(
        title: 'Jump back in',
        first: first,
        tiles: [for (var i = 0; i < jump.length; i++) PxTileData(title: jump[i].title, subtitle: jump[i].artist, url: jump[i].thumbnail, seed: jump[i].key, onTap: () => actions.play(jump, i))],
      ));
      first = false;
    }
    for (final s in list) {
      if (s.playlists.isNotEmpty) {
        out.add(PxShelf(
          title: s.title,
          first: first,
          tiles: [
            for (final pl in s.playlists)
              PxTileData(title: pl.title, subtitle: pl.subtitle, url: pl.thumbnail, seed: pl.id, busy: _opening == pl.id, onTap: _opening == null ? () => unawaited(_playPlaylist(pl)) : null),
          ],
        ));
      } else if (s.albums.isNotEmpty) {
        out.add(PxShelf(
          title: s.title,
          first: first,
          tiles: [
            for (final a in s.albums)
              PxTileData(title: a.title, subtitle: a.artist, url: a.thumbnail, seed: a.id, onTap: () => ref.read(searchRequestProvider.notifier).run('${a.title} ${a.artist}')),
          ],
        ));
      } else if (s.songs.isNotEmpty) {
        // Songs from the similar-music shelves are a table with a Play all; a genre's songs are covers.
        out.add(PxSectionHead(s.title, count: s.songs.length, first: first, actions: [PxButton(label: 'Play all', small: true, kind: PxButtonKind.primary, onPressed: () => actions.play(s.songs))]));
        out.add(PxTrackList(tracks: s.songs, onPlay: (i) => actions.play(s.songs, i)));
      } else {
        out.add(PxSectionHead(s.title, first: first));
        out.add(Text('Nothing new to suggest here yet.', style: PxText.body(p.muted)));
      }
      first = false;
    }
    if (list.isEmpty && jump.isEmpty) {
      out.add(Padding(
        padding: const EdgeInsets.only(top: 24),
        child: PxNotice(children: [Text('Like a few songs or play something and Explore will add shelves of similar music here.', style: PxText.body(p.muted))]),
      ));
    }
    return out;
  }
}
