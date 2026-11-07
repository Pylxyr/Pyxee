import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../bridge/bridge.dart';
import '../models/models.dart';

/// Opened once in `main` and handed in as an override, so a missing library shows an error screen instead of a crash.
final bridgeProvider = Provider<Bridge>((ref) => throw UnimplementedError());

Stream<Object?> _events(Bridge bridge, String name) => bridge.events
    .where((e) => e.name == name)
    .map((e) => e.data);

/// The player without its position. Rebuilds only when something other than the position moved.
class PlayerNotifier extends Notifier<PlayerState> {
  @override
  PlayerState build() {
    final bridge = ref.watch(bridgeProvider);
    final sub = _events(bridge, 'state').listen((data) {
      final next = PlayerState.fromJson(data! as Json);
      if (next.seq > state.seq) state = next;
    });
    ref.onDispose(sub.cancel);
    return PlayerState.fromJson(bridge.call('getState')! as Json);
  }
}

final playerProvider = NotifierProvider<PlayerNotifier, PlayerState>(
  PlayerNotifier.new,
);

/// The playback position in ms, four times a second while playing. Only the progress widgets watch it.
class PositionNotifier extends Notifier<int> {
  int _seq = 0;

  @override
  int build() {
    final bridge = ref.watch(bridgeProvider);
    void apply(Object? data) {
      final json = data! as Json;
      final seq = json['seq']! as int;
      if (seq > _seq) {
        _seq = seq;
        state = json['positionMs']! as int;
      }
    }

    final subs = [
      _events(bridge, 'state').listen(apply),
      _events(bridge, 'position').listen(apply),
    ];
    ref.onDispose(() {
      for (final s in subs) {
        unawaited(s.cancel());
      }
    });
    final first = bridge.call('getState')! as Json;
    _seq = first['seq']! as int;
    return first['positionMs']! as int;
  }
}

final positionProvider = NotifierProvider<PositionNotifier, int>(
  PositionNotifier.new,
);

final currentSongProvider = Provider<Song?>(
  (ref) => ref.watch(playerProvider.select((p) => p.currentEntry?.song)),
);

final isPlayingProvider = Provider<bool>(
  (ref) => ref.watch(playerProvider.select((p) => p.transport.playing)),
);

class LibraryNotifier extends Notifier<LibraryState> {
  @override
  LibraryState build() {
    final bridge = ref.watch(bridgeProvider);
    final sub = _events(bridge, 'library').listen((data) {
      final next = LibraryState.fromJson(data! as Json);
      if (next.revision >= state.revision) state = next;
    });
    ref.onDispose(sub.cancel);
    try {
      return LibraryState.fromJson(bridge.call('getLibrary')! as Json);
    } on BridgeException {
      return LibraryState.empty;
    }
  }
}

final libraryProvider = NotifierProvider<LibraryNotifier, LibraryState>(
  LibraryNotifier.new,
);

class SleepNotifier extends Notifier<SleepStatus?> {
  @override
  SleepStatus? build() {
    final bridge = ref.watch(bridgeProvider);
    final sub = _events(bridge, 'sleep').listen((data) {
      final status = (data! as Json)['status'];
      state = status == null ? null : SleepStatus.fromJson(status as Json);
    });
    ref.onDispose(sub.cancel);
    final now = bridge.call('sleepGet')! as Json;
    final status = now['status'];
    return status == null ? null : SleepStatus.fromJson(status as Json);
  }
}

final sleepProvider = NotifierProvider<SleepNotifier, SleepStatus?>(
  SleepNotifier.new,
);

/// Messages from the core that are not player state (a song was skipped, the output device changed).
final noticeProvider = StreamProvider<String>((ref) {
  final bridge = ref.watch(bridgeProvider);
  return _events(bridge, 'notice').map((d) => (d! as Json)['message']! as String);
});

/// What the window should do, asked by the system media widget (Linux): `show` or `quit`.
final windowRequestProvider = StreamProvider<String>((ref) {
  final bridge = ref.watch(bridgeProvider);
  return _events(bridge, 'window').map((d) => (d! as Json)['action']! as String);
});

// ------------------------------------------------------------------ commands and queries

class PlayerActions {
  PlayerActions(this._bridge);

  final Bridge _bridge;

  void _d(Json command) => _bridge.dispatch(command);

  void play(List<Song> songs, [int start = 0]) {
    if (songs.isEmpty) return;
    _d({'type': 'playTracks', 'tracks': [for (final s in songs) s.toJson()], 'start': start});
  }

