import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/bridge/bridge.dart';
import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/lists.dart';
import '../../shared/widgets.dart';

/// Results of a search: songs, artists and playlists in one table, with the filters of the music source.
class SearchPage extends ConsumerWidget {
  const SearchPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final req = ref.watch(searchRequestProvider);
    final notifier = ref.read(searchRequestProvider.notifier);
    final actions = ref.read(actionsProvider);
    final toast = ref.read(toastProvider.notifier);
    final supported = ref.watch(searchFiltersProvider).value;
    final query = req.asQuery;
    if (query == null) {
      return const PxScrollPage(header: null, children: [PxEmptyState(icon: 'search', title: 'Search', text: 'Find songs, videos, artists and playlists. Type above and press Enter.')]);
    }
    const labels = {
      SearchKind.all: 'All',
      SearchKind.songs: 'Songs',
      SearchKind.videos: 'Videos',
      SearchKind.albums: 'Albums',
      SearchKind.artists: 'Artists',
      SearchKind.playlists: 'Playlists',
    };
    final results = ref.watch(searchProvider(query));
    return PxScrollPage(
      header: PxPageHeader(
        title: 'Results for “${req.query}”',
        below: PxSegmented<SearchKind>(
          value: req.kind,
          onSelect: notifier.setKind,
          items: [
            for (final e in labels.entries)
              (
                id: e.key,
                label: e.value,
                disabled: supported != null && e.key != SearchKind.all && !supported.contains(e.key),
                tooltip: 'Not available with the current music source yet',
              ),
          ],
        ),
      ),
      children: [
        results.when(
          loading: () => const PxSkeleton(rows: 8),
          error: (e, _) {
            final kind = e is BridgeException ? e.kind : null;
            final hint = switch (kind) {
              'offline' => 'Check your internet connection, then try again.',
              'toolMissing' => 'Pyxee looks for yt-dlp next to the app. Run "cargo xtask tools" (see docs/development.md) or set PYXEE_YTDLP to its path.',
              'rateLimited' => 'Wait a little while before searching again.',
              _ => '',
            };
            return PxNotice(
              children: [
                Text(errorText(e), style: PxText.strong(p.fg)),
                if (hint.isNotEmpty) Padding(padding: const EdgeInsets.only(top: 4), child: Text(hint, style: PxText.body(p.muted))),
                const SizedBox(height: 12),
                PxButton(label: 'Try again', kind: PxButtonKind.primary, onPressed: () => ref.invalidate(searchProvider(query))),
              ],
            );
          },
          data: (items) {
            if (items.isEmpty) {
              return PxEmptyState(icon: 'search', title: 'No results', text: 'Nothing matched “${req.query}”. Try different words or another filter.');
            }
            return PxTrackList(
              head: false,
              rows: [
                for (var i = 0; i < items.length; i++)
                  if (items[i].song != null)
                    PxTrackRow(song: items[i].song!, index: i, onPlay: () => actions.play([items[i].song!]))
                  else if (items[i].isArtist)
                    PxEntityRow(title: items[i].title!, subtitle: 'Artist · show songs', url: items[i].thumbnail, seed: items[i].key!, round: true, onTap: () => notifier.run(items[i].title!, kind: SearchKind.songs))
                  else
                    PxEntityRow(title: items[i].title!, subtitle: 'Playlist', url: items[i].thumbnail, seed: items[i].key!, onTap: () => toast.push('Opening playlists from search is not available yet.')),
              ],
            );
          },
        ),
      ],
    );
  }
}
