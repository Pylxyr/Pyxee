import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/theme/colors.dart';
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
    this.discordPresence = true,
    this.checkPrereleases = false,
    this.volume = 80,
    this.custom = const [],
  });

  factory Prefs.fromJson(Json j) {
    final custom = <ThemeDef>[];
    final raw = j['custom'];
    if (raw is List) {
      for (final c in raw) {
        final def = themeFromJson(c);
        if (def != null && def.id.isNotEmpty && !builtinThemes.any((t) => t.id == def.id) && !custom.any((t) => t.id == def.id) && custom.length < maxCustomThemes) {
          custom.add(def);
        }
      }
    }
    return Prefs._fromJson(j, custom);
  }

  factory Prefs._fromJson(Json j, List<ThemeDef> custom) => Prefs(
    followSystem: (j['followSystem'] as bool?) ?? true,
    themeId: (j['themeId'] as String?) ?? 'dark',
    lightId: (j['lightId'] as String?) ?? 'light',
    darkId: (j['darkId'] as String?) ?? 'dark',
    artColors: (j['artColors'] as bool?) ?? false,
    mica: (j['mica'] as bool?) ?? true,
    closeToTray: (j['closeToTray'] as bool?) ?? true,
    discordPresence: (j['discordPresence'] as bool?) ?? true,
    checkPrereleases: (j['checkPrereleases'] as bool?) ?? false,
    volume: ((j['volume'] as int?) ?? 80).clamp(0, 100),
    custom: custom,
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

  /// Discord Rich Presence ("Listening to Pyxee"). Only does anything in a build that has a Discord application ID.
  final bool discordPresence;

  /// Settings > Updates > Include pre-releases.
  final bool checkPrereleases;
  final int volume;

  /// Themes the user made or imported.
  final List<ThemeDef> custom;

  Prefs copyWith({
    bool? followSystem,
    String? themeId,
    String? lightId,
    String? darkId,
    bool? artColors,
    bool? mica,
    bool? closeToTray,
    bool? discordPresence,
    bool? checkPrereleases,
    int? volume,
    List<ThemeDef>? custom,
  }) => Prefs(
    followSystem: followSystem ?? this.followSystem,
    themeId: themeId ?? this.themeId,
    lightId: lightId ?? this.lightId,
    darkId: darkId ?? this.darkId,
    artColors: artColors ?? this.artColors,
    mica: mica ?? this.mica,
    closeToTray: closeToTray ?? this.closeToTray,
    discordPresence: discordPresence ?? this.discordPresence,
    checkPrereleases: checkPrereleases ?? this.checkPrereleases,
    volume: volume ?? this.volume,
    custom: custom ?? this.custom,
  );

  Json toJson() => {
    'followSystem': followSystem,
    'themeId': themeId,
    'lightId': lightId,
    'darkId': darkId,
    'artColors': artColors,
    'mica': mica,
    'closeToTray': closeToTray,
    'discordPresence': discordPresence,
    'checkPrereleases': checkPrereleases,
    'volume': volume,
    'custom': [for (final t in custom) t.toJson()],
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

  /// Saves a custom theme: a new one when [def] has no id (or an unknown one), else replaces it. Returns what was
  /// stored; throws a [StateError] with a sentence when there is no room.
  ThemeDef saveCustom(ThemeDef def) {
    final prefs = state;
    final existing = def.id.isNotEmpty && prefs.custom.any((t) => t.id == def.id);
    if (!existing && prefs.custom.length >= maxCustomThemes) {
      throw StateError('You can keep up to $maxCustomThemes custom themes. Delete one first.');
    }
    final id = existing ? def.id : _newId(prefs.custom);
    final saved = ThemeDef(id, def.name.trim().isEmpty ? 'My theme' : def.name.trim(), def.dark, 'Custom theme', def.colors);
    final custom = existing ? [for (final t in prefs.custom) t.id == id ? saved : t] : [...prefs.custom, saved];
    // A theme switched from dark to light can no longer serve as the system-dark theme (and vice versa).
    bool ok(String themeId, bool dark) => themeById(themeId, custom).dark == dark;
    update(prefs.copyWith(
      custom: custom,
      darkId: ok(prefs.darkId, true) ? prefs.darkId : defaultDarkId,
      lightId: ok(prefs.lightId, false) ? prefs.lightId : defaultLightId,
    ));
    return saved;
  }

  void deleteCustom(String id) {
    final prefs = state;
    if (!prefs.custom.any((t) => t.id == id)) return;
    update(prefs.copyWith(
      custom: [for (final t in prefs.custom) if (t.id != id) t],
      themeId: prefs.themeId == id ? defaultDarkId : prefs.themeId,
      darkId: prefs.darkId == id ? defaultDarkId : prefs.darkId,
      lightId: prefs.lightId == id ? defaultLightId : prefs.lightId,
    ));
  }

  /// Picks one theme for good (and stops following the system).
  void select(String id) => update(state.copyWith(followSystem: false, themeId: id));

  String _newId(List<ThemeDef> custom) {
    var id = 'custom-${DateTime.now().millisecondsSinceEpoch.toRadixString(36)}';
    var n = 0;
    while (custom.any((t) => t.id == id)) {
      id = 'custom-${DateTime.now().millisecondsSinceEpoch.toRadixString(36)}${n++}';
    }
    return id;
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