  void toggle() => _d({'type': 'toggle'});
  void next() => _d({'type': 'next'});
  void previous() => _d({'type': 'previous'});
  void seek(int ms) => _d({'type': 'seek', 'ms': ms});
  void retry() => _d({'type': 'retry'});
  void playEntry(int id) => _d({'type': 'playEntry', 'id': id});
  void addToQueue(Song s) => _d({'type': 'addToQueue', 'track': s.toJson()});
  void playNext(Song s) => _d({'type': 'playNext', 'track': s.toJson()});
  void remove(int id) => _d({'type': 'remove', 'id': id});
  void moveUpcoming(int id, int to) => _d({'type': 'moveUpcoming', 'id': id, 'to': to});
  void clear() => _d({'type': 'clear'});
  void undo() => _d({'type': 'undo'});
  void setShuffle(bool on) => _d({'type': 'setShuffle', 'on': on});
  void setAutoplay(bool on) => _d({'type': 'setAutoplay', 'on': on});
  void setRepeat(RepeatMode r) => _d({'type': 'setRepeat', 'repeat': r.name});
  void setVolume(int percent) => _bridge.call('setVolume', {'percent': percent});
  void sleepMinutes(int minutes) =>
      _bridge.call('sleepSet', {'setting': {'kind': 'minutes', 'minutes': minutes}});
  void sleepEndOfTrack() => _bridge.call('sleepSet', {'setting': {'kind': 'endOfTrack'}});
  void sleepCancel() => _bridge.call('sleepCancel');
  void sleepExtend(int minutes) => _bridge.call('sleepExtend', {'minutes': minutes});
}

final actionsProvider = Provider<PlayerActions>(
  (ref) => PlayerActions(ref.watch(bridgeProvider)),
);

class CoreApi {
  CoreApi(this._b);

  final Bridge _b;

  Future<List<SearchHit>> search(String query, SearchKind kind) async {
    final data = await _b.callAsync('search', {'query': query, 'filter': kind.name});
    final items = (data! as Json)['items']! as List<Object?>;
    return [
      for (final i in items)
        switch ((i! as Json)['type']) {
          'track' => SearchHit.song(Song.fromJson((i as Json)['track']! as Json)),
          'playlist' => SearchHit.playlist(
            key: (i as Json)['key']! as String,
            title: i['title']! as String,
            thumbnail: i['thumbnail'] as String?,
          ),
          _ => SearchHit.artist(
            key: (i as Json)['key']! as String,
            title: i['name']! as String,
            thumbnail: i['thumbnail'] as String?,
          ),
        },
    ];
  }

  Future<List<SearchKind>> supportedFilters() async {
    final data = await _b.callAsync('searchFilters') as List<Object?>;
    return [
      for (final f in data)
        if ((f! as Json)['supported']! as bool) SearchKind.values.byName((f as Json)['filter']! as String),
    ];
  }

  Future<List<Shelf>> explore() async => _shelves(await _b.callAsync('explore'));

  Future<List<Genre>> genres() async => [
    for (final g in await _b.callAsync('genres') as List<Object?>)
      Genre(title: (g! as Json)['title']! as String, params: (g as Json)['params']! as String),
  ];

  Future<List<Shelf>> genre(String params) async =>
      _shelves(await _b.callAsync('genre', {'params': params}));

  Future<List<Song>> playlistTracks(String id) async =>
      songs(await _b.callAsync('playlistTracks', {'id': id}));

  Future<List<Song>> related(String key) async =>
      songs(await _b.callAsync('related', {'key': key, 'count': 20}));

  Future<LyricsData> lyrics(Song s) async {
    final data = await _b.callAsync('lyrics', {
      'key': s.key,
      'title': s.title,
      'artist': s.artist,
      'durationMs': s.durationMs,
    });
    return LyricsData.fromJson(data! as Json);
  }

  List<Shelf> _shelves(Object? data) => [
    for (final s in data! as List<Object?>) Shelf.fromJson(s! as Json),
  ];

  // library
  Future<HomeData> home() async => HomeData.fromJson(await _b.callAsync('home') as Json);
  Future<List<Song>> liked() async => songs(await _b.callAsync('liked'));

  Future<List<HistoryItem>> history() async => [
    for (final h in await _b.callAsync('historyGet', {'limit': 200}) as List<Object?>)
      HistoryItem.fromJson(h! as Json),
  ];

  Future<PlaylistDetail> playlist(int id) async =>
      PlaylistDetail.fromJson(await _b.callAsync('playlistGet', {'id': id}) as Json);

  Future<void> setLiked(Song s, bool liked) =>
      _b.callAsync('setLiked', {'track': s.toJson(), 'liked': liked});

