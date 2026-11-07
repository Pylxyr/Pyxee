import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/theme/metrics.dart';
import '../../core/theme/prefs.dart';
import '../../core/models/models.dart';
import '../../core/state/providers.dart';
import '../../shared/widgets/basics.dart';
import 'orbit_state.dart';

/// The full-screen player: the cover and controls, and beside or under them the queue, the lyrics and what is related.
class OrbitView extends ConsumerWidget {
  const OrbitView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    final split = MediaQuery.sizeOf(context).width >= Metrics.orbitSplit;
    const tabs = _OrbitTabs();
    return SizedBox.expand(
      child: Material(
        color: scheme.surface,
        child: Column(
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: Padding(
                padding: const EdgeInsets.all(8),
                child: IconButton(
                  onPressed: () => ref.read(orbitOpenProvider.notifier).close(),
                  tooltip: 'Close',
                  icon: const Icon(Icons.keyboard_arrow_down),
                ),
              ),
            ),
            Expanded(
              child: split
                  ? const Row(
                      children: [
                        Expanded(child: _Deck(compact: false)),
                        Expanded(child: tabs),
                      ],
                    )
                  : const Column(
                      children: [
                        _Deck(compact: true),
                        Expanded(child: tabs),
                      ],
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Cover, title, seek bar, transport and volume.
class _Deck extends ConsumerWidget {
  const _Deck({required this.compact});

  final bool compact;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final song = ref.watch(currentSongProvider);
    final cover = compact ? 96.0 : 320.0;
    final info = Column(
      crossAxisAlignment: compact ? CrossAxisAlignment.start : CrossAxisAlignment.center,
      children: [
        Text(
          song?.title ?? 'Nothing playing',
          style: compact ? theme.textTheme.titleMedium : theme.textTheme.headlineSmall,
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          textAlign: compact ? TextAlign.start : TextAlign.center,
        ),
        Text(song?.artist ?? '', style: theme.textTheme.bodyMedium, maxLines: 1, overflow: TextOverflow.ellipsis),
      ],
    );
    final controls = Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        _Seek(durationMs: song?.durationMs),
        const _Transport(),
        if (!compact) const _VolumeRow(),
      ],
    );
    final body = compact
        ? Column(
            children: [
              Row(
                spacing: 16,
                children: [
                  PxArt(url: song?.thumbnail, size: cover, radius: 16),
                  Expanded(child: info),
                ],
              ),
              controls,
            ],
          )
        : Column(
            mainAxisAlignment: MainAxisAlignment.center,
            spacing: 20,
            children: [PxArt(url: song?.thumbnail, size: cover, radius: 24), info, controls],
          );
    return Padding(padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 8), child: body);
  }
}

class _Seek extends ConsumerStatefulWidget {
  const _Seek({required this.durationMs});

  final int? durationMs;

  @override
  ConsumerState<_Seek> createState() => _SeekState();
}

class _SeekState extends ConsumerState<_Seek> {
  double? _drag;

  @override
  Widget build(BuildContext context) {
    final position = ref.watch(positionProvider);
    final total = widget.durationMs;
    if (total == null || total <= 0) {
      return Row(children: [Text(clock(position)), const Expanded(child: Slider(value: 0, onChanged: null))]);
    }
    final value = (_drag ?? position.toDouble()).clamp(0.0, total.toDouble());
    return RepaintBoundary(
      child: Row(
        children: [
          SizedBox(width: 44, child: Text(clock(value.round()))),
          Expanded(
            child: Slider(
              value: value,
              max: total.toDouble(),
              onChanged: (v) => setState(() => _drag = v),
              onChangeEnd: (v) {
                ref.read(actionsProvider).seek(v.round());
                setState(() => _drag = null);
              },
            ),
          ),
          SizedBox(width: 44, child: Text(clock(total), textAlign: TextAlign.end)),
        ],
      ),
    );
  }
}

class _Transport extends ConsumerWidget {
  const _Transport();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shuffle = ref.watch(playerProvider.select((p) => p.shuffle));
    final repeat = ref.watch(playerProvider.select((p) => p.repeat));
    final playing = ref.watch(isPlayingProvider);
    final a = ref.read(actionsProvider);
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        IconButton(
          onPressed: () => a.setShuffle(!shuffle),
          tooltip: 'Shuffle',
          isSelected: shuffle,
          icon: const Icon(Icons.shuffle),
        ),
        IconButton(onPressed: a.previous, tooltip: 'Previous', icon: const Icon(Icons.skip_previous)),
        IconButton.filled(
          onPressed: a.toggle,
          tooltip: playing ? 'Pause' : 'Play',
          iconSize: 32,
          icon: Icon(playing ? Icons.pause : Icons.play_arrow),
        ),
        IconButton(onPressed: a.next, tooltip: 'Next', icon: const Icon(Icons.skip_next)),
        IconButton(
          onPressed: () => a.setRepeat(RepeatMode.values[(repeat.index + 1) % RepeatMode.values.length]),
          tooltip: 'Repeat: ${repeat.name}',
          isSelected: repeat != RepeatMode.off,
          icon: Icon(repeat == RepeatMode.one ? Icons.repeat_one : Icons.repeat),
        ),
      ],
    );
  }
}

