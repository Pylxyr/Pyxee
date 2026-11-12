import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/models/system.dart';
import '../../core/state/providers.dart';
import '../../core/state/updates.dart';
import '../../core/theme/prefs.dart';
import '../../shared/widgets.dart';

String _megabytes(int bytes) => (bytes / (1024 * 1024)).toStringAsFixed(1);

/// A download or install in progress: what it is doing, and a bar when the size is known.
class PxUpdateProgress extends StatelessWidget {
  const PxUpdateProgress({super.key, required this.progress, required this.what});

  final UpdateProgress progress;
  final String what;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final total = progress.total ?? 0;
    final known = total > 0;
    final label = switch (progress.phase) {
      'verifying' => 'Checking $what…',
      'testing' => 'Testing $what…',
      'starting' => 'Starting the installer…',
      _ => known ? 'Downloading $what… ${_megabytes(progress.received)} of ${_megabytes(total)} MB' : 'Downloading $what… ${_megabytes(progress.received)} MB',
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: PxText.caption(p.muted)),
        const SizedBox(height: 6),
        ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: LinearProgressIndicator(
            // A bar that fills while the size is known, an endless one while it is not or while it only checks.
            value: progress.phase == 'downloading' && known ? (progress.received / total).clamp(0.0, 1.0) : null,
            minHeight: 4,
            color: p.accent,
            backgroundColor: p.surface2,
          ),
        ),
      ],
    );
  }
}

/// Settings > Updates: Pyxee itself (the GitHub releases) and yt-dlp, the part that finds the music.
class UpdatesSection extends ConsumerStatefulWidget {
  const UpdatesSection({super.key});

  @override
  ConsumerState<UpdatesSection> createState() => _UpdatesSectionState();
}

enum _ToolPhase { idle, checking, installing }

class _UpdatesSectionState extends ConsumerState<UpdatesSection> {
  _ToolPhase _toolPhase = _ToolPhase.idle;
  YtdlpUpdate? _tool;
  String? _toolInstalled;
  String? _toolError;

  Future<void> _checkTool() async {
    setState(() {
      _toolPhase = _ToolPhase.checking;
      _toolError = null;
      _toolInstalled = null;
    });
    try {
      final tool = await ref.read(apiProvider).updateCheckYtdlp();
      if (mounted) setState(() => _tool = tool);
    } on Object catch (e) {
      if (mounted) {
        setState(() {
          _tool = null;
          _toolError = errorText(e);
        });
      }
    }
    if (mounted) setState(() => _toolPhase = _ToolPhase.idle);
  }

  Future<void> _installTool() async {
    setState(() {
      _toolPhase = _ToolPhase.installing;
      _toolError = null;
    });
    try {
      final version = await ref.read(apiProvider).updateInstallYtdlp();
      if (mounted) setState(() => _toolInstalled = version);
      try {
        final tool = await ref.read(apiProvider).updateCheckYtdlp();
        if (mounted) setState(() => _tool = tool);
      } on Object {
        if (mounted) setState(() => _tool = YtdlpUpdate(available: false, currentVersion: version));
      }
    } on Object catch (e) {
      if (mounted) setState(() => _toolError = errorText(e));
    }
    if (mounted) setState(() => _toolPhase = _ToolPhase.idle);
  }

  Future<void> _openReleasePage() async {
    try {
      await ref.read(apiProvider).openReleasePage();
    } on Object catch (e) {
      ref.read(toastProvider.notifier).push(errorText(e));
    }
  }

  Widget _well(BuildContext context, List<Widget> children) {
    final p = context.px;
    return Container(
      margin: const EdgeInsets.fromLTRB(12, 8, 12, 0),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(color: p.surface, borderRadius: BorderRadius.circular(rMd), border: Border.all(color: p.rule)),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: children),
    );
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final updates = ref.watch(updatesProvider);
    final notifier = ref.read(updatesProvider.notifier);
    final prefs = ref.watch(prefsProvider);
    final info = updates.info;
    final idle = updates.phase == UpdatePhase.idle;
    final toolProgress = ref.watch(updateProgressProvider).value;

