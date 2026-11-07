import 'package:flutter/foundation.dart';

typedef Json = Map<String, Object?>;

List<T> _list<T>(Object? v, T Function(Json) f) => [
  for (final e in (v as List<Object?>? ?? const <Object?>[])) f(e! as Json),
];

@immutable
class Song {
  const Song({
    required this.key,
    required this.title,
    required this.artist,
    this.durationMs,
    this.thumbnail,
  });

  factory Song.fromJson(Json j) => Song(
    key: j['key']! as String,
    title: j['title']! as String,
    artist: j['artist']! as String,
    durationMs: j['durationMs'] as int?,
    thumbnail: j['thumbnail'] as String?,
  );

  final String key;
  final String title;
  final String artist;
  final int? durationMs;
  final String? thumbnail;

  Json toJson() => {
    'key': key,
    'title': title,
    'artist': artist,
    'durationMs': durationMs,
    'thumbnail': thumbnail,
  };

  @override
  bool operator ==(Object other) =>
      other is Song &&
      other.key == key &&
      other.title == title &&
      other.artist == artist &&
      other.durationMs == durationMs &&
      other.thumbnail == thumbnail;

  @override
  int get hashCode => Object.hash(key, title, artist, durationMs, thumbnail);
}

List<Song> songs(Object? v) => _list(v, Song.fromJson);

enum RepeatMode { off, all, one }

@immutable
class QueueEntry {
  const QueueEntry({required this.id, required this.song, required this.radio});

  factory QueueEntry.fromJson(Json j) => QueueEntry(
    id: j['id']! as int,
    song: Song.fromJson(j['track']! as Json),
    radio: j['origin'] == 'radio',
  );

  final int id;
  final Song song;
  final bool radio;
}

/// `idle`, `loading`, `playing`, `paused`, `stalled` or `failed`, as the core reports it.
@immutable
class Transport {
  const Transport(this.state, {this.id, this.failure});

  factory Transport.fromJson(Json j) => Transport(
    j['state']! as String,
    id: j['id'] as int?,
    failure: j['failure'] == null ? null : _failureText(j['failure']! as Json),
  );

  final String state;
  final int? id;
  final String? failure;

  bool get playing => state == 'playing';
  bool get busy => state == 'loading' || state == 'stalled';
  bool get failed => state == 'failed';
}

String _failureText(Json f) => switch (f['kind']) {
  'offline' => 'You are offline.',
  'rateLimited' => 'The service is limiting requests. Try again soon.',
  'unavailable' => 'This song is not available.',
  'decode' => 'This song could not be decoded.',
  'expired' => 'The stream expired. Try again.',
  _ => (f['message'] as String?) ?? 'Playback failed.',
};

/// Everything about the player except the position, which has a provider of its own.
@immutable
class PlayerState {
  const PlayerState({
    required this.seq,
    required this.transport,
    required this.repeat,
    required this.shuffle,
    required this.autoplay,
    required this.canUndo,
    required this.queue,
    required this.current,
    required this.upcoming,
  });

  factory PlayerState.fromJson(Json j) => PlayerState(
    seq: j['seq']! as int,
    transport: Transport.fromJson(j['transport']! as Json),
    repeat: RepeatMode.values.byName(j['repeat']! as String),
    shuffle: j['shuffle']! as bool,
    autoplay: j['autoplay']! as bool,
    canUndo: j['canUndo']! as bool,
    queue: _list(j['queue'], QueueEntry.fromJson),
    current: j['current'] as int?,
    upcoming: [for (final i in j['upcoming']! as List<Object?>) i! as int],
  );

  final int seq;
  final Transport transport;
  final RepeatMode repeat;
  final bool shuffle;
  final bool autoplay;
  final bool canUndo;
  final List<QueueEntry> queue;
  final int? current;
  final List<int> upcoming;

  QueueEntry? get currentEntry {
    final id = current;
    if (id == null) return null;
    for (final e in queue) {
      if (e.id == id) return e;
    }
    return null;
  }

  QueueEntry? entry(int id) {
    for (final e in queue) {
      if (e.id == id) return e;
    }
    return null;
  }
}

@immutable
class PlaylistSummary {
  const PlaylistSummary({
    required this.id,
    required this.name,
    required this.trackCount,
    this.thumbnail,
  });

  factory PlaylistSummary.fromJson(Json j) => PlaylistSummary(
    id: j['id']! as int,
    name: j['name']! as String,
    trackCount: j['trackCount']! as int,
    thumbnail: j['thumbnail'] as String?,
  );

  final int id;
  final String name;
  final int trackCount;
  final String? thumbnail;
}

@immutable
class LibraryState {
  const LibraryState({
    required this.revision,
    required this.likedKeys,
    required this.playlists,
    required this.historyCount,
    this.error,
  });

  factory LibraryState.fromJson(Json j) => LibraryState(
    revision: j['revision']! as int,
    likedKeys: {for (final k in j['likedKeys']! as List<Object?>) k! as String},
    playlists: _list(j['playlists'], PlaylistSummary.fromJson),
    historyCount: j['historyCount']! as int,
    error: j['error'] as String?,
  );

  static const empty = LibraryState(
    revision: 0,
    likedKeys: {},
    playlists: [],
    historyCount: 0,
  );

