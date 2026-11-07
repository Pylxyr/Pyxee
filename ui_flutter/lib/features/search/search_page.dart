import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/widgets/basics.dart';

class SearchPage extends ConsumerStatefulWidget {
  const SearchPage({super.key, required this.query});

  final String query;

  @override
  ConsumerState<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends ConsumerState<SearchPage> {
  late final TextEditingController _controller = TextEditingController(text: widget.query);
  late String _text = widget.query.trim();
  SearchKind _kind = SearchKind.all;

  @override
  void didUpdateWidget(SearchPage old) {
    super.didUpdateWidget(old);
    if (widget.query != old.query) _run(widget.query);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _run(String text, [SearchKind? kind]) {
    _controller.text = text;
    setState(() {
      _text = text.trim();
      _kind = kind ?? _kind;
    });
  }

  @override
  Widget build(BuildContext context) {
    final filters = ref.watch(searchFiltersProvider).value ?? const [SearchKind.all, SearchKind.songs];
    final query = (text: _text, kind: _kind);
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
          child: TextField(
            controller: _controller,
            autofocus: _text.isEmpty,
            textInputAction: TextInputAction.search,
            decoration: const InputDecoration(
              prefixIcon: Icon(Icons.search),
              hintText: 'Search songs, artists, playlists',
              border: OutlineInputBorder(borderRadius: BorderRadius.all(Radius.circular(28))),
            ),
            onSubmitted: _run,
          ),
        ),
        SizedBox(
          height: 48,
          child: ListView(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(horizontal: 16),
            children: [
              for (final f in filters)
                Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: ChoiceChip(
                    label: Text(f.name[0].toUpperCase() + f.name.substring(1)),
                    selected: f == _kind,
                    onSelected: (_) => setState(() => _kind = f),
                  ),
                ),
            ],
          ),
        ),
        Expanded(
          child: _text.isEmpty
              ? const PxMessage('Search for something to play.')
              : PxAsync<List<SearchHit>>(
                  value: ref.watch(searchProvider(query)),
                  onRetry: () => ref.invalidate(searchProvider(query)),
                  builder: (hits) => _Results(hits: hits, onSearch: _run),
                ),
        ),
      ],
    );
  }
}

class _Results extends ConsumerWidget {
  const _Results({required this.hits, required this.onSearch});

  final List<SearchHit> hits;
  final void Function(String text, [SearchKind? kind]) onSearch;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (hits.isEmpty) return const PxMessage('Nothing found.');
    final actions = ref.read(actionsProvider);
    final songs = [for (final h in hits) ?h.song];
    return ListView.builder(
      itemCount: hits.length,
      itemExtent: 64,
      itemBuilder: (context, i) {
        final hit = hits[i];
        final song = hit.song;
        if (song != null) {
          return PxSongTile(song: song, onTap: () => actions.play(songs, songs.indexOf(song)));
        }
        final isArtist = hit.key!.startsWith('yt-channel');
        return ListTile(
          leading: PxArt(url: hit.thumbnail, size: 44, radius: isArtist ? 22 : 8),
          title: Text(hit.title!, maxLines: 1, overflow: TextOverflow.ellipsis),
          subtitle: Text(isArtist ? 'Artist' : 'Playlist'),
          onTap: () => isArtist
              ? onSearch(hit.title!, SearchKind.songs)
              : unawaited(ref.read(apiProvider).playlistTracks(hit.key!).then(actions.play)),
        );
      },
    );
  }
}
