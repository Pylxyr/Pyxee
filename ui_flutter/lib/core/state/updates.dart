import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/system.dart';
import '../theme/prefs.dart';
import 'providers.dart';

enum UpdatePhase { idle, checking, installing }

@immutable
class UpdatesState {
  const UpdatesState({
    this.phase = UpdatePhase.idle,
    this.info,
    this.error,
    this.progress,
    this.changelogOpen = false,
  });

  final UpdatePhase phase;

  /// What the last check found.
  final AppUpdate? info;

  /// Why the last check or install failed. A failed check at start-up stays silent (offline is normal).
  final String? error;
  final UpdateProgress? progress;
  final bool changelogOpen;

  UpdatesState copyWith({
    UpdatePhase? phase,
    AppUpdate? info,
    bool clearInfo = false,
    String? error,
    bool clearError = false,
    UpdateProgress? progress,
    bool clearProgress = false,
    bool? changelogOpen,
  }) => UpdatesState(
    phase: phase ?? this.phase,
    info: clearInfo ? null : (info ?? this.info),
    error: clearError ? null : (error ?? this.error),
    progress: clearProgress ? null : (progress ?? this.progress),
    changelogOpen: changelogOpen ?? this.changelogOpen,
  );
}

/// The state of the Pyxee update itself, shared by Settings > Updates and the changelog sheet. A check that finds a
/// newer version opens the changelog at once, whether the person asked for the check or Pyxee did it by itself at
/// start-up.
class UpdatesNotifier extends Notifier<UpdatesState> {
  @override
  UpdatesState build() {
    ref.listen(updateProgressProvider, (_, next) {
      next.whenData((p) {
        // yt-dlp's own download is followed by its section in Settings.
        if (p.target == 'app' && state.phase == UpdatePhase.installing) {
          state = state.copyWith(progress: p);
        }
      });
    });
    return const UpdatesState();
  }

  /// The check Pyxee runs by itself when it starts. Silent unless a newer version exists.
  Future<void> checkAtStartup() => _check(report: false);

  /// The check behind the "Check for updates" button: failures are shown.
  Future<void> check() => _check(report: true);

  Future<void> _check({required bool report}) async {
    if (state.phase != UpdatePhase.idle) return;
    state = state.copyWith(phase: UpdatePhase.checking, clearError: report);
    try {
      final info = await ref.read(apiProvider).updateCheckApp(prereleases: ref.read(prefsProvider).checkPrereleases);
      state = state.copyWith(info: info, changelogOpen: info.available || state.changelogOpen);
    } on Object catch (e) {
      if (report) state = state.copyWith(clearInfo: true, error: errorText(e));
    }
    state = state.copyWith(phase: UpdatePhase.idle);
  }

  Future<void> install() async {
    if (state.phase == UpdatePhase.installing) return;
    state = state.copyWith(phase: UpdatePhase.installing, clearError: true, clearProgress: true);
    try {
      // Downloads and checks the installer and starts it; the core then asks the window to quit.
      await ref.read(apiProvider).updateInstallApp(prereleases: ref.read(prefsProvider).checkPrereleases);
    } on Object catch (e) {
      state = state.copyWith(error: errorText(e));
    }
    state = state.copyWith(phase: UpdatePhase.idle, clearProgress: true);
  }

  void openChangelog() => state = state.copyWith(changelogOpen: true);
  void closeChangelog() => state = state.copyWith(changelogOpen: false);
}

final updatesProvider = NotifierProvider<UpdatesNotifier, UpdatesState>(UpdatesNotifier.new);
