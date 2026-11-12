import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/models/models.dart';
import '../../core/state/providers.dart';

/// The pages, in the order they sit in: a page after the current one slides in from the right, before it from the left.
enum PxRoute { home, explore, library, playlist, queue, search }

enum LibraryTab { liked, playlists, history, downloads }

enum NpTab { upnext, lyrics, related }

@immutable
class NavState {
  const NavState({
    this.route = PxRoute.home,
    this.libraryTab = LibraryTab.liked,
    this.playlistId,
    this.npOpen = false,
    this.npTab = NpTab.upnext,
    this.dir = 1,
  });

  final PxRoute route;
  final LibraryTab libraryTab;
  final int? playlistId;
  final bool npOpen;
  final NpTab npTab;

  /// Which way the page slides: 1 when the new page is after the old one, -1 when before.
  final int dir;

  NavState copyWith({
    PxRoute? route,
    LibraryTab? libraryTab,
    int? playlistId,
    bool? npOpen,
    NpTab? npTab,
    int? dir,
  }) => NavState(
    route: route ?? this.route,
    libraryTab: libraryTab ?? this.libraryTab,
    playlistId: playlistId ?? this.playlistId,
    npOpen: npOpen ?? this.npOpen,
    npTab: npTab ?? this.npTab,
    dir: dir ?? this.dir,
  );
}

class NavNotifier extends Notifier<NavState> {
  @override
  NavState build() => const NavState();

  int _dirTo(PxRoute route) => route.index >= state.route.index ? 1 : -1;

  void go(PxRoute route) {
    if (route == state.route) return;
    state = state.copyWith(route: route, dir: _dirTo(route));
  }

  void openPlaylist(int id) {
    state = state.copyWith(route: PxRoute.playlist, playlistId: id, dir: _dirTo(PxRoute.playlist));
  }

  void setLibraryTab(LibraryTab tab) => state = state.copyWith(libraryTab: tab);

  void openLibrary(LibraryTab tab) {
    state = state.copyWith(route: PxRoute.library, libraryTab: tab, dir: _dirTo(PxRoute.library));
  }

  void setNpTab(NpTab tab) => state = state.copyWith(npTab: tab);

  void openNowPlaying() {
    debugPrint('[pyxee] open now playing (already open: ${state.npOpen})');
    if (!state.npOpen) state = state.copyWith(npOpen: true);
  }

  /// Given a page, Now Playing closes onto that page.
  void closeNowPlaying([PxRoute? then]) {
    if (!state.npOpen) return;
    state = state.copyWith(npOpen: false, route: then, dir: then == null ? null : _dirTo(then));
  }
}

final navProvider = NotifierProvider<NavNotifier, NavState>(NavNotifier.new);

// ---------------------------------------------------------------------------------------------------- search

@immutable
class SearchRequest {
  const SearchRequest({this.query = '', this.kind = SearchKind.all});

  final String query;
  final SearchKind kind;

  SearchQuery? get asQuery => query.isEmpty ? null : (text: query, kind: kind);
}

class SearchNotifier extends Notifier<SearchRequest> {
  @override
  SearchRequest build() => const SearchRequest();

  /// Starts a search and shows the results page.
  void run(String query, {SearchKind kind = SearchKind.all}) {
    final q = query.trim();
    if (q.isEmpty) return;
    state = SearchRequest(query: q, kind: kind);
    ref.read(navProvider.notifier).go(PxRoute.search);
  }

  void setKind(SearchKind kind) => state = SearchRequest(query: state.query, kind: kind);
}

final searchRequestProvider = NotifierProvider<SearchNotifier, SearchRequest>(SearchNotifier.new);

// -------------------------------------------------------------------------------------------------- snackbar

@immutable
class Toast {
  const Toast(this.id, this.message, {this.actionLabel, this.onAction});

  final int id;
  final String message;
  final String? actionLabel;
  final VoidCallback? onAction;
}

class ToastNotifier extends Notifier<List<Toast>> {
  int _next = 1;

  @override
  List<Toast> build() => const [];

  /// A message at the foot of the window, gone after [timeout]. An [actionLabel] adds a button (Undo, Open).
  void push(String message, {String? actionLabel, VoidCallback? onAction, Duration timeout = const Duration(seconds: 5)}) {
    final id = _next++;
    state = [...state.length >= 3 ? state.sublist(state.length - 2) : state, Toast(id, message, actionLabel: actionLabel, onAction: onAction)];
    Timer(timeout, () {
      if (ref.mounted) dismiss(id);
    });
  }

  void dismiss(int id) => state = [for (final t in state) if (t.id != id) t];

  void act(int id) {
    for (final t in state) {
      if (t.id == id) {
        dismiss(id);
        t.onAction?.call();
        return;
      }
    }
  }
}

final toastProvider = NotifierProvider<ToastNotifier, List<Toast>>(ToastNotifier.new);

/// Search suggestions for what is typed so far.
final suggestionsProvider = FutureProvider.autoDispose.family<List<String>, String>((ref, prefix) async {
  if (prefix.trim().length < 2) return const [];
  // Wait a moment: a new letter disposes this provider, so only the last one asks.
  var cancelled = false;
  ref.onDispose(() => cancelled = true);
  await Future<void>.delayed(const Duration(milliseconds: 180));
  if (cancelled) return const [];
  try {
    return await ref.read(apiProvider).suggest(prefix.trim());
  } on Object {
    return const [];
  }
});
