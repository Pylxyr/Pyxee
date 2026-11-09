import 'dart:async';
import 'dart:math' as math;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/shell/deck.dart';
import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/lists.dart';
import '../../shared/widgets.dart';
import '../settings/settings_sheet.dart';

/// Now Playing, full screen: the cover and controls on the left, and beside them Up next, Lyrics and Related. It slides
/// up over the shell and is not built at all while closed.
class PxNowPlaying extends ConsumerStatefulWidget {
  const PxNowPlaying({super.key});

  @override
  ConsumerState<PxNowPlaying> createState() => _PxNowPlayingState();
}

class _PxNowPlayingState extends ConsumerState<PxNowPlaying> with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(vsync: this, duration: const Duration(milliseconds: 360), reverseDuration: const Duration(milliseconds: 260));
  late final Animation<Offset> _slide = Tween(begin: const Offset(0, 0.08), end: Offset.zero).animate(CurvedAnimation(parent: _c, curve: Curves.easeOutExpo, reverseCurve: Curves.easeInCubic));

  @override
  void initState() {
    super.initState();
    _c.addStatusListener((_) {
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final open = ref.watch(navProvider.select((n) => n.npOpen));
    final hasSong = ref.watch(currentSongProvider) != null;
    final want = open && hasSong;
    // Started after the frame: starting an animation notifies listeners, which must not happen while building.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      if (want) {
        if (!_c.isCompleted && _c.status != AnimationStatus.forward) unawaited(_c.forward());
      } else if (!_c.isDismissed && _c.status != AnimationStatus.reverse) {
        unawaited(_c.reverse());
      }
    });
    if (_c.isDismissed && !want) return const SizedBox.shrink();
    final p = context.px;
    return FadeTransition(
      opacity: _c,
      child: SlideTransition(
        position: _slide,
        child: Container(
          color: Color.alphaBlend(p.accent.withValues(alpha: 0.14), p.solid),
          child: const _NpBody(),
        ),
      ),
    );
  }
}