  final int revision;
  final Set<String> likedKeys;
  final List<PlaylistSummary> playlists;
  final int historyCount;
  final String? error;
}

@immutable
class PlaylistEntry {
  const PlaylistEntry({required this.entryId, required this.song});

  final int entryId;
  final Song song;
}

@immutable
class PlaylistDetail {
  const PlaylistDetail({required this.id, required this.name, required this.entries});

  factory PlaylistDetail.fromJson(Json j) => PlaylistDetail(
    id: j['id']! as int,
    name: j['name']! as String,
    entries: _list(
      j['entries'],
      (e) => PlaylistEntry(
        entryId: e['entryId']! as int,
        song: Song.fromJson(e['track']! as Json),
      ),
    ),
  );

  final int id;
  final String name;
  final List<PlaylistEntry> entries;
}

@immutable
class HistoryItem {
  const HistoryItem({required this.song, required this.playedAt, required this.raw});

  factory HistoryItem.fromJson(Json j) => HistoryItem(
    song: Song.fromJson(j['track']! as Json),
    playedAt: j['playedAt']! as int,
    raw: j,
  );

  final Song song;
  final int playedAt;

  /// Kept as received, so a cleared history can be sent back for Undo.
  final Json raw;
}

@immutable
class HomeData {
  const HomeData({required this.quickPicks, required this.recent, required this.likedCount});

  factory HomeData.fromJson(Json j) => HomeData(
    quickPicks: songs(j['quickPicks']),
    recent: songs(j['recent']),
    likedCount: j['likedCount']! as int,
  );

  final List<Song> quickPicks;
  final List<Song> recent;
  final int likedCount;
}

@immutable
class AlbumCover {
  const AlbumCover({required this.id, required this.title, required this.artist, this.thumbnail});

  final String id;
  final String title;
  final String artist;
  final String? thumbnail;
}

@immutable
class PlaylistCover {
  const PlaylistCover({required this.id, required this.title, required this.subtitle, this.thumbnail});

  final String id;
  final String title;
  final String subtitle;
  final String? thumbnail;
}

/// One row of Explore or of a genre page: songs, albums or playlists.
@immutable
class Shelf {
  const Shelf({
    required this.id,
    required this.title,
    this.songs = const [],
    this.albums = const [],
    this.playlists = const [],
  });

  factory Shelf.fromJson(Json j) => Shelf(
    id: j['id']! as String,
    title: j['title']! as String,
    songs: j['kind'] == 'songs' ? _list(j['tracks'], Song.fromJson) : const [],
    albums: j['kind'] == 'albums'
        ? _list(
            j['albums'],
            (a) => AlbumCover(
              id: a['id']! as String,
              title: a['title']! as String,
              artist: a['artist']! as String,
              thumbnail: a['thumbnail'] as String?,
            ),
          )
        : const [],
    playlists: j['kind'] == 'playlists'
        ? _list(
            j['playlists'],
            (p) => PlaylistCover(
              id: p['id']! as String,
              title: p['title']! as String,
              subtitle: p['subtitle']! as String,
              thumbnail: p['thumbnail'] as String?,
            ),
          )
        : const [],
  );

  final String id;
  final String title;
  final List<Song> songs;
  final List<AlbumCover> albums;
  final List<PlaylistCover> playlists;
}

@immutable
class Genre {
  const Genre({required this.title, required this.params});

  final String title;
  final String params;
}

enum SearchKind { all, songs, videos, albums, artists, playlists }

@immutable
class SearchHit {
  const SearchHit.song(Song this.song) : key = null, title = null, thumbnail = null;

  const SearchHit.playlist({
    required String this.key,
    required String this.title,
    this.thumbnail,
  }) : song = null;

  const SearchHit.artist({
    required String this.key,
    required String this.title,
    this.thumbnail,
  }) : song = null;

  final Song? song;
  final String? key;
  final String? title;
  final String? thumbnail;
}

@immutable
class LyricLine {
  const LyricLine(this.timeMs, this.text);

  final int timeMs;
  final String text;
}

/// `synced`, `plain`, `instrumental`, `notFound` or `error`.
@immutable
class LyricsData {
  const LyricsData(this.kind, {this.lines = const [], this.text = ''});

  factory LyricsData.fromJson(Json j) => LyricsData(
    j['kind']! as String,
    lines: _list(j['lines'], (l) => LyricLine(l['timeMs']! as int, l['text']! as String)),
    text: (j['text'] as String?) ?? (j['message'] as String?) ?? '',
  );

  final String kind;
  final List<LyricLine> lines;
  final String text;
}

/// The sleep timer: `minutes` (with [remainingMs]), `endOfTrack` or `tracks` (with [left]).
@immutable
class SleepStatus {
  const SleepStatus(this.kind, {this.remainingMs = 0, this.left = 0});

  factory SleepStatus.fromJson(Json j) => SleepStatus(
    j['kind']! as String,
    remainingMs: (j['remainingMs'] as int?) ?? 0,
    left: (j['left'] as int?) ?? 0,
  );

  final String kind;
  final int remainingMs;
  final int left;

  String get label => switch (kind) {
    'minutes' => '${(remainingMs / 60000).ceil()} min left',
    'endOfTrack' => 'Stops after this song',
    _ => 'Stops after $left songs',
  };
}
