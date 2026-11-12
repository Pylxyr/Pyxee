import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/px_theme.dart';
import '../../core/state/providers.dart';
import '../../core/state/updates.dart';
import '../../shared/sheets.dart';
import '../../shared/widgets.dart';
import 'changelog.dart';
import 'updates_section.dart';

/// "What's new": opens by itself when a check finds a newer version, and from Settings > Updates.
Future<void> showChangelogSheet(BuildContext context, WidgetRef ref) {
  final updates = ref.read(updatesProvider.notifier);
  return showPxSheet<void>(
    context,
    title: "What's new",
    builder: (sheet) => Consumer(
      builder: (context, ref, _) {
        final p = context.px;
        final state = ref.watch(updatesProvider);
        final info = state.info;
        if (info == null) return const SizedBox.shrink();
        final blocks = parseChangelog(info.notes);
        final installing = state.phase == UpdatePhase.installing;
        // How many releases the notes cover, when the update spans more than one (the notes then have a heading each).
        final releases = blocks.where((b) => b is ChangelogHeading && isVersionHeading(b.text)).length;

        Widget block(int i, ChangelogBlock b) => switch (b) {
          ChangelogHeading(:final text) when isVersionHeading(text) => Padding(
            padding: EdgeInsets.only(top: i > 0 ? 16 : 0, bottom: 2),
            child: Text('Pyxee $text', style: PxText.lead(p.fg)),
          ),
          ChangelogHeading(:final text) => Padding(
            padding: EdgeInsets.only(top: i > 0 ? 12 : 0),
            child: Text(text, style: PxText.strong(p.fg)),
          ),
          ChangelogList(:final items) => Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                for (final item in items)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Padding(padding: const EdgeInsets.only(left: 4, right: 8), child: Text('•', style: PxText.body(p.muted))),
                        Expanded(child: Text(item, style: PxText.body(p.fg))),
                      ],
                    ),
                  ),
              ],
            ),
          ),
          ChangelogParagraph(:final text) => Padding(padding: const EdgeInsets.only(top: 4), child: Text(text, style: PxText.body(p.fg))),
        };

        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Text('Pyxee ${info.latest} is available', style: PxText.strong(p.fg)),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 4, 12, 0),
              child: Text(
                'You have ${info.current}.${releases > 1 ? ' Below is what changed in each of the $releases releases since.' : ''}',
                style: PxText.body(p.muted),
              ),
            ),
            Container(
              margin: const EdgeInsets.fromLTRB(12, 12, 12, 0),
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(color: p.surface, borderRadius: BorderRadius.circular(rMd), border: Border.all(color: p.rule)),
              child: blocks.isEmpty
                  ? Text('This release has no written changelog.', style: PxText.body(p.muted))
                  : Column(crossAxisAlignment: CrossAxisAlignment.start, children: [for (var i = 0; i < blocks.length; i++) block(i, blocks[i])]),
            ),
            if (installing)
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
                child: state.progress != null
                    ? PxUpdateProgress(progress: state.progress!, what: info.latest)
                    : Text('Downloading and checking the installer. Pyxee closes when it starts.', style: PxText.caption(p.muted)),
              ),
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
              child: Text(
                info.portable
                    ? 'This is a portable copy. Download the new zip from ${info.notesUrl} and unpack it over this folder; your data folder stays.'
                    : info.installable
                    ? 'Full release page: ${info.notesUrl}'
                    : 'This release has no installer with a checksum list attached, or this system cannot update in place. Download it from ${info.notesUrl}.',
                style: PxText.caption(p.muted),
              ),
            ),
            if (state.error != null)
              Padding(padding: const EdgeInsets.fromLTRB(12, 8, 12, 0), child: Text(state.error!, style: PxText.body(p.accent))),
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
              child: Wrap(
                alignment: WrapAlignment.end,
                spacing: 8,
                runSpacing: 8,
                children: [
                  PxButton(label: 'Later', small: true, onPressed: () => Navigator.of(sheet).pop()),
                  if (!info.installable)
                    PxButton(
                      label: 'Open release page',
                      small: true,
                      onPressed: () async {
                        try {
                          await ref.read(apiProvider).openReleasePage();
                        } on Object catch (e) {
                          ref.read(toastProvider.notifier).push(errorText(e));
                        }
                      },
                    ),
                  if (info.installable)
                    PxButton(
                      label: 'Download and install ${info.latest}',
                      small: true,
                      kind: PxButtonKind.primary,
                      onPressed: installing ? null : () => unawaited(updates.install()),
                    ),
                ],
              ),
            ),
          ],
        );
      },
    ),
  ).whenComplete(updates.closeChangelog);
}
