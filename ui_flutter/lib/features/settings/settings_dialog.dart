import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/state/providers.dart';
import '../../core/theme/prefs.dart';
import '../orbit/orbit_view.dart';

Future<void> showSettings(BuildContext context) => showDialog<void>(
  context: context,
  builder: (context) => const Dialog(
    child: ConstrainedBox(constraints: BoxConstraints(maxWidth: 480, maxHeight: 640), child: _Settings()),
  ),
);

class _Settings extends ConsumerWidget {
  const _Settings();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final prefs = ref.watch(prefsProvider);
    final notifier = ref.read(prefsProvider.notifier);
    final titleStyle = Theme.of(context).textTheme.titleSmall;
    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        Text('Settings', style: Theme.of(context).textTheme.titleLarge),
        const SizedBox(height: 16),
        Text('Theme', style: titleStyle),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            for (final t in pxThemes)
              ChoiceChip(
                avatar: CircleAvatar(backgroundColor: Color(t.seedValue), radius: 8),
                label: Text(t.name),
                selected: t.id == prefs.themeId,
                onSelected: (_) => notifier.update(prefs.copyWith(themeId: t.id)),
              ),
          ],
        ),
        const SizedBox(height: 12),
        SegmentedButton<ModeChoice>(
          segments: const [
            ButtonSegment(value: ModeChoice.system, label: Text('System')),
            ButtonSegment(value: ModeChoice.dark, label: Text('Dark')),
            ButtonSegment(value: ModeChoice.light, label: Text('Light')),
          ],
          selected: {prefs.mode},
          onSelectionChanged: (s) => notifier.update(prefs.copyWith(mode: s.first)),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Colours from the album art'),
          subtitle: const Text('The accent follows the cover of the song that plays.'),
          value: prefs.artColors,
          onChanged: (v) => notifier.update(prefs.copyWith(artColors: v)),
        ),
        if (micaSupported)
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Mica window material'),
            value: prefs.mica,
            onChanged: (v) => notifier.update(prefs.copyWith(mica: v)),
          ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Keep playing in the tray when closed'),
          value: prefs.closeToTray,
          onChanged: (v) => notifier.update(prefs.copyWith(closeToTray: v)),
        ),
        const SizedBox(height: 8),
        Text('Volume', style: titleStyle),
        const VolumeSlider(),
        const SizedBox(height: 8),
        Text('Sleep timer', style: titleStyle),
        const SizedBox(height: 8),
        const _SleepTimer(),
      ],
    );
  }
}

class _SleepTimer extends ConsumerWidget {
  const _SleepTimer();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(sleepProvider);
    final a = ref.read(actionsProvider);
    if (status != null) {
      return Row(
        spacing: 8,
        children: [
          Expanded(child: Text(status.label)),
          if (status.kind == 'minutes') OutlinedButton(onPressed: () => a.sleepExtend(15), child: const Text('+15 min')),
          FilledButton.tonal(onPressed: a.sleepCancel, child: const Text('Cancel')),
        ],
      );
    }
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final m in const [15, 30, 60])
          OutlinedButton(onPressed: () => a.sleepMinutes(m), child: Text('$m min')),
        OutlinedButton(onPressed: a.sleepEndOfTrack, child: const Text('End of song')),
      ],
    );
  }
}