class _NpBody extends ConsumerWidget {
  const _NpBody();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final song = ref.watch(currentSongProvider);
    if (song == null) return const SizedBox.shrink();
    final size = MediaQuery.sizeOf(context);
    final w = size.width;
    final pad = pagePad(w);
    final lg = w >= 1024;
    final sideWidth = w >= 1536 ? 704.0 : (w >= 1280 ? 544.0 : 448.0);
    final gap = w >= 1536 ? 64.0 : 40.0;
    final nav = ref.read(navProvider.notifier);
    return Stack(
      children: [
        PxBackdrop(url: song.thumbnail, seed: song.key, opacity: 0.5),
        Column(
          children: [
            Padding(
              padding: EdgeInsets.symmetric(horizontal: pad, vertical: 12),
              child: Row(
                children: [
                  PxIconButton('down', label: 'Close Now Playing', large: true, strong: true, onPressed: nav.closeNowPlaying),
                  const SizedBox(width: 8),
                  Text('Now playing', style: PxText.strong(p.muted)),
                ],
              ),
            ),
            Expanded(
              child: Padding(
                padding: EdgeInsets.fromLTRB(w >= 1536 ? 40 : 24, 0, w >= 1536 ? 40 : 24, lg ? 32 : 16),
                child: lg
                    ? Row(
                        children: [
                          const Expanded(child: _Controls()),
                          SizedBox(width: gap),
                          SizedBox(width: sideWidth, child: const _SidePanel()),
                        ],
                      )
                    : Column(
                        children: [
                          const _Controls(compact: true),
                          const SizedBox(height: 16),
                          const Expanded(child: _SidePanel()),
                        ],
                      ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _Controls extends ConsumerWidget {
  const _Controls({this.compact = false});

  final bool compact;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final song = ref.watch(currentSongProvider)!;
    final size = MediaQuery.sizeOf(context);
    final w = size.width;
    final actions = ref.read(actionsProvider);
    final shuffle = ref.watch(playerProvider.select((s) => s.shuffle));
    final repeat = ref.watch(playerProvider.select((s) => s.repeat));
    final failure = ref.watch(playerProvider.select((s) => s.transport.failure));
    final sleep = ref.watch(sleepProvider);
    final art = compact
        ? math.min(w * 0.56, size.height * 0.26)
        : (w >= 1536 ? math.min(640.0, size.height * 0.56) : (w >= 1280 ? math.min(480.0, size.height * 0.5) : math.min(352.0, size.height * 0.48)));
    final rowWidth = math.min(w * 0.8, w >= 1536 ? 640.0 : (w >= 1280 ? 480.0 : 352.0));
    final column = Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        PxArt(url: song.thumbnail, seed: song.key, size: art, radius: rMd),
        SizedBox(height: compact ? 12 : 20),
        SizedBox(
          width: rowWidth,
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(song.title, style: PxText.npTitle(w, p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
                    Text(song.artist, style: PxText.body(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
                  ],
                ),
              ),
              PxIconButton(
                'moon',
                label: 'Sleep timer',
                pressed: sleep != null,
                width: sleep == null ? null : 84,
                onPressed: () => unawaited(showSleepSheet(context, ref)),
                child: sleep == null
                    ? null
                    : Padding(padding: const EdgeInsets.only(left: 4), child: Text(sleep.label.replaceAll(' left', ''), style: PxText.caption(p.accent).copyWith(fontWeight: FontWeight.w600))),
              ),
              PxLikeButton(song: song),
            ],
          ),
        ),
        const SizedBox(height: 12),
        SizedBox(
          width: rowWidth,
          child: Column(
            children: [
              PxSeek(durationMs: song.durationMs),
              _NpTimes(durationMs: song.durationMs),
            ],
          ),
        ),
        if (failure != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Row(mainAxisSize: MainAxisSize.min, children: [Text(failure, style: PxText.strong(p.accent)), const SizedBox(width: 8), PxLink('Retry', onTap: actions.retry, color: p.accent)]),
          ),
        const SizedBox(height: 12),
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            PxIconButton('shuffle', label: 'Shuffle', large: true, pressed: shuffle, onPressed: () => actions.setShuffle(!shuffle)),
            const SizedBox(width: 16),
            PxIconButton('prev', label: 'Previous', large: true, strong: true, iconSize: 26, onPressed: actions.previous),
            const SizedBox(width: 16),
            const PxPlayButton(size: PxPlaySize.large, announceLoading: false),
            const SizedBox(width: 16),
            PxIconButton('next', label: 'Next', large: true, strong: true, iconSize: 26, onPressed: actions.next),
            const SizedBox(width: 16),
            PxIconButton(
              'repeat',
              label: 'Repeat: ${repeat.name}',
              large: true,
              pressed: repeat != PxRepeat.off,
              onPressed: () => actions.setRepeat(PxRepeat.values[(repeat.index + 1) % 3]),
              child: null,
            ),
            if (repeat == PxRepeat.one)
              Transform.translate(offset: const Offset(-24, -10), child: Text('1', style: PxText.caption(p.accent).copyWith(fontSize: 10, fontWeight: FontWeight.w600))),
          ],
        ),
      ],
    );
    // Centred while there is room, from the top when there is not: a short window scrolls instead of cutting the cover off.
    return LayoutBuilder(
      builder: (context, box) => SingleChildScrollView(
        child: ConstrainedBox(constraints: BoxConstraints(minHeight: box.maxHeight), child: Center(child: column)),
      ),
    );
  }
}

class _NpTimes extends ConsumerWidget {
  const _NpTimes({required this.durationMs});

  final int? durationMs;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final pos = ref.watch(positionProvider);
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(formatTime(pos), style: PxText.caption(p.muted)),
        Text(durationMs != null && durationMs! > 0 ? formatTime(durationMs) : 'length unknown', style: PxText.caption(p.muted)),
      ],
    );
  }
}

class _SidePanel extends ConsumerWidget {
  const _SidePanel();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final nav = ref.watch(navProvider);
    final notifier = ref.read(navProvider.notifier);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Align(
                alignment: Alignment.centerLeft,
                child: PxSegmented<NpTab>(
                  value: nav.npTab,
                  onSelect: notifier.setNpTab,
                  items: const [
                    (id: NpTab.upnext, label: 'Up next', disabled: false, tooltip: null),
                    (id: NpTab.lyrics, label: 'Lyrics', disabled: false, tooltip: null),
                    (id: NpTab.related, label: 'Related', disabled: false, tooltip: null),
                  ],
                ),
              ),
            ),
            PxButton(label: 'Open queue', small: true, kind: PxButtonKind.quiet, onPressed: () => notifier.closeNowPlaying(PxRoute.queue)),
          ],
        ),
        const SizedBox(height: 12),
        Expanded(
          child: switch (nav.npTab) {
            NpTab.upnext => const _UpNext(),
            NpTab.lyrics => const _Lyrics(),
            NpTab.related => const _Related(),
          },
        ),
      ],
    );
  }
}