class _VolumeRow extends ConsumerWidget {
  const _VolumeRow();

  @override
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox(width: 280, child: VolumeSlider());
}

// ------------------------------------------------------------------ tabs

class _OrbitTabs extends StatelessWidget {
  const _OrbitTabs();

  @override
  Widget build(BuildContext context) {
    return const DefaultTabController(
      length: 3,
      child: Column(
        children: [
          TabBar(tabs: [Tab(text: 'Queue'), Tab(text: 'Lyrics'), Tab(text: 'Related')]),
          Expanded(child: TabBarView(children: [_QueueTab(), _LyricsTab(), _RelatedTab()])),
        ],
      ),
    );
  }
}

class _QueueTab extends ConsumerWidget {
  const _QueueTab();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final player = ref.watch(playerProvider);
    final a = ref.read(actionsProvider);
    final current = player.currentEntry;
    final upcoming = [
      for (final id in player.upcoming)
        if (player.entry(id) case final e?) e,
    ];
    if (player.queue.isEmpty) return const PxMessage('The queue is empty.');
    return Column(
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            if (player.canUndo) TextButton.icon(onPressed: a.undo, icon: const Icon(Icons.undo), label: const Text('Undo')),
            TextButton(onPressed: a.clear, child: const Text('Clear')),
            Switch(value: player.autoplay, onChanged: a.setAutoplay),
            const Padding(padding: EdgeInsets.only(right: 12), child: Text('Autoplay')),
          ],
        ),
        if (current != null)
          ListTile(
            leading: PxArt(url: current.song.thumbnail, size: 44, radius: 8),
            title: Text(current.song.title, maxLines: 1, overflow: TextOverflow.ellipsis),
            subtitle: Text(current.song.artist, maxLines: 1, overflow: TextOverflow.ellipsis),
            trailing: const Icon(Icons.equalizer),
          ),
        Align(
          alignment: Alignment.centerLeft,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
            child: Text('Up next', style: Theme.of(context).textTheme.labelLarge),
          ),
        ),
        Expanded(
          child: upcoming.isEmpty
              ? const PxMessage('Nothing is up next.')
              : ReorderableListView.builder(
                  buildDefaultDragHandles: false,
                  itemCount: upcoming.length,
                  itemExtent: 64,
                  onReorderItem: (oldIndex, newIndex) => a.moveUpcoming(upcoming[oldIndex].id, newIndex),
                  itemBuilder: (context, i) {
                    final e = upcoming[i];
                    return ListTile(
                      key: ValueKey(e.id),
                      onTap: () => a.playEntry(e.id),
                      leading: PxArt(url: e.song.thumbnail, size: 44, radius: 8),
                      title: Text(e.song.title, maxLines: 1, overflow: TextOverflow.ellipsis),
                      subtitle: Text(e.song.artist, maxLines: 1, overflow: TextOverflow.ellipsis),
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          IconButton(
                            onPressed: () => a.remove(e.id),
                            tooltip: 'Remove',
                            icon: const Icon(Icons.close),
                          ),
                          ReorderableDragStartListener(index: i, child: const Icon(Icons.drag_handle)),
                        ],
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _RelatedTab extends ConsumerWidget {
  const _RelatedTab();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final song = ref.watch(currentSongProvider);
    if (song == null) return const PxMessage('Play something to see related songs.');
    return PxAsync<List<Song>>(
      value: ref.watch(relatedProvider(song.key)),
      onRetry: () => ref.invalidate(relatedProvider(song.key)),
      builder: (songs) => PxSongList(
        songs: songs,
        empty: 'No related songs found.',
        onPlay: (i) => ref.read(actionsProvider).play(songs, i),
      ),
    );
  }
}

class _LyricsTab extends ConsumerWidget {
  const _LyricsTab();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final song = ref.watch(currentSongProvider);
    if (song == null) return const PxMessage('Play something to see its lyrics.');
    return PxAsync<LyricsData>(
      value: ref.watch(lyricsProvider(song)),
      onRetry: () => ref.invalidate(lyricsProvider(song)),
      builder: (lyrics) => switch (lyrics.kind) {
        'synced' => _SyncedLyrics(lines: lyrics.lines),
        'plain' => SingleChildScrollView(padding: const EdgeInsets.all(24), child: Text(lyrics.text)),
        'instrumental' => const PxMessage('Instrumental.'),
        'error' => PxMessage(lyrics.text, onRetry: () => ref.invalidate(lyricsProvider(song))),
        _ => const PxMessage('No lyrics found for this song.'),
      },
    );
  }
}

int lyricIndex(List<LyricLine> lines, int ms) {
  var lo = 0;
  var hi = lines.length - 1;
  var found = -1;
  while (lo <= hi) {
    final mid = (lo + hi) ~/ 2;
    if (lines[mid].timeMs <= ms) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return found;
}

/// The lines, with the current one highlighted. Watching the position through `select` means this rebuilds (and
/// scrolls) only when the line changes, not four times a second.
class _SyncedLyrics extends ConsumerStatefulWidget {
  const _SyncedLyrics({required this.lines});

  final List<LyricLine> lines;

  @override
  ConsumerState<_SyncedLyrics> createState() => _SyncedLyricsState();
}

class _SyncedLyricsState extends ConsumerState<_SyncedLyrics> {
  static const _extent = 48.0;
  final _scroll = ScrollController();

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  void _follow(int index) {
    if (index < 0) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_scroll.hasClients) return;
      final target = (index * _extent - _scroll.position.viewportDimension / 2 + _extent / 2)
          .clamp(0.0, _scroll.position.maxScrollExtent);
      unawaited(_scroll.animateTo(target, duration: const Duration(milliseconds: 300), curve: Curves.easeOut));
    });
  }

  @override
  Widget build(BuildContext context) {
    final lines = widget.lines;
    final index = ref.watch(positionProvider.select((ms) => lyricIndex(lines, ms)));
    _follow(index);
    final theme = Theme.of(context);
    return ListView.builder(
      controller: _scroll,
      itemCount: lines.length,
      itemExtent: _extent,
      padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 8),
      itemBuilder: (context, i) => InkWell(
        onTap: () => ref.read(actionsProvider).seek(lines[i].timeMs),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text(
            lines[i].text.isEmpty ? '...' : lines[i].text,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: i == index
                ? theme.textTheme.titleMedium?.copyWith(color: theme.colorScheme.primary)
                : theme.textTheme.titleMedium?.copyWith(color: theme.colorScheme.onSurfaceVariant),
          ),
        ),
      ),
    );
  }
}

/// Output volume, kept in the settings file and applied to the core.
class VolumeSlider extends ConsumerWidget {
  const VolumeSlider({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) => const _Volume();
}

class _Volume extends ConsumerWidget {
  const _Volume();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final volume = ref.watch(volumeProvider);
    return Row(
      children: [
        Icon(volume == 0 ? Icons.volume_off : Icons.volume_up, size: 20),
        Expanded(
          child: Slider(
            value: volume.toDouble(),
            max: 100,
            divisions: 50,
            onChanged: (v) => ref.read(volumeProvider.notifier).set(v.round()),
          ),
        ),
        SizedBox(width: 36, child: Text('$volume', textAlign: TextAlign.end)),
      ],
    );
  }
}
