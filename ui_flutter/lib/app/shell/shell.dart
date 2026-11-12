import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';
import 'package:window_manager/window_manager.dart';

import '../../core/state/providers.dart';
import '../../core/state/updates.dart';
import '../../features/explore/explore_page.dart';
import '../../features/home/home_page.dart';
import '../../features/library/library_page.dart';
import '../../features/now_playing/now_playing.dart';
import '../../features/playlist/playlist_page.dart';
import '../../features/queue/queue_page.dart';
import '../../features/search/search_page.dart';
import '../../features/settings/changelog_sheet.dart';
import '../../shared/widgets.dart';
import '../state/ui_state.dart';
import '../theme/px_theme.dart';
import 'deck.dart';
import 'navigation.dart';
import 'title_bar.dart';
import 'top_bar.dart';

/// The frame around every page, as the Tauri UI draws it. A wide window (900 px and up) has a masthead (the wordmark,
/// the places as tabs, the search box, Settings), the page, and the deck along the bottom; from 1440 px the queue
/// lane sits beside the page. A narrow one has the search box on top of the page, then the dock and the bottom bar.
class PxShell extends ConsumerStatefulWidget {
  const PxShell({super.key});

  @override
  ConsumerState<PxShell> createState() => _PxShellState();
}

class _PxShellState extends ConsumerState<PxShell> with WindowListener {
  bool _fullscreen = false;
  final _focus = FocusNode(debugLabel: 'shell');

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    // Pyxee looks for a newer version by itself once, after the first frame. It says nothing unless there is one.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(ref.read(updatesProvider.notifier).checkAtStartup());
    });
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    _focus.dispose();
    super.dispose();
  }

  @override
  void onWindowEnterFullScreen() => setState(() => _fullscreen = true);

  @override
  void onWindowLeaveFullScreen() => setState(() => _fullscreen = false);

  /// In-app keys: Space play/pause, Left/Right seek 5 s, N next, P previous, Esc close Now Playing, F11 full screen.
  /// Ignored while typing in a text field.
  KeyEventResult _onKey(FocusNode node, KeyEvent e) {
    if (e is KeyUpEvent) return KeyEventResult.ignored;
    final key = e.logicalKey;
    if (key == LogicalKeyboardKey.f11) {
      unawaited(windowManager.setFullScreen(!_fullscreen));
      return KeyEventResult.handled;
    }
    final nav = ref.read(navProvider);
    if (key == LogicalKeyboardKey.escape) {
      if (nav.npOpen) {
        ref.read(navProvider.notifier).closeNowPlaying();
        return KeyEventResult.handled;
      }
      return KeyEventResult.ignored;
    }
    final typing = FocusManager.instance.primaryFocus?.context?.findAncestorWidgetOfExactType<EditableText>() != null;
    final hw = HardwareKeyboard.instance;
    if (typing || hw.isControlPressed || hw.isAltPressed || hw.isMetaPressed) return KeyEventResult.ignored;
    final song = ref.read(currentSongProvider);
    if (song == null) return KeyEventResult.ignored;
    final actions = ref.read(actionsProvider);
    if (key == LogicalKeyboardKey.space) {
      actions.toggle();
    } else if (key == LogicalKeyboardKey.arrowRight || key == LogicalKeyboardKey.arrowLeft) {
      final delta = key == LogicalKeyboardKey.arrowRight ? 5000 : -5000;
      final max = song.durationMs ?? 1 << 40;
      actions.seek((ref.read(positionProvider) + delta).clamp(0, max));
    } else if (key == LogicalKeyboardKey.keyN) {
      actions.next();
    } else if (key == LogicalKeyboardKey.keyP) {
      actions.previous();
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final width = MediaQuery.sizeOf(context).width;
    final wide = width >= 900;
    final lane = wide && width >= 1440 && ref.watch(currentSongProvider) != null;
    final pad = pagePad(width);
    final libraryError = ref.watch(libraryProvider.select((l) => l.error));
    final hasSong = ref.watch(currentSongProvider) != null;

    // Messages from the core that are not player state (a song was skipped, the output device changed).
    ref.listen(noticeProvider, (_, next) => next.whenData((m) => ref.read(toastProvider.notifier).push(m, timeout: const Duration(seconds: 10))));

    // A check that found a newer version shows what's new, whether the person asked or Pyxee looked by itself.
    ref.listen(updatesProvider.select((u) => u.changelogOpen), (was, open) {
      if (open && was != true) unawaited(showChangelogSheet(context, ref));
    });

    final page = _Page(libraryError: libraryError, topBar: wide ? null : Padding(padding: EdgeInsets.symmetric(horizontal: pad, vertical: 12), child: const PxTopBar(showSettings: true)));

    final Widget body = wide
        ? Column(
            children: [
              Container(
                padding: EdgeInsets.symmetric(horizontal: pad),
                decoration: BoxDecoration(color: p.layer, border: Border(bottom: BorderSide(color: p.rule))),
                child: Row(
                  children: [
                    const PxTabs(),
                    const SizedBox(width: 24),
                    const Expanded(child: PxTopBarWide()),
                  ],
                ),
              ),
              Expanded(
                child: Row(
                  children: [
                    Expanded(child: page),
                    if (lane) const PxLane(),
                  ],
                ),
              ),
              const PxDeck(),
            ],
          )
        : Column(
            children: [
              Expanded(child: page),
              if (hasSong) const PxDock(),
              const PxBottomBar(),
            ],
          );

    return Focus(
      focusNode: _focus,
      autofocus: true,
      onKeyEvent: _onKey,
      child: Container(
        color: p.bg,
        child: Column(
          children: [
            if (ownTitleBar && !_fullscreen) PxTitleBar(showBrand: !wide),
            Expanded(
              child: Stack(
                children: [
                  Positioned.fill(child: body),
                  const Positioned.fill(child: PxNowPlaying()),
                  Positioned(left: 0, right: 0, bottom: wide ? 24 : 144, child: const _Toasts()),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The search box inside the masthead: it sits left and Settings sits on the far right.
class PxTopBarWide extends ConsumerWidget {
  const PxTopBarWide({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) => const Padding(
    padding: EdgeInsets.symmetric(vertical: 12),
    child: PxTopBar(showSettings: true),
  );
}

class _Page extends ConsumerWidget {
  const _Page({required this.libraryError, required this.topBar});

  final String? libraryError;
  final Widget? topBar;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final nav = ref.watch(navProvider);
    final pad = pagePad(MediaQuery.sizeOf(context).width);
    final Widget content = switch (nav.route) {
      PxRoute.home => const HomePage(),
      PxRoute.explore => const ExplorePage(),
      PxRoute.library => const LibraryPage(),
      PxRoute.playlist => const PlaylistPage(),
      PxRoute.queue => const QueuePage(),
      PxRoute.search => const SearchPage(),
    };
    return Container(
      color: p.layer,
      child: Column(
        children: [
          ?topBar,
          if (libraryError != null)
            Padding(
              padding: EdgeInsets.fromLTRB(pad, 0, pad, 8),
              child: PxNotice(children: [Text("Your library couldn't be opened ($libraryError). Likes, playlists and history won't be saved this session.", style: PxText.body(p.fg))]),
            ),
          Expanded(
            // Fade through: the old page is gone in the first half and the new one comes in during the second, with a
            // short slide along the order of the tabs, so the two are never drawn on top of each other.
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 240),
              switchInCurve: const Interval(0.5, 1.0, curve: Curves.easeOutCubic),
              switchOutCurve: const Interval(0.5, 1.0, curve: Curves.easeInCubic),
              layoutBuilder: (current, previous) => Stack(fit: StackFit.expand, children: [...previous, ?current]),
              transitionBuilder: (child, animation) => FadeTransition(
                opacity: animation,
                child: SlideTransition(position: Tween(begin: Offset(nav.dir * 0.02, 0), end: Offset.zero).animate(animation), child: child),
              ),
              child: KeyedSubtree(key: ValueKey(nav.route), child: content),
            ),
          ),
        ],
      ),
    );
  }
}

/// Messages at the foot of the window: the text colour as the background, an accent bar on the left, and Undo.
class _Toasts extends ConsumerWidget {
  const _Toasts();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final toasts = ref.watch(toastProvider);
    return IgnorePointer(
      ignoring: toasts.isEmpty,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final t in toasts)
            Padding(
              padding: const EdgeInsets.only(bottom: 8, left: 16, right: 16),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 448),
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                  decoration: BoxDecoration(
                    color: p.fg,
                    borderRadius: BorderRadius.circular(rMd),
                    border: Border(left: BorderSide(color: p.accent, width: 3)),
                    boxShadow: [BoxShadow(color: Colors.black.withValues(alpha: p.dark ? 0.3 : 0.16), blurRadius: 30, offset: const Offset(0, 10))],
                  ),
                  child: Row(
                    children: [
                      Expanded(child: Text(t.message, style: PxText.body(p.onFg))),
                      if (t.actionLabel != null) ...[
                        const SizedBox(width: 16),
                        PxLink(t.actionLabel!, color: p.onFg, onTap: () => ref.read(toastProvider.notifier).act(t.id)),
                      ],
                    ],
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}