class _UpNext extends ConsumerWidget {
  const _UpNext();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final player = ref.watch(playerProvider);
    final entry = player.currentEntry;
    if (entry == null) return const SizedBox.shrink();
    final upcoming = [for (final id in player.upcoming) ?player.entry(id)];
    final actions = ref.read(actionsProvider);
    final firstRadio = upcoming.where((e) => e.radio).map((e) => e.id).firstOrNull;
    return ListView(
      children: [
        Container(
          margin: const EdgeInsets.only(bottom: 2),
          decoration: BoxDecoration(color: p.selected, borderRadius: BorderRadius.circular(rSm), border: Border(left: BorderSide(color: p.accent, width: 3))),
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          child: Row(
            children: [
              PxThumb(url: entry.song.thumbnail, seed: entry.song.key, size: 44),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(entry.song.title, style: PxText.body(p.accent).copyWith(fontWeight: FontWeight.w600), maxLines: 1, overflow: TextOverflow.ellipsis),
                    Text('Now playing', style: PxText.caption(p.muted)),
                  ],
                ),
              ),
              Text(formatTime(entry.song.durationMs), style: PxText.caption(p.muted)),
            ],
          ),
        ),
        if (upcoming.isEmpty) Padding(padding: const EdgeInsets.all(24), child: Text('Nothing queued after this track.', style: PxText.body(p.muted))),
        ReorderableListView.builder(
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          buildDefaultDragHandles: false,
          itemCount: upcoming.length,
          proxyDecorator: (child, _, _) => Material(color: Colors.transparent, child: child),
          onReorder: (from, to) {
            final dest = to > from ? to - 1 : to;
            if (dest != from) actions.moveUpcoming(upcoming[from].id, dest);
          },
          itemBuilder: (context, i) {
            final e = upcoming[i];
            return Column(
              key: ValueKey(e.id),
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (e.id == firstRadio)
                  Padding(padding: const EdgeInsets.fromLTRB(12, 16, 12, 4), child: Text('Autoplay · similar songs', style: PxText.caption(p.muted))),
                PxEditableRow(
                  song: e.song,
                  coverSize: 44,
                  onPlay: () => actions.playEntry(e.id),
                  handle: ReorderableDragStartListener(index: i, child: const PxHandle()),
                  actions: [
                    PxIconButton('top', iconSize: 18, label: 'Play ${e.song.title} next', onPressed: i == 0 ? null : () => actions.moveUpcoming(e.id, 0)),
                    PxIconButton('up', iconSize: 18, label: 'Move ${e.song.title} up', onPressed: i == 0 ? null : () => actions.moveUpcoming(e.id, i - 1)),
                    PxIconButton('down', iconSize: 18, label: 'Move ${e.song.title} down', onPressed: i == upcoming.length - 1 ? null : () => actions.moveUpcoming(e.id, i + 1)),
                    PxIconButton('close', iconSize: 18, label: 'Remove ${e.song.title} from queue', onPressed: () => removeFromQueue(ref, e)),
                  ],
                ),
              ],
            );
          },
        ),
      ],
    );
  }
}

/// Removes a song from the queue, with Undo.
void removeFromQueue(WidgetRef ref, QueueEntry e) {
  final actions = ref.read(actionsProvider);
  actions.remove(e.id);
  ref.read(toastProvider.notifier).push('Removed "${e.song.title}"', actionLabel: 'Undo', onAction: actions.undo);
}

class _Related extends ConsumerWidget {
  const _Related();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final song = ref.watch(currentSongProvider);
    if (song == null) return Text('Play a song to see similar music.', style: PxText.body(p.muted));
    final value = ref.watch(relatedProvider(song.key));
    final actions = ref.read(actionsProvider);
    return ListView(
      children: [
        PxAsync<List<Song>>(
          value: value,
          skeletonRows: 5,
          onRetry: () => ref.invalidate(relatedProvider(song.key)),
          builder: (tracks) => tracks.isEmpty
              ? Padding(padding: const EdgeInsets.all(12), child: Text('No related songs found for this one.', style: PxText.body(p.muted)))
              : Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Padding(
                      padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
                      child: Row(
                        children: [
                          Expanded(child: Text('Similar to this song', style: PxText.strong(p.fg))),
                          PxButton(label: 'Play all', small: true, kind: PxButtonKind.primary, onPressed: () => actions.play(tracks)),
                        ],
                      ),
                    ),
                    PxTrackList(tracks: tracks, head: false, onPlay: (i) => actions.play(tracks, i)),
                  ],
                ),
        ),
      ],
    );
  }
}

