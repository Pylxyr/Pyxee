import 'dart:async';
import 'dart:math' as math;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../app/theme/px_theme.dart';
import '../core/models/models.dart';
import '../core/state/providers.dart';
import 'icons.dart';
import 'sheets.dart';
import 'widgets.dart';

const _tabular = [FontFeature.tabularFigures()];

/// How wide the list is decides its columns (a container query in the Tauri UI): under 600 px the artist sits under
/// the title; from 600 px it has its own column; from 1000 px the columns are further apart.
enum _Cols { narrow, medium, wide }

class _ListWidth extends InheritedWidget {
  const _ListWidth({required this.cols, required super.child});

  final _Cols cols;

  static _Cols of(BuildContext context) => context.dependOnInheritedWidgetOfExactType<_ListWidth>()?.cols ?? _Cols.narrow;

  @override
  bool updateShouldNotify(_ListWidth old) => old.cols != cols;
}

double _gap(_Cols c) => c == _Cols.wide ? 24 : 12;
double _timeWidth(_Cols c) => c == _Cols.wide ? 72 : 48;

/// A list of songs as a table (`TrackList`): column names (#, Title, Artist, Time) over rows with the place number, a
/// small cover, the title, the artist and the length. Give [tracks] and [onPlay] for the usual rows, or [rows] for your
/// own (Search mixes songs with artists and playlists).
class PxTrackList extends StatelessWidget {
  const PxTrackList({super.key, this.tracks, this.onPlay, this.rows, this.numbered = true, this.head = true});

  final List<Song>? tracks;
  final void Function(int index)? onPlay;
  final List<Widget>? rows;
  final bool numbered;
  final bool head;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return LayoutBuilder(
      builder: (context, box) {
        final cols = box.maxWidth >= 1000 ? _Cols.wide : (box.maxWidth >= 600 ? _Cols.medium : _Cols.narrow);
        final list = tracks;
        return _ListWidth(
          cols: cols,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (head)
                Container(
                  padding: const EdgeInsets.fromLTRB(8, 4, 72, 4),
                  decoration: BoxDecoration(border: Border(bottom: BorderSide(color: p.rule))),
                  child: DefaultTextStyle(
                    style: PxText.caption(p.muted),
                    child: Row(
                      children: [
                        const SizedBox(width: 32, child: Center(child: Text('#'))),
                        SizedBox(width: _gap(cols) + 40 + _gap(cols)),
                        Expanded(flex: cols == _Cols.narrow ? 1 : 14, child: const Text('Title')),
                        if (cols != _Cols.narrow) ...[
                          SizedBox(width: _gap(cols)),
                          const Expanded(flex: 10, child: Text('Artist')),
                        ],
                        SizedBox(width: _gap(cols)),
                        SizedBox(width: _timeWidth(cols), child: const Text('Time', textAlign: TextAlign.right)),
                      ],
                    ),
                  ),
                ),
              if (list != null)
                for (var i = 0; i < list.length; i++)
                  PxTrackRow(song: list[i], index: numbered ? i : null, onPlay: () => onPlay?.call(i)),
              ...?rows,
            ],
          ),
        );
      },
    );
  }
}

/// The frame of a row: hover tint, the accent bar and tint of the song that is playing, a hairline under it.
class PxRowFrame extends StatefulWidget {
  const PxRowFrame({super.key, required this.builder, this.current = false, this.onSecondaryTap, this.padding = EdgeInsets.zero, this.borderBottom = true, this.radius = 0.0});

  final Widget Function(BuildContext context, bool hovered) builder;
  final bool current;
  final VoidCallback? onSecondaryTap;
  final EdgeInsets padding;
  final bool borderBottom;
  final double radius;

  @override
  State<PxRowFrame> createState() => _PxRowFrameState();
}

class _PxRowFrameState extends State<PxRowFrame> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.translucent,
        onSecondaryTap: widget.onSecondaryTap,
        child: Stack(
          children: [
            Container(
              padding: widget.padding,
              decoration: BoxDecoration(
                color: widget.current ? p.selected : (_hovered ? p.hover : Colors.transparent),
                borderRadius: BorderRadius.circular(widget.radius),
                border: widget.borderBottom ? Border(bottom: BorderSide(color: p.rule)) : null,
              ),
              child: widget.builder(context, _hovered),
            ),
            if (widget.current)
              Positioned(left: 0, top: 0, bottom: 0, width: 3, child: ColoredBox(color: p.accent)),
          ],
        ),
      ),
    );
  }
}

