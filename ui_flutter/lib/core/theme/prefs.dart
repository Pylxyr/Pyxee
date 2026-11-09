import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../state/providers.dart';

/// Troubleshooting switches: `PYXEE_OFF=mica,tray,media` (comma separated) leaves those features out at startup.
bool featureOff(String name) => (Platform.environment['PYXEE_OFF'] ?? '').split(',').contains(name);

/// Mica needs Windows 11 (build 22000 or later).
final bool micaSupported = () {
  if (!Platform.isWindows || featureOff('mica') || featureOff('acrylic')) return false;
  final build = RegExp(r'Build (\d+)').firstMatch(Platform.operatingSystemVersion);
  return (int.tryParse(build?.group(1) ?? '') ?? 0) >= 22000;
}();

@immutable
class Prefs {
  const Prefs({
    this.followSystem = true,
    this.themeId = 'dark',
    this.lightId = 'light',
    this.darkId = 'dark',
    this.artColors = false,
    this.mica = true,
    this.closeToTray = true,
    this.volume = 80,
  });

  factory Prefs.fromJson(Json j) => Prefs(
    followSystem: (j['followSystem'] as bool?) ?? true,
    themeId: (j['themeId'] as String?) ?? 'dark',
    lightId: (j['lightId'] as String?) ?? 'light',
    darkId: (j['darkId'] as String?) ?? 'dark',
    artColors: (j['artColors'] as bool?) ?? false,
    mica: (j['mica'] as bool?) ?? true,
    closeToTray: (j['closeToTray'] as bool?) ?? true,
    volume: ((j['volume'] as int?) ?? 80).clamp(0, 100),
  );

  /// Use [lightId] or [darkId] according to the system's light/dark setting.
  final bool followSystem;

  /// The theme used when not following the system.
  final String themeId;
  final String lightId;
  final String darkId;

  /// Take the whole palette from the cover of the song that is playing.
  final bool artColors;
  final bool mica;
  final bool closeToTray;
  final int volume;

  Prefs copyWith({
    bool? followSystem,
    String? themeId,
    String? lightId,
    String? darkId,
    bool? artColors,
    bool? mica,
    bool? closeToTray,
    int? volume,
  }) => Prefs(
    followSystem: followSystem ?? this.followSystem,
    themeId: themeId ?? this.themeId,
    lightId: lightId ?? this.lightId,
    darkId: darkId ?? this.darkId,
    artColors: artColors ?? this.artColors,
    mica: mica ?? this.mica,
    closeToTray: closeToTray ?? this.closeToTray,
    volume: volume ?? this.volume,
  );

  Json toJson() => {
    'followSystem': followSystem,
    'themeId': themeId,
    'lightId': lightId,
    'darkId': darkId,
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
