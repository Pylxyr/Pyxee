import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/colors.dart';
import '../../app/theme/px_theme.dart';
import '../../core/state/providers.dart';
import '../../core/theme/prefs.dart';
import '../../shared/icons.dart';
import '../../shared/sheets.dart';
import '../../shared/widgets.dart';

/// Settings: a tab that slides in from the right edge (the Settings button is in the top right corner).
Future<void> showSettingsSheet(BuildContext context) => showPxSheet<void>(
  context,
  title: 'Settings',
  right: true,
  builder: (_) => const _Settings(),
);

class _Settings extends ConsumerWidget {
  const _Settings();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final p = context.px;
    final prefs = ref.watch(prefsProvider);
    final notifier = ref.read(prefsProvider.notifier);
    final sleep = ref.watch(sleepProvider);
    final selected = prefs.followSystem ? (p.dark ? prefs.darkId : prefs.lightId) : prefs.themeId;
    final darks = builtinThemes.where((t) => t.dark).toList();
    final lights = builtinThemes.where((t) => !t.dark).toList();

    Widget section(String title, List<Widget> children, {bool first = false}) => Container(
      padding: EdgeInsets.only(top: first ? 0 : 16, bottom: 16),
      decoration: BoxDecoration(border: first ? null : Border(top: BorderSide(color: p.line))),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(padding: const EdgeInsets.fromLTRB(12, 0, 12, 4), child: Text(title, style: PxText.button(p.fg, FontWeight.w600))),
          ...children,
        ],
      ),
    );

    Widget chip(ThemeDef t) {
      final on = selected == t.id;
      final c = t.colors;
      return PxPress(
        onTap: () => notifier.update(prefs.copyWith(followSystem: false, themeId: t.id)),
        semanticLabel: t.name,
        builder: (context, hovered, _) => Tooltip(
          message: t.description,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: on || hovered ? p.surface2 : p.surface,
              borderRadius: BorderRadius.circular(rMd),
              border: Border.all(color: on ? p.accent : Colors.transparent, width: 2),
            ),
            child: Row(
              children: [
                ClipRRect(
                  borderRadius: BorderRadius.circular(10),
                  child: Container(
                    decoration: BoxDecoration(border: Border.all(color: p.line), borderRadius: BorderRadius.circular(10)),
                    child: Row(mainAxisSize: MainAxisSize.min, children: [for (final col in [c.bg, c.surface2, c.fg, c.accent]) Container(width: 10, height: 20, color: col)]),
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(child: Text(t.name, style: PxText.body(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis)),
              ],
            ),
          ),
        ),
      );
    }

    Widget grid(List<ThemeDef> items) => LayoutBuilder(
      builder: (context, box) {
        final w = (box.maxWidth - 8) / 2;
        return Wrap(spacing: 8, runSpacing: 8, children: [for (final t in items) SizedBox(width: w, child: chip(t))]);
      },
    );

    Widget themeSelect(String label, String value, List<ThemeDef> items, ValueChanged<String> onChanged) => Expanded(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: PxText.caption(p.muted)),
          const SizedBox(height: 4),
          PopupMenuButton<String>(
            tooltip: label,
            color: p.solid2,
            onSelected: onChanged,
            itemBuilder: (_) => [for (final t in items) PopupMenuItem(value: t.id, child: Text(t.name, style: PxText.body(p.fg)))],
            child: Container(
              height: 32,
              padding: const EdgeInsets.symmetric(horizontal: 12),
              decoration: BoxDecoration(color: p.surface, borderRadius: BorderRadius.circular(rSm), border: Border(bottom: BorderSide(color: p.muted), top: BorderSide(color: p.rule), left: BorderSide(color: p.rule), right: BorderSide(color: p.rule))),
              child: Row(children: [Expanded(child: Text(themeById(value).name, style: PxText.body(p.fg))), IconTheme(data: IconThemeData(color: p.muted), child: const PxIcon('down', size: 16))]),
            ),
          ),
        ],
      ),
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        section('Appearance', [
          // With "Color from album art" on, the cover is the theme, so these are switched off.
          Opacity(
            opacity: prefs.artColors ? 0.4 : 1,
            child: IgnorePointer(
              ignoring: prefs.artColors,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  PxToggle(
                    label: 'Match system',
                    hint: 'Use one theme when your system is light and another when it is dark.',
                    checked: prefs.followSystem,
                    onChanged: (on) => notifier.update(prefs.copyWith(followSystem: on)),
                  ),
                  if (prefs.followSystem)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
                      child: Row(
                        spacing: 12,
                        children: [
                          themeSelect('When light', prefs.lightId, lights, (id) => notifier.update(prefs.copyWith(lightId: id))),
                          themeSelect('When dark', prefs.darkId, darks, (id) => notifier.update(prefs.copyWith(darkId: id))),
                        ],
                      ),
                    ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(8, 8, 8, 8),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Padding(padding: const EdgeInsets.fromLTRB(4, 8, 4, 8), child: Text('Dark', style: PxText.button(p.fg, FontWeight.w600))),
                        grid(darks),
                        Padding(padding: const EdgeInsets.fromLTRB(4, 16, 4, 8), child: Text('Light', style: PxText.button(p.fg, FontWeight.w600))),
                        grid(lights),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
          PxToggle(
            label: 'Color from album art',
            hint: 'The cover of the song that is playing becomes the whole theme: background, panels, text and accent. The themes above are off while this is on.',
            checked: prefs.artColors,
            onChanged: (on) => notifier.update(prefs.copyWith(artColors: on)),
          ),
        ], first: true),
        section(micaSupported ? 'Windows' : 'System', [
          PxToggle(
            label: 'Keep playing in the tray when the window is closed',
            checked: prefs.closeToTray,
            onChanged: (on) => notifier.update(prefs.copyWith(closeToTray: on)),
          ),
          if (micaSupported)
            PxToggle(
              label: 'Mica window material',
              hint: 'Lets the wallpaper tint the window background. Needs Windows 11.',
              checked: prefs.mica,
              onChanged: (on) => notifier.update(prefs.copyWith(mica: on)),
            ),
          PxMenuItem(
            onTap: () {
              Navigator.of(context).pop();
              unawaited(showSleepSheet(context, ref));
            },
            child: Row(children: [const Expanded(child: Text('Sleep timer')), Text(sleep?.label ?? 'Off', style: PxText.body(p.muted))]),
          ),
        ]),
      ],
    );
  }
}