/// A song in a list. The song that is playing is marked in every list it appears in, not only in the queue.
class PxTrackRow extends ConsumerWidget {
  const PxTrackRow({super.key, required this.song, required this.onPlay, this.index});

  final Song song;
  final VoidCallback onPlay;
  final int? index;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final cols = _ListWidth.of(context);
    final current = ref.watch(playerProvider.select((s) => s.currentEntry?.song.key == song.key));
    final playing = ref.watch(isPlayingProvider);
    final liked = ref.watch(libraryProvider.select((l) => l.likedKeys.contains(song.key)));
    void menu() => unawaited(showTrackMenu(context, ref, song));
    final gap = _gap(cols);
    return PxRowFrame(
      current: current,
      onSecondaryTap: menu,
      builder: (context, hovered) => Row(
        children: [
          Expanded(
            child: PxPress(
              onTap: onPlay,
              semanticLabel: 'Play ${song.title}',
              builder: (context, _, _) => Padding(
                padding: const EdgeInsets.all(8),
                child: Row(
                  children: [
                    SizedBox(
                      width: 32,
                      child: Center(
                        child: current
                            ? PxPlayingMark(playing: playing)
                            : (hovered
                                  ? IconTheme(data: IconThemeData(color: p.fg), child: const PxIcon('play', size: 14, fill: true))
                                  : (index != null ? Text('${index! + 1}', style: PxText.caption(p.muted).copyWith(fontFeatures: _tabular)) : const SizedBox.shrink())),
                      ),
                    ),
                    SizedBox(width: gap),
                    PxThumb(url: song.thumbnail, seed: song.key, size: 40),
                    SizedBox(width: gap),
                    if (cols == _Cols.narrow)
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            _title(p, current),
                            Text(song.artist, style: PxText.caption(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
                          ],
                        ),
                      )
                    else ...[
                      Expanded(flex: 14, child: _title(p, current)),
                      SizedBox(width: gap),
                      Expanded(flex: 10, child: Text(song.artist, style: PxText.body(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis)),
                    ],
                    SizedBox(width: gap),
                    SizedBox(
                      width: _timeWidth(cols),
                      child: Text(formatTime(song.durationMs), textAlign: TextAlign.right, style: PxText.caption(p.muted).copyWith(fontFeatures: _tabular)),
                    ),
                  ],
                ),
              ),
            ),
          ),
          Opacity(
            opacity: hovered || liked ? 1 : 0,
            child: PxIconButton('heart', iconSize: 18, fill: liked, pressed: liked, label: liked ? 'Unlike ${song.title}' : 'Like ${song.title}', onPressed: () => ref.read(apiProvider).setLiked(song, !liked)),
          ),
          Opacity(
            opacity: hovered ? 1 : 0,
            child: PxIconButton('more', iconSize: 18, label: 'More options for ${song.title}', onPressed: menu),
          ),
        ],
      ),
    );
  }

  Widget _title(PxColors p, bool current) => Text(
    song.title,
    style: PxText.body(current ? p.accent : p.fg).copyWith(fontWeight: current ? FontWeight.w600 : FontWeight.w400),
    maxLines: 1,
    overflow: TextOverflow.ellipsis,
  );
}

/// An artist or a playlist among the songs of a search.
class PxEntityRow extends StatelessWidget {
  const PxEntityRow({super.key, required this.title, required this.subtitle, required this.url, required this.seed, required this.onTap, this.round = false});

