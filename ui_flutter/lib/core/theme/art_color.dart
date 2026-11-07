import 'dart:async';
import 'dart:collection';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/painting.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_color_utilities/material_color_utilities.dart';

import '../state/providers.dart';

const _hosts = ['ytimg.com', 'ggpht.com', 'googleusercontent.com', 'youtube.com'];
final _cache = LinkedHashMap<String, Color?>();

bool _coverHost(Uri u) => u.scheme == 'https' && _hosts.any((h) => u.host == h || u.host.endsWith('.$h'));

/// The dominant colour of a cover: the picture is fetched small (at most 512 KB, covers only), decoded at 48 px and
/// scored with Material's own quantizer, so the result is a colour that works as an accent.
Future<Color?> artColor(String url) async {
  if (_cache.containsKey(url)) return _cache[url];
  final color = await _extract(url);
  if (_cache.length >= 48) _cache.remove(_cache.keys.first);
  _cache[url] = color;
  return color;
}

Future<Color?> _extract(String url) async {
  final uri = Uri.tryParse(url);
  if (uri == null || !_coverHost(uri)) return null;
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 6);
  try {
    final request = await client.getUrl(uri);
    final response = await request.close().timeout(const Duration(seconds: 8));
    if (response.statusCode != 200) return null;
    final bytes = BytesBuilder(copy: false);
    await for (final chunk in response) {
      bytes.add(chunk);
      if (bytes.length > 512 * 1024) return null;
    }
    final codec = await ui.instantiateImageCodec(bytes.takeBytes(), targetWidth: 48);
    final frame = await codec.getNextFrame();
    final data = await frame.image.toByteData();
    frame.image.dispose();
    codec.dispose();
    if (data == null) return null;
    final pixels = <int>[];
    for (var i = 0; i + 3 < data.lengthInBytes; i += 4) {
      if (data.getUint8(i + 3) < 255) continue;
      final r = data.getUint8(i);
      final g = data.getUint8(i + 1);
      final b = data.getUint8(i + 2);
      pixels.add(0xFF000000 | (r << 16) | (g << 8) | b);
    }
    if (pixels.isEmpty) return null;
    final quantized = await QuantizerCelebi().quantize(pixels, 64);
    final ranked = Score.score(quantized.colorToCount, desired: 1);
    return ranked.isEmpty ? null : Color(ranked.first);
  } on Object {
    return null;
  } finally {
    client.close(force: true);
  }
}

/// The accent taken from the cover of the song that is playing, or null when there is none (yet). Changing it
/// retargets the theme, which animates over a few hundred milliseconds and then costs nothing.
class ArtSeedNotifier extends Notifier<Color?> {
  int _token = 0;

  @override
  Color? build() {
    ref.listen(
      playerProvider.select((p) => p.currentEntry?.song.thumbnail),
      (_, url) => unawaited(_load(url)),
      fireImmediately: true,
    );
    return null;
  }

  Future<void> _load(String? url) async {
    final mine = ++_token;
    if (url == null) {
      state = null;
      return;
    }
    final color = await artColor(url);
    if (ref.mounted && mine == _token && color != null) state = color;
  }
}

final artSeedProvider = NotifierProvider<ArtSeedNotifier, Color?>(ArtSeedNotifier.new);