/// The sleep timer: pauses playback by itself.
Future<void> showSleepSheet(BuildContext context, WidgetRef ref) {
  final actions = ref.read(actionsProvider);
  final toast = ref.read(toastProvider.notifier);
  final options = <(String, VoidCallback)>[
    ('15 minutes', () => actions.sleepMinutes(15)),
    ('30 minutes', () => actions.sleepMinutes(30)),
    ('45 minutes', () => actions.sleepMinutes(45)),
    ('1 hour', () => actions.sleepMinutes(60)),
    ('90 minutes', () => actions.sleepMinutes(90)),
    ('End of this song', actions.sleepEndOfTrack),
    ('After 2 songs', () => actions.sleepTracks(2)),
    ('After 3 songs', () => actions.sleepTracks(3)),
  ];
  return showPxSheet<void>(
    context,
    title: 'Sleep timer',
    builder: (sheet) => Consumer(
      builder: (context, ref, _) {
        final p = context.px;
        final status = ref.watch(sleepProvider);
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (status != null)
              Container(
                margin: const EdgeInsets.only(bottom: 8),
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(borderRadius: BorderRadius.circular(rMd), border: Border.all(color: p.rule)),
                child: Row(
                  children: [
                    Expanded(child: Text(status.label, style: PxText.strong(p.fg))),
                    if (status.kind == 'minutes') ...[PxButton(label: '+15 min', small: true, onPressed: () => actions.sleepExtend(15)), const SizedBox(width: 8)],
                    PxButton(label: 'Turn off', small: true, kind: PxButtonKind.primary, onPressed: actions.sleepCancel),
                  ],
                ),
              ),
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
              child: Text('Playback pauses by itself. A timer in minutes fades the volume out over its last 15 seconds.', style: PxText.caption(p.muted)),
            ),
            for (final o in options)
              PxMenuItem(
                onTap: () {
                  o.$2();
                  Navigator.of(sheet).pop();
                  toast.push('Sleep timer: ${o.$1.toLowerCase()}', actionLabel: 'Turn off', onAction: actions.sleepCancel);
                },
                child: Text(o.$1),
              ),
          ],
        );
      },
    ),
  );
}
