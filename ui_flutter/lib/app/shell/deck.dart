import 'dart:async';
import 'dart:ui' show FontFeature;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/logo.dart';
import '../../shared/widgets.dart';
import '../state/ui_state.dart';
import '../theme/px_theme.dart';

/// The song that is playing, or null.
final _entryProvider = Provider<QueueEntry?>((ref) => ref.watch(playerProvider.select((s) => s.currentEntry)));

class _Failure extends ConsumerWidget {
  const _Failure();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final failure = ref.watch(playerProvider.select((s) => s.transport.failure));
    if (failure == null) return const SizedBox.shrink();
    return Row(
      children: [
        Flexible(child: Text(failure, style: PxText.caption(p.accent).copyWith(fontWeight: FontWeight.w600), maxLines: 1, overflow: TextOverflow.ellipsis)),
        const SizedBox(width: 8),
        PxButton(label: 'Retry', small: true, kind: PxButtonKind.primary, onPressed: ref.read(actionsProvider).retry),
      ],
    );
  }
}

/// The deck along the bottom of a wide window: what is playing, the transport, volume, and a seek line that runs the
/// full width of the window along its top edge. Before anything plays it says so and keeps its height.
class PxDeck extends ConsumerWidget {
  const PxDeck({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final entry = ref.watch(_entryProvider);
    final pad = pagePad(MediaQuery.sizeOf(context).width);
    final actions = ref.read(actionsProvider);
    final nav = ref.read(navProvider.notifier);
    final failed = ref.watch(playerProvider.select((s) => s.transport.failed));
    if (entry == null) {
      return Container(
        height: 88,
        padding: EdgeInsets.symmetric(horizontal: pad),
        decoration: BoxDecoration(color: p.surface, border: Border(top: BorderSide(color: p.rule))),
        child: Row(children: [const PxLogo(size: 24), const SizedBox(width: 12), Text('Nothing playing. Pick a song and it shows up here.', style: PxText.body(p.muted))]),
      );
    }
    final song = entry.song;
    return Stack(
      clipBehavior: Clip.none,
      children: [
        GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () => nav.go(PxRoute.queue),
          child: Container(
            height: 88,
            padding: EdgeInsets.symmetric(horizontal: pad),
            decoration: BoxDecoration(color: p.surface, border: Border(top: BorderSide(color: p.rule))),
            child: Row(
              children: [
                Expanded(
                  child: Row(
                    children: [
                      PxPress(
                        onTap: nav.openNowPlaying,
                        semanticLabel: 'Open Now Playing',
                        builder: (context, _, _) => PxArt(url: song.thumbnail, seed: song.key, size: 64),
                      ),
                      const SizedBox(width: 14),
                      Expanded(
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(song.title, style: PxText.strong(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
                            if (failed) const _Failure() else Text(song.artist, style: PxText.body(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
                          ],
                        ),
                      ),
                      PxLikeButton(song: song),
                    ],
                  ),
                ),
                const SizedBox(width: 24),
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    PxIconButton('prev', label: 'Previous', large: true, strong: true, iconSize: 24, onPressed: actions.previous),
                    const SizedBox(width: 12),
                    const PxPlayButton(),
                    const SizedBox(width: 12),
                    PxIconButton('next', label: 'Next', large: true, strong: true, iconSize: 24, onPressed: actions.next),
                    const SizedBox(width: 12),
                    const _Time(),
                  ],
                ),
                const SizedBox(width: 24),
                Expanded(
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      const SizedBox(width: 128, child: PxVolume()),
                      const SizedBox(width: 8),
                      PxIconButton('expand', label: 'Expand Now Playing', onPressed: nav.openNowPlaying),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
        Positioned(left: 0, right: 0, top: -6, height: 20, child: PxSeek(durationMs: song.durationMs)),
      ],
    );
  }
}

class _Time extends ConsumerWidget {
  const _Time();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final pos = ref.watch(positionProvider);
    final dur = ref.watch(currentSongProvider.select((s) => s?.durationMs));
    return SizedBox(
      width: 104,
      child: Text(
        '${formatTime(pos)} / ${dur != null && dur > 0 ? formatTime(dur) : '--:--'}',
        textAlign: TextAlign.center,
        style: PxText.caption(p.muted).copyWith(fontFeatures: const [FontFeature.tabularFigures()]),
      ),
    );
  }
}

/// The heart for the song that is playing.
class PxLikeButton extends ConsumerWidget {
  const PxLikeButton({super.key, required this.song, this.large = false});

  final Song song;
  final bool large;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final liked = ref.watch(libraryProvider.select((l) => l.likedKeys.contains(song.key)));
    return PxIconButton(
      'heart',
      fill: liked,
      pressed: liked,
      large: large,
      label: liked ? 'Unlike' : 'Like',
      onPressed: () => unawaited(ref.read(apiProvider).setLiked(song, !liked)),
    );
  }
}

/// The player of a narrow window: one strip above the bottom bar with the seek line along its top edge.
class PxDock extends ConsumerWidget {
  const PxDock({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final entry = ref.watch(_entryProvider);
    if (entry == null) return const SizedBox.shrink();
    final song = entry.song;
    final actions = ref.read(actionsProvider);
    final nav = ref.read(navProvider.notifier);
    final failure = ref.watch(playerProvider.select((s) => s.transport.failure));
    return Stack(
      clipBehavior: Clip.none,
      children: [
        GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () => nav.go(PxRoute.queue),
          child: Container(
            height: 64,
            padding: const EdgeInsets.fromLTRB(12, 0, 8, 0),
            decoration: BoxDecoration(color: p.surface, border: Border(top: BorderSide(color: p.line))),
            child: Row(
              children: [
                Expanded(
                  child: PxPress(
                    onTap: nav.openNowPlaying,
                    semanticLabel: 'Open Now Playing',
                    builder: (context, _, _) => Row(
                      children: [
                        PxThumb(url: song.thumbnail, seed: song.key, size: 44),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(song.title, style: PxText.strong(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
                              Text(failure ?? song.artist, style: PxText.caption(failure != null ? p.accent : p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                if (failure != null) PxButton(label: 'Retry', small: true, kind: PxButtonKind.primary, onPressed: actions.retry),
                PxIconButton('prev', label: 'Previous', large: true, strong: true, iconSize: 24, onPressed: actions.previous),
                const PxPlayButton(),
                PxIconButton('next', label: 'Next', large: true, strong: true, iconSize: 24, onPressed: actions.next),
              ],
            ),
          ),
        ),
        Positioned(left: 0, right: 0, top: -6, height: 20, child: PxSeek(durationMs: song.durationMs)),
      ],
    );
  }
}

/// From 1440 px the queue lane sits beside the page, so what comes next stays in sight while you browse.
class PxLane extends ConsumerWidget {
  const PxLane({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final player = ref.watch(playerProvider);
    final entry = player.currentEntry;
    if (entry == null) return const SizedBox.shrink();
    final width = (MediaQuery.sizeOf(context).width * 0.22).clamp(320.0, 480.0);
    final upcoming = [for (final id in player.upcoming) ?player.entry(id)];
    final actions = ref.read(actionsProvider);
    return Container(
      width: width,
      decoration: BoxDecoration(
        border: Border(left: BorderSide(color: p.rule)),
        color: p.layer,
        gradient: LinearGradient(begin: Alignment.topCenter, end: Alignment.bottomCenter, colors: [p.stageWash, Colors.transparent], stops: const [0, 0.45]),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 14, 16, 8),
            child: Row(
              children: [
                Expanded(child: Text('Up next', style: PxText.lead(p.fg))),
                PxButton(label: 'Open queue', small: true, kind: PxButtonKind.quiet, onPressed: () => ref.read(navProvider.notifier).go(PxRoute.queue)),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(10, 0, 10, 12),
              children: [
                Container(
                  margin: const EdgeInsets.only(bottom: 4),
                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                  decoration: BoxDecoration(color: p.selected, borderRadius: BorderRadius.circular(rMd), border: Border(left: BorderSide(color: p.accent, width: 3))),
                  child: _LaneRow(song: entry.song, subtitle: 'Now playing', current: true),
                ),
                for (final e in upcoming)
                  PxPress(
                    onTap: () => actions.playEntry(e.id),
                    builder: (context, hovered, _) => Container(
                      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                      decoration: BoxDecoration(color: hovered ? p.hover : Colors.transparent, borderRadius: BorderRadius.circular(rSm)),
                      child: _LaneRow(song: e.song, subtitle: e.song.artist),
                    ),
                  ),
                if (upcoming.isEmpty)
                  Padding(padding: const EdgeInsets.all(12), child: Text('Nothing queued after this song.', style: PxText.caption(p.muted))),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _LaneRow extends StatelessWidget {
  const _LaneRow({required this.song, required this.subtitle, this.current = false});

  final Song song;
  final String subtitle;
  final bool current;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Row(
      children: [
        PxThumb(url: song.thumbnail, seed: song.key, size: 40),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(song.title, style: PxText.body(current ? p.accent : p.fg).copyWith(fontWeight: current ? FontWeight.w600 : FontWeight.w400), maxLines: 1, overflow: TextOverflow.ellipsis),
              Text(subtitle, style: PxText.caption(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
            ],
          ),
        ),
        const SizedBox(width: 8),
        Text(formatTime(song.durationMs), style: PxText.caption(p.muted)),
      ],
    );
  }
}
