import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/lists.dart';
import '../../shared/sheets.dart';
import '../../shared/widgets.dart';

/// Library: liked songs, playlists, history and downloads.
class LibraryPage extends ConsumerWidget {
  const LibraryPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tab = ref.watch(navProvider.select((n) => n.libraryTab));
    final nav = ref.read(navProvider.notifier);
    const labels = {
      LibraryTab.liked: 'Liked songs',
      LibraryTab.playlists: 'Playlists',
      LibraryTab.history: 'History',
      LibraryTab.downloads: 'Downloads',
    };
    return PxScrollPage(
      header: PxPageHeader(
        title: 'Library',
        below: PxSegmented<LibraryTab>(
          value: tab,
          onSelect: nav.setLibraryTab,
          items: [for (final e in labels.entries) (id: e.key, label: e.value, disabled: false, tooltip: null)],
        ),
      ),
      children: switch (tab) {
        LibraryTab.liked => _liked(context, ref),
        LibraryTab.playlists => _playlists(context, ref),
        LibraryTab.history => _history(context, ref),
        LibraryTab.downloads => const [PxEmptyState(icon: 'library', title: 'Downloads', text: 'Songs saved for offline listening live here. Downloads arrive in a later release.')],
      },
    );
  }

  Widget _toolbar(BuildContext context, List<Widget> left, [Widget? right]) {
    final p = context.px;
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 12),
      margin: const EdgeInsets.only(bottom: 4),
      decoration: BoxDecoration(border: Border(bottom: BorderSide(color: p.rule))),
      child: Row(children: [...left, const Spacer(), ?right]),
    );
  }

  List<Widget> _liked(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final liked = ref.watch(likedProvider);
    final actions = ref.read(actionsProvider);
    return [
      liked.when(
        loading: () => const PxSkeleton(rows: 6),
        error: (e, _) => Text(errorText(e), style: PxText.strong(p.accent)),
        data: (songs) {
          if (songs.isEmpty) return const PxEmptyState(icon: 'heart', title: 'No liked songs yet', text: 'Tap the heart on a song and it will show up here.');
          void playAll(bool shuffle) {
            if (shuffle != ref.read(playerProvider).shuffle) actions.setShuffle(shuffle);
            actions.play(songs);
          }

          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _toolbar(
                context,
                [
                  PxButton(label: 'Play all', kind: PxButtonKind.primary, onPressed: () => playAll(false)),
                  const SizedBox(width: 8),
                  PxButton(label: 'Shuffle', onPressed: () => playAll(true)),
                ],
                Text(songsCount(songs.length), style: PxText.body(p.muted)),
              ),
              PxTrackList(tracks: songs, onPlay: (i) => actions.play(songs, i)),
            ],
          );
        },
      ),
    ];
  }

  List<Widget> _playlists(BuildContext context, WidgetRef ref) {
    final playlists = ref.watch(libraryProvider.select((l) => l.playlists));
    final nav = ref.read(navProvider.notifier);
    final api = ref.read(apiProvider);
    void create() => unawaited(
      showNameSheet(
        context,
        title: 'New playlist',
        confirm: 'Create',
        run: (name) async {
          final id = await api.createPlaylist(name.trim(), const []);
          nav.openPlaylist(id);
        },
      ),
    );
    return [
      _toolbar(context, [PxButton(label: 'New playlist', icon: 'plus', kind: PxButtonKind.primary, onPressed: create)]),
      if (playlists.isEmpty)
        const PxEmptyState(icon: 'library', title: 'No playlists yet', text: 'Create one here, or choose Add to playlist on any song. You can also save your queue.')
      else
        Padding(
          padding: const EdgeInsets.only(top: 16),
          child: PxCoverGrid(tiles: [
            for (final pl in playlists) PxTileData(title: pl.name, subtitle: songsCount(pl.trackCount), url: pl.thumbnail, seed: 'pl:${pl.id}', onTap: () => nav.openPlaylist(pl.id)),
          ]),
        ),
    ];
  }

  List<Widget> _history(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final history = ref.watch(historyProvider);
    final count = ref.watch(libraryProvider.select((l) => l.historyCount));
    final actions = ref.read(actionsProvider);
    final api = ref.read(apiProvider);
    final toast = ref.read(toastProvider.notifier);
    return [
      history.when(
        loading: () => const PxSkeleton(rows: 6),
        error: (e, _) => Text(errorText(e), style: PxText.strong(p.accent)),
        data: (items) {
          if (items.isEmpty) return const PxEmptyState(icon: 'music', title: 'No history yet', text: 'Songs you listen to for a while are listed here.');
          Future<void> clear() async {
            try {
              final removed = await api.clearHistory();
              toast.push('History cleared', actionLabel: 'Undo', onAction: () => unawaited(api.restoreHistory(removed)));
            } on Object catch (e) {
              toast.push(errorText(e));
            }
          }

          final groups = _groupByDay(items);
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _toolbar(context, [Text('$count ${count == 1 ? 'play' : 'plays'}', style: PxText.body(p.muted))], PxButton(label: 'Clear history', onPressed: () => unawaited(clear()))),
              for (var g = 0; g < groups.length; g++) ...[
                PxSectionHead(groups[g].$1),
                PxTrackList(
                  head: g == 0,
                  rows: [for (final h in groups[g].$2) PxTrackRow(song: h.song, onPlay: () => actions.play([h.song]))],
                ),
              ],
            ],
          );
        },
      ),
    ];
  }
}

/// Newest-first items under "Today", "Yesterday" or a date, by the local calendar day.
List<(String, List<HistoryItem>)> _groupByDay(List<HistoryItem> items) {
  const weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  final now = DateTime.now();
  final today = DateTime(now.year, now.month, now.day);
  final groups = <(String, List<HistoryItem>)>[];
  for (final item in items) {
    final at = DateTime.fromMillisecondsSinceEpoch(item.playedAt);
    final day = DateTime(at.year, at.month, at.day);
    final diff = today.difference(day).inDays;
    final label = diff == 0
        ? 'Today'
        : (diff == 1 ? 'Yesterday' : '${weekdays[day.weekday - 1]}, ${months[day.month - 1]} ${day.day}${diff > 300 ? ', ${day.year}' : ''}');
    if (groups.isNotEmpty && groups.last.$1 == label) {
      groups.last.$2.add(item);
    } else {
      groups.add((label, [item]));
    }
  }
  return groups;
}
