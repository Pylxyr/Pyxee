import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/lists.dart';
import '../../shared/widgets.dart';

/// Home: the hero (what is playing, or the last song) over "On rotation" and "Recently played".
class HomePage extends ConsumerWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final home = ref.watch(homeProvider);
    final actions = ref.read(actionsProvider);
    final List<Widget> children = home.when(
      loading: () => const [PxSectionHead('On rotation', first: true), PxSkeleton(rows: 8)],
      error: (e, _) => [
        PxNotice(children: [Text("Couldn't load Home. ${errorText(e)}", style: PxText.body(context.px.fg))]),
      ],
      data: (h) {
        if (h.quickPicks.isEmpty && h.recent.isEmpty) {
          return const [PxEmptyState(icon: 'music', title: 'Nothing here yet', text: 'Search for something to play. Songs you listen to and like will fill this page with quick picks.')];
        }
        return [
          _Hero(home: h),
          const SizedBox(height: 36),
          if (h.quickPicks.isNotEmpty) ...[
            PxSectionHead('On rotation', count: h.quickPicks.length, first: true),
            _Picks(songs: h.quickPicks, onPlay: (i) => actions.play(h.quickPicks, i)),
            const SizedBox(height: 36),
          ],
          if (h.recent.isNotEmpty)
            PxShelf(
              title: 'Recently played',
              first: true,
              tiles: [
                for (var i = 0; i < h.recent.length; i++)
                  PxTileData(title: h.recent[i].title, subtitle: h.recent[i].artist, url: h.recent[i].thumbnail, seed: h.recent[i].key, onTap: () => actions.play(h.recent, i)),
              ],
            ),
        ];
      },
    );
    return PxScrollPage(header: const PxPageHeader(title: 'Home'), children: children);
  }
}

class _Hero extends ConsumerWidget {
  const _Hero({required this.home});

  final HomeData home;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final player = ref.watch(playerProvider);
    final actions = ref.read(actionsProvider);
    final width = MediaQuery.sizeOf(context).width;
    final transport = player.transport;
    final now = player.currentEntry?.song;
    Song? song;
    String label = 'Pick up where you left off';
    String button = 'Resume';
    String? icon = 'play';
    VoidCallback onTap;
    if (now != null && transport.state != 'idle') {
      song = now;
      if (transport.playing || transport.busy) {
        label = 'Now playing';
        button = 'Pause';
        icon = 'pause';
        onTap = actions.toggle;
      } else if (transport.failed) {
        label = "Couldn't play this song";
        button = 'Try again';
        icon = null;
        onTap = actions.retry;
      } else {
        onTap = actions.toggle;
      }
    } else if (home.recent.isNotEmpty) {
      song = home.recent.first;
      onTap = () => actions.play(home.recent);
    } else {
      return const SizedBox.shrink();
    }
    final art = (width * 0.13).clamp(150.0, 240.0);
    return Container(
      constraints: BoxConstraints(minHeight: (width * 0.22).clamp(220.0, 320.0)),
      decoration: BoxDecoration(color: p.surface, borderRadius: BorderRadius.circular(rLg), border: Border.all(color: p.rule)),
      clipBehavior: Clip.antiAlias,
      child: Stack(
        children: [
          PxBackdrop(url: song.thumbnail, seed: song.key, opacity: 0.55),
          Positioned.fill(
            child: DecoratedBox(
              decoration: BoxDecoration(gradient: LinearGradient(colors: [p.layer.withValues(alpha: 0.78), Colors.transparent], stops: const [0, 0.85])),
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(28),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                PxArt(url: song.thumbnail, seed: song.key, size: art, radius: rMd),
                const SizedBox(width: 28),
                Expanded(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(label, style: PxText.body(p.accent).copyWith(fontSize: 13, fontWeight: FontWeight.w600)),
                      const SizedBox(height: 4),
                      Text(song.title, style: PxText.heroTitle(width, p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
                      Text(song.artist, style: PxText.body(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
                      const SizedBox(height: 16),
                      Wrap(
                        spacing: 8,
                        runSpacing: 8,
                        children: [
                          PxButton(label: button, kind: PxButtonKind.primary, icon: icon, iconFill: icon == 'play', onPressed: onTap),
                          if (home.likedCount > 0)
                            PxButton(
                              label: 'Liked songs',
                              trailing: Text('${home.likedCount}', style: PxText.body(p.muted)),
                              onPressed: () => ref.read(navProvider.notifier).openLibrary(LibraryTab.liked),
                            ),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// "On rotation": the table's rows in as many columns as fit.
class _Picks extends StatelessWidget {
  const _Picks({required this.songs, required this.onPlay});

  final List<Song> songs;
  final void Function(int index) onPlay;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      const gap = 32.0;
      final cols = ((box.maxWidth + gap) / (340 + gap)).floor().clamp(1, 6);
      final w = (box.maxWidth - gap * (cols - 1)) / cols;
      // Row by row, as a grid fills: 1 2 3 / 4 5 6.
      return Wrap(
        spacing: gap,
        children: [
          for (var i = 0; i < songs.length; i++)
            SizedBox(
              width: w,
              child: PxTrackRow(song: songs[i], index: i, onPlay: () => onPlay(i)),
            ),
        ],
      );
    },
  );
}
