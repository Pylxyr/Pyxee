import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../state/providers.dart';

enum ModeChoice { system, dark, light }

/// A selectable accent. The album art can override it while a song plays (see `artSeedProvider`).
@immutable
class PxTheme {
  const PxTheme(this.id, this.name, this.seedValue);

  final String id;
  final String name;
  final int seedValue;
}

const pxThemes = [
  PxTheme('citrus', 'Citrus', 0xFFC8F169),
  PxTheme('ember', 'Ember', 0xFFFF8A5B),
  PxTheme('tide', 'Tide', 0xFF5BC0EB),
  PxTheme('orchid', 'Orchid', 0xFFC77DFF),
  PxTheme('steel', 'Steel', 0xFFB4BCC8),
];

/// Mica needs Windows 11 (build 22000 or later).
final bool micaSupported = () {
  if (!Platform.isWindows) return false;
  final build = RegExp(r'Build (\d+)').firstMatch(Platform.operatingSystemVersion);
  return (int.tryParse(build?.group(1) ?? '') ?? 0) >= 22000;
}();

@immutable
class Prefs {
  const Prefs({
    this.themeId = 'citrus',
    this.mode = ModeChoice.dark,
    this.artColors = true,
    this.mica = true,
    this.closeToTray = true,
    this.volume = 100,
  });

  factory Prefs.fromJson(Json j) => Prefs(
    themeId: (j['themeId'] as String?) ?? 'citrus',
    mode: ModeChoice.values.asNameMap()[j['mode']] ?? ModeChoice.dark,
    artColors: (j['artColors'] as bool?) ?? true,
    mica: (j['mica'] as bool?) ?? true,
    closeToTray: (j['closeToTray'] as bool?) ?? true,
    volume: ((j['volume'] as int?) ?? 100).clamp(0, 100),
  );

  final String themeId;
  final ModeChoice mode;
  final bool artColors;
  final bool mica;
  final bool closeToTray;
  final int volume;

  PxTheme get theme => pxThemes.firstWhere((t) => t.id == themeId, orElse: () => pxThemes.first);

  Prefs copyWith({
    String? themeId,
    ModeChoice? mode,
    bool? artColors,
    bool? mica,
    bool? closeToTray,
    int? volume,
  }) => Prefs(
    themeId: themeId ?? this.themeId,
    mode: mode ?? this.mode,
    artColors: artColors ?? this.artColors,
    mica: mica ?? this.mica,
    closeToTray: closeToTray ?? this.closeToTray,
    volume: volume ?? this.volume,
  );

  Json toJson() => {
    'themeId': themeId,
    'mode': mode.name,
    'artColors': artColors,
    'mica': mica,
    'closeToTray': closeToTray,
    'volume': volume,
  };
}

/// Read once in `main` (before the first frame) and handed in as an override.
final initialPrefsProvider = Provider<Prefs>((ref) => const Prefs());

class PrefsNotifier extends Notifier<Prefs> {
  Timer? _save;

  @override
  Prefs build() {
    ref.onDispose(() => _save?.cancel());
    return ref.read(initialPrefsProvider);
  }

  void update(Prefs next) {
    state = next;
    _save?.cancel();
    _save = Timer(const Duration(milliseconds: 400), () {
      unawaited(ref.read(apiProvider).savePrefs(next.toJson()));
    });
  }
}

final prefsProvider = NotifierProvider<PrefsNotifier, Prefs>(PrefsNotifier.new);

/// Output volume, 0-100: applied to the core at once and saved with the other settings.
class VolumeNotifier extends Notifier<int> {
  @override
  int build() => ref.read(initialPrefsProvider).volume;

  void set(int percent) {
    state = percent;
    ref.read(actionsProvider).setVolume(percent);
    final prefs = ref.read(prefsProvider);
    ref.read(prefsProvider.notifier).update(prefs.copyWith(volume: percent));
  }
}

final volumeProvider = NotifierProvider<VolumeNotifier, int>(VolumeNotifier.new);