  Future<int> createPlaylist(String name, List<Song> tracks) async {
    final id = await _b.callAsync('playlistCreate', {
      'name': name,
      'tracks': [for (final t in tracks) t.toJson()],
    });
    return id! as int;
  }

  Future<void> renamePlaylist(int id, String name) =>
      _b.callAsync('playlistRename', {'id': id, 'name': name});

  /// Returns what Undo needs to send to [restorePlaylist].
  Future<Json> deletePlaylist(int id) async =>
      await _b.callAsync('playlistDelete', {'id': id}) as Json;

  Future<void> restorePlaylist(Json snapshot) =>
      _b.callAsync('playlistRestore', {'snapshot': snapshot});

  Future<void> addToPlaylist(int id, List<Song> tracks) => _b.callAsync('playlistAdd', {
    'id': id,
    'tracks': [for (final t in tracks) t.toJson()],
  });

  Future<Json> removeEntry(int entryId) async =>
      await _b.callAsync('playlistRemoveEntry', {'entryId': entryId}) as Json;

  Future<void> restoreEntry(Json removed) =>
      _b.callAsync('playlistRestoreEntry', {'removed': removed});

  Future<void> moveEntry(int entryId, int to) =>
      _b.callAsync('playlistMoveEntry', {'entryId': entryId, 'to': to});

  Future<List<HistoryItem>> clearHistory() async => [
    for (final h in await _b.callAsync('historyClear') as List<Object?>)
      HistoryItem.fromJson(h! as Json),
  ];

  Future<void> restoreHistory(List<HistoryItem> items) =>
      _b.callAsync('historyRestore', {'items': [for (final i in items) i.raw]});

  // settings file
  Future<Json> prefs() async {
    final text = await _b.callAsync('prefsGet');
    if (text is! String) return {};
    try {
      return jsonDecode(text) as Json;
    } on FormatException {
      return {};
    }
  }

  Future<void> savePrefs(Json prefs) => _b.callAsync('prefsSet', {'json': jsonEncode(prefs)});
  Future<void> quit() => _b.callAsync('quit');
}

final apiProvider = Provider<CoreApi>((ref) => CoreApi(ref.watch(bridgeProvider)));

String errorText(Object e) {
  if (e is BridgeException) {
    return switch (e.kind) {
      'offline' => 'You are offline.',
      'rateLimited' => 'The service is limiting requests. Try again in a moment.',
      'toolMissing' => 'yt-dlp was not found. Put it next to the app or set PYXEE_YTDLP.',
      _ => e.message,
    };
  }
  return e.toString();
}

// Queries. They refetch when the library changes, so a like or a new playlist shows up without a refresh.

final exploreProvider = FutureProvider<List<Shelf>>((ref) => ref.watch(apiProvider).explore());
final genresProvider = FutureProvider<List<Genre>>((ref) => ref.watch(apiProvider).genres());

final genreProvider = FutureProvider.family<List<Shelf>, String>(
  (ref, params) => ref.watch(apiProvider).genre(params),
);

final homeProvider = FutureProvider.autoDispose<HomeData>((ref) {
  ref.watch(libraryProvider.select((l) => l.revision));
  return ref.watch(apiProvider).home();
});

final likedProvider = FutureProvider.autoDispose<List<Song>>((ref) {
  ref.watch(libraryProvider.select((l) => l.revision));
  return ref.watch(apiProvider).liked();
});

final historyProvider = FutureProvider.autoDispose<List<HistoryItem>>((ref) {
  ref.watch(libraryProvider.select((l) => l.revision));
  return ref.watch(apiProvider).history();
});

final playlistProvider = FutureProvider.autoDispose.family<PlaylistDetail, int>((ref, id) {
  ref.watch(libraryProvider.select((l) => l.revision));
  return ref.watch(apiProvider).playlist(id);
});

final relatedProvider = FutureProvider.autoDispose.family<List<Song>, String>(
  (ref, key) => ref.watch(apiProvider).related(key),
);

final lyricsProvider = FutureProvider.autoDispose.family<LyricsData, Song>(
  (ref, song) => ref.watch(apiProvider).lyrics(song),
);

final searchFiltersProvider = FutureProvider<List<SearchKind>>(
  (ref) => ref.watch(apiProvider).supportedFilters(),
);

typedef SearchQuery = ({String text, SearchKind kind});

final searchProvider = FutureProvider.autoDispose.family<List<SearchHit>, SearchQuery>(
  (ref, q) => ref.watch(apiProvider).search(q.text, q.kind),
);