    final status = switch (updates.phase) {
      UpdatePhase.checking => 'Checking for a newer version…',
      UpdatePhase.installing => 'Downloading and checking the installer. Pyxee closes when it starts.',
      UpdatePhase.idle when info != null && info.available => 'Version ${info.latest} is available (you have ${info.current}).',
      UpdatePhase.idle when info != null => 'You have the latest version (${info.current}).',
      UpdatePhase.idle => "Looks at the GitHub releases. The installer's SHA-256 is checked before it runs.",
    };
    final toolStatus = switch (_toolPhase) {
      _ToolPhase.checking => 'Checking…',
      _ToolPhase.installing => 'Downloading, checking and testing the new yt-dlp…',
      _ToolPhase.idle when _toolInstalled != null => 'Updated to $_toolInstalled. It is used from the next search.',
      _ToolPhase.idle when _tool != null && _tool!.available => 'A newer yt-dlp is available${_tool!.currentVersion != null ? ' (you have ${_tool!.currentVersion})' : ''}.',
      _ToolPhase.idle when _tool != null => 'yt-dlp is up to date${_tool!.currentVersion != null ? ' (${_tool!.currentVersion})' : ''}.',
      _ToolPhase.idle => "Downloads are checked against yt-dlp's published SHA-256 and test-run before they replace the current one; if anything fails, nothing changes.",
    };

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _well(context, [
          Text('Pyxee', style: PxText.strong(p.fg)),
          const SizedBox(height: 4),
          Text(status, style: PxText.body(p.muted)),
          if (updates.phase == UpdatePhase.installing && updates.progress != null) ...[
            const SizedBox(height: 12),
            PxUpdateProgress(progress: updates.progress!, what: info?.latest ?? 'Pyxee'),
          ],
          if (info != null && info.available && info.portable)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text('This is a portable copy. Download the new zip from ${info.notesUrl} and unpack it over this folder; your data folder stays.', style: PxText.caption(p.muted)),
            )
          else if (info != null && info.available && !info.installable)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text('This release has no installer with a checksum list attached, or this system cannot update in place. Download it from ${info.notesUrl}.', style: PxText.caption(p.muted)),
            ),
          if (updates.error != null)
            Padding(padding: const EdgeInsets.only(top: 4), child: Text(updates.error!, style: PxText.body(p.accent))),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              PxButton(label: 'Check for updates', small: true, onPressed: idle ? () => unawaited(notifier.check()) : null),
              if (info != null && info.available) ...[
                PxButton(label: "What's new", small: true, onPressed: updates.phase == UpdatePhase.installing ? null : notifier.openChangelog),
                if (!info.installable) PxButton(label: 'Open release page', small: true, onPressed: () => unawaited(_openReleasePage())),
                if (info.installable)
                  PxButton(label: 'Download and install ${info.latest}', small: true, kind: PxButtonKind.primary, onPressed: idle ? () => unawaited(notifier.install()) : null),
              ],
            ],
          ),
          PxToggle(
            label: 'Include pre-releases',
            hint: 'Also offers release candidates (versions such as 0.6.0-rc1). They are for testing and may have bugs. A copy that already is a pre-release keeps getting them whatever this says.',
            checked: prefs.checkPrereleases,
            onChanged: (on) {
              ref.read(prefsProvider.notifier).update(prefs.copyWith(checkPrereleases: on));
              if (ref.read(updatesProvider).phase == UpdatePhase.idle) unawaited(notifier.check());
            },
          ),
        ]),
        _well(context, [
          Text('yt-dlp (finds the music)', style: PxText.strong(p.fg)),
          const SizedBox(height: 4),
          Text(toolStatus, style: PxText.body(p.muted)),
          if (_toolPhase == _ToolPhase.installing && toolProgress != null && toolProgress.target == 'ytdlp') ...[
            const SizedBox(height: 12),
            PxUpdateProgress(progress: toolProgress, what: 'yt-dlp'),
          ],
          if (_toolError != null) Padding(padding: const EdgeInsets.only(top: 4), child: Text(_toolError!, style: PxText.body(p.accent))),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              PxButton(label: 'Check yt-dlp', small: true, onPressed: _toolPhase == _ToolPhase.idle ? () => unawaited(_checkTool()) : null),
              if (_tool != null && _tool!.available && _toolInstalled == null)
                PxButton(label: 'Update yt-dlp', small: true, kind: PxButtonKind.primary, onPressed: _toolPhase == _ToolPhase.idle ? () => unawaited(_installTool()) : null),
            ],
          ),
        ]),
      ],
    );
  }
}