  final String title;
  final String subtitle;
  final String? url;
  final String seed;
  final VoidCallback onTap;
  final bool round;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final cols = _ListWidth.of(context);
    final gap = _gap(cols);
    return PxRowFrame(
      builder: (context, hovered) => PxPress(
        onTap: onTap,
        semanticLabel: title,
        builder: (context, _, _) => Padding(
          padding: const EdgeInsets.all(8),
          child: Row(
            children: [
              SizedBox(width: 32 + gap),
              PxThumb(url: url, seed: seed, size: 40, radius: round ? 20 : rSm),
              SizedBox(width: gap),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title, style: PxText.body(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
                    Text(subtitle, style: PxText.caption(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A row that can be edited in place (`EditableRow`): the queue, Up next in Now Playing and a playlist. A handle to
/// drag it by, the song, and the row's own buttons.
class PxEditableRow extends ConsumerWidget {
  const PxEditableRow({super.key, required this.song, required this.onPlay, required this.actions, this.handle, this.index, this.status = '', this.current = false, this.coverSize = 40});

  final Song song;
  final VoidCallback onPlay;
  final List<Widget> actions;
  final Widget? handle;
  final int? index;

  /// Replaces the artist line, in the accent colour ("Now playing", "Loading").
  final String status;
  final bool current;
  final double coverSize;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    return PxRowFrame(
      current: current,
      borderBottom: false,
      radius: rSm,
      onSecondaryTap: () => unawaited(showTrackMenu(context, ref, song)),
      builder: (context, hovered) => Row(
        children: [
          ?handle,
          Expanded(
            child: PxPress(
              onTap: onPlay,
              semanticLabel: 'Play ${song.title}',
              builder: (context, _, _) => Padding(
                padding: const EdgeInsets.fromLTRB(0, 6, 8, 6),
                child: Row(
                  children: [
                    if (index != null) SizedBox(width: 24, child: Text('${index! + 1}', textAlign: TextAlign.center, style: PxText.caption(p.muted).copyWith(fontFeatures: _tabular))),
                    if (index != null) const SizedBox(width: 12) else const SizedBox(width: 8),
                    PxThumb(url: song.thumbnail, seed: song.key, size: coverSize),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            song.title,
                            style: PxText.body(current ? p.accent : p.fg).copyWith(fontWeight: current ? FontWeight.w600 : FontWeight.w400),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          Text(
                            status.isNotEmpty ? status : song.artist,
                            style: PxText.caption(status.isNotEmpty ? p.accent : p.muted),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 12),
                    Text(formatTime(song.durationMs), style: PxText.caption(p.muted).copyWith(fontFeatures: _tabular)),
                  ],
                ),
              ),
            ),
          ),
          ...actions,
        ],
      ),
    );
  }
}

/// The drag handle of an editable row (six dots). Wrap it in a `ReorderableDragStartListener`.
class PxHandle extends StatelessWidget {
  const PxHandle({super.key});

  @override
  Widget build(BuildContext context) => MouseRegion(
    cursor: SystemMouseCursors.grab,
    child: Padding(
      padding: const EdgeInsets.fromLTRB(8, 8, 4, 8),
      child: IconTheme(data: IconThemeData(color: context.px.muted), child: const PxIcon('drag', size: 18)),
    ),
  );
}

// ------------------------------------------------------------------------------------------------- covers

/// One cover with its title under it; the pointer over it brings a play button up in the corner.
class PxCoverTile extends StatelessWidget {
  const PxCoverTile({super.key, required this.title, required this.subtitle, required this.url, required this.seed, required this.onTap, this.busy = false});

  final String title;
  final String subtitle;
  final String? url;
  final String seed;
  final VoidCallback? onTap;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return PxPress(
      onTap: onTap,
      semanticLabel: 'Play $title',
      builder: (context, hovered, pressed) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          AspectRatio(
            aspectRatio: 1,
            child: Stack(
              fit: StackFit.expand,
              children: [
                PxThumb(url: url, seed: seed, radius: rMd),
                Positioned.fill(
                  child: IgnorePointer(
                    child: DecoratedBox(
                      decoration: BoxDecoration(borderRadius: BorderRadius.circular(rMd), border: Border.all(color: p.fg.withValues(alpha: 0.14))),
                    ),
                  ),
                ),
                Positioned(
                  right: 8,
                  bottom: 8,
                  child: AnimatedSlide(
                    duration: const Duration(milliseconds: 160),
                    offset: hovered || busy ? Offset.zero : const Offset(0, 0.25),
                    child: AnimatedOpacity(
                      duration: const Duration(milliseconds: 160),
                      opacity: hovered || busy ? 1 : 0,
                      child: Container(
                        width: 36,
                        height: 36,
                        decoration: BoxDecoration(color: p.accent, borderRadius: BorderRadius.circular(13)),
                        child: busy
                            ? Padding(padding: const EdgeInsets.all(9), child: CircularProgressIndicator(strokeWidth: 2, color: p.onAccent))
                            : IconTheme(data: IconThemeData(color: p.onAccent), child: const Center(child: PxIcon('play', size: 18, fill: true))),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 8),
          Text(title, style: PxText.body(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
          if (subtitle.isNotEmpty) Text(subtitle, style: PxText.caption(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
        ],
      ),
    );
  }
}

class PxTileData {
  const PxTileData({required this.title, this.subtitle = '', this.url, required this.seed, this.onTap, this.busy = false});

  final String title;
  final String subtitle;
  final String? url;
  final String seed;
  final VoidCallback? onTap;
  final bool busy;
}

double _tileMin(double window) => (window * 0.09).clamp(132.0, 190.0);

/// Tiles that wrap onto as many rows as needed (`.cover-grid`): Library's playlists.
class PxCoverGrid extends StatelessWidget {
  const PxCoverGrid({super.key, required this.tiles});

  final List<PxTileData> tiles;

  @override
  Widget build(BuildContext context) {
    final min = _tileMin(MediaQuery.sizeOf(context).width);
    return Padding(
      padding: const EdgeInsets.only(top: 4),
      child: LayoutBuilder(
        builder: (context, box) {
          const gap = 12.0;
          final count = math.max(1, ((box.maxWidth + gap) / (min + gap)).floor());
          final width = (box.maxWidth - gap * (count - 1)) / count;
          return Wrap(
            spacing: gap,
            runSpacing: gap,
            children: [
              for (final t in tiles)
                SizedBox(width: width, child: PxCoverTile(title: t.title, subtitle: t.subtitle, url: t.url, seed: t.seed, onTap: t.onTap, busy: t.busy)),
            ],
          );
        },
      ),
    );
  }
}

/// A heading and, under it, one row of covers that scrolls sideways to the page's edges; the arrows in the heading
/// page it (`.shelves .cover-grid`).
class PxShelf extends StatefulWidget implements PxFullBleed {
  const PxShelf({super.key, required this.title, required this.tiles, this.actions = const [], this.first = false});

  final String title;
  final List<PxTileData> tiles;
  final List<Widget> actions;
  final bool first;

  @override
  State<PxShelf> createState() => _PxShelfState();
}

class _PxShelfState extends State<PxShelf> {
  final _c = ScrollController();
  bool _back = false;
  bool _forward = false;

  @override
  void initState() {
    super.initState();
    _c.addListener(_measure);
    WidgetsBinding.instance.addPostFrameCallback((_) => _measure());
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  void _measure() {
    if (!_c.hasClients || !mounted) return;
    final back = _c.offset > 4;
    final forward = _c.offset + _c.position.viewportDimension < _c.position.maxScrollExtent - 4;
    if (back != _back || forward != _forward) {
      setState(() {
        _back = back;
        _forward = forward;
      });
    }
  }

  void _page(int dir) {
    if (!_c.hasClients) return;
    final to = (_c.offset + dir * _c.position.viewportDimension * 0.85).clamp(0.0, _c.position.maxScrollExtent);
    _c.animateTo(to, duration: const Duration(milliseconds: 320), curve: Curves.easeOutCubic);
  }

  @override
  Widget build(BuildContext context) {
    final window = MediaQuery.sizeOf(context).width;
    final pad = pagePad(window);
    final tile = (window * 0.105).clamp(148.0, 220.0);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: EdgeInsets.symmetric(horizontal: pad),
          child: PxSectionHead(
            widget.title,
            first: widget.first,
            actions: [
              ...widget.actions,
              PxIconButton('down', label: 'Scroll back', quarterTurns: 1, onPressed: _back ? () => _page(-1) : null),
              PxIconButton('down', label: 'Scroll forward', quarterTurns: 3, onPressed: _forward ? () => _page(1) : null),
            ],
          ),
        ),
        SizedBox(
          height: tile + 78,
          child: ScrollConfiguration(
            behavior: const PxScrollBehavior().copyWith(scrollbars: false),
            child: ListView.separated(
              controller: _c,
              scrollDirection: Axis.horizontal,
              padding: EdgeInsets.fromLTRB(pad, 4, pad, 14),
              itemCount: widget.tiles.length,
              separatorBuilder: (_, _) => const SizedBox(width: 16),
              itemBuilder: (context, i) {
                final t = widget.tiles[i];
                return SizedBox(
                  width: tile,
                  child: PxCoverTile(title: t.title, subtitle: t.subtitle, url: t.url, seed: t.seed, onTap: t.onTap, busy: t.busy),
                );
              },
            ),
          ),
        ),
      ],
    );
  }
}