class _Lyrics extends ConsumerStatefulWidget {
  const _Lyrics();

  @override
  ConsumerState<_Lyrics> createState() => _LyricsState();
}

class _LyricsState extends ConsumerState<_Lyrics> {
  final _keys = <int, GlobalKey>{};
  int _last = -2;
  DateTime _byHand = DateTime.fromMillisecondsSinceEpoch(0);

  int _currentLine(List<LyricLine> lines, int ms) {
    var index = -1;
    for (var i = 0; i < lines.length; i++) {
      if (lines[i].timeMs <= ms) {
        index = i;
      } else {
        break;
      }
    }
    return index;
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final song = ref.watch(currentSongProvider);
    if (song == null) return Text('Play a song to see its lyrics.', style: PxText.body(p.muted));
    final value = ref.watch(lyricsProvider(song));
    return value.when(
      loading: () => Padding(padding: const EdgeInsets.all(12), child: Text('Looking for lyrics…', style: PxText.body(p.muted))),
      error: (e, _) => Padding(
        padding: const EdgeInsets.all(12),
        child: Row(children: [Flexible(child: Text('Could not get the lyrics (${errorText(e)}).', style: PxText.strong(p.accent))), const SizedBox(width: 8), PxLink('Try again', onTap: () => ref.invalidate(lyricsProvider(song)))]),
      ),
      data: (lyrics) => switch (lyrics.kind) {
        'synced' => _synced(context, lyrics.lines),
        'plain' => ListView(
          padding: const EdgeInsets.all(12),
          children: [
            Text(lyrics.text, style: PxText.lead(p.fg).copyWith(fontWeight: FontWeight.w400)),
            const SizedBox(height: 16),
            Text('These lyrics are not timed, so they do not follow the song.', style: PxText.caption(p.muted)),
          ],
        ),
        'instrumental' => Padding(padding: const EdgeInsets.all(12), child: Text('This one is instrumental, so there are no lyrics.', style: PxText.body(p.muted))),
        'notFound' => Padding(padding: const EdgeInsets.all(12), child: Text('No lyrics found for this song.', style: PxText.body(p.muted))),
        _ => Padding(
          padding: const EdgeInsets.all(12),
          child: Row(children: [Flexible(child: Text('Could not get the lyrics (${lyrics.text}).', style: PxText.strong(p.accent))), const SizedBox(width: 8), PxLink('Try again', onTap: () => ref.invalidate(lyricsProvider(song)))]),
        ),
      },
    );
  }

  Widget _synced(BuildContext context, List<LyricLine> lines) {
    final p = context.px;
    final pos = ref.watch(positionProvider);
    final index = _currentLine(lines, pos);
    if (index != _last) {
      _last = index;
      if (index >= 0 && DateTime.now().difference(_byHand) > const Duration(seconds: 4)) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          final ctx = _keys[index]?.currentContext;
          if (ctx != null) unawaited(Scrollable.ensureVisible(ctx, alignment: 0.5, duration: const Duration(milliseconds: 360), curve: Curves.easeOutCubic));
        });
      }
    }
    return Listener(
      onPointerDown: (_) => _byHand = DateTime.now(),
      onPointerSignal: (_) => _byHand = DateTime.now(),
      child: ListView.builder(
        padding: const EdgeInsets.fromLTRB(12, 16, 12, 96),
        itemCount: lines.length,
        itemBuilder: (context, i) {
          final key = _keys.putIfAbsent(i, GlobalKey.new);
          final d = index < 0 ? 0 : math.min((i - index).abs(), 4);
          final isNow = i == index;
          final color = isNow ? p.fg : (i < index ? p.muted.withValues(alpha: 0.6) : p.muted);
          return Padding(
            key: key,
            padding: const EdgeInsets.only(bottom: 12),
            child: PxPress(
              onTap: () => ref.read(actionsProvider).seek(lines[i].timeMs),
              builder: (context, hovered, _) => AnimatedOpacity(
                duration: const Duration(milliseconds: 240),
                opacity: isNow || hovered ? 1 : 1 - d * 0.16,
                child: AnimatedScale(
                  scale: isNow ? 1.05 : 1,
                  alignment: Alignment.centerLeft,
                  duration: const Duration(milliseconds: 240),
                  child: Text(lines[i].text.isEmpty ? '♪' : lines[i].text, style: PxText.subtitle(color)),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}
