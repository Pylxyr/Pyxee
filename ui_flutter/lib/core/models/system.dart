import 'models.dart';

/// One setting kept in the `.env` file in the data folder (Settings > Advanced).
class EnvVar {
  const EnvVar(this.name, this.value, {this.fromWindows = false});

  factory EnvVar.fromJson(Json j) => EnvVar(
    j['name']! as String,
    (j['value'] as String?) ?? '',
    fromWindows: (j['fromWindows'] as bool?) ?? false,
  );

  final String name;
  final String value;

  /// The system sets it too, and that value is the one used.
  final bool fromWindows;

  Json toJson() => {'name': name, 'value': value, 'fromWindows': fromWindows};
}

/// The `.env` file (null when there is no data folder to keep it in) and its settings, in display order.
class EnvSettings {
  const EnvSettings(this.file, this.vars);

  factory EnvSettings.fromJson(Json j) => EnvSettings(
    j['file'] as String?,
    [for (final v in (j['vars'] as List<Object?>?) ?? const []) EnvVar.fromJson(v! as Json)],
  );

  final String? file;
  final List<EnvVar> vars;
}

/// What looking at the GitHub releases found.
class AppUpdate {
  const AppUpdate({
    required this.current,
    required this.latest,
    required this.available,
    required this.notesUrl,
    required this.notes,
    required this.installable,
    required this.portable,
  });

  factory AppUpdate.fromJson(Json j) => AppUpdate(
    current: (j['current'] as String?) ?? '',
    latest: (j['latest'] as String?) ?? '',
    available: (j['available'] as bool?) ?? false,
    notesUrl: (j['notesUrl'] as String?) ?? '',
    notes: (j['notes'] as String?) ?? '',
    installable: (j['installable'] as bool?) ?? false,
    portable: (j['portable'] as bool?) ?? false,
  );

  final String current;
  final String latest;
  final bool available;

  /// The release page, always there so there is something to open even when it cannot be installed in place.
  final String notesUrl;

  /// The release description (Markdown); empty when there is none.
  final String notes;

  /// The release has an installer and a checksum list, and this copy is an installed one.
  final bool installable;

  /// A portable copy is replaced by unpacking the new zip, never by running an installer.
  final bool portable;
}

class YtdlpUpdate {
  const YtdlpUpdate({required this.available, this.currentVersion});

  factory YtdlpUpdate.fromJson(Json j) => YtdlpUpdate(
    available: (j['available'] as bool?) ?? false,
    currentVersion: j['currentVersion'] as String?,
  );

  final bool available;
  final String? currentVersion;
}

/// Where an install is. `total` is unknown when the server sends no length.
class UpdateProgress {
  const UpdateProgress({required this.target, required this.phase, required this.received, this.total});

  factory UpdateProgress.fromJson(Json j) => UpdateProgress(
    target: (j['target'] as String?) ?? 'app',
    phase: (j['phase'] as String?) ?? 'downloading',
    received: (j['received'] as num?)?.toInt() ?? 0,
    total: (j['total'] as num?)?.toInt(),
  );

  /// `app` or `ytdlp`.
  final String target;

  /// `downloading`, `verifying`, `testing` or `starting`.
  final String phase;
  final int received;
  final int? total;
}
