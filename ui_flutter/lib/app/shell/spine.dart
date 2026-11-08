import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../features/settings/settings_dialog.dart';
import '../../shared/widgets/basics.dart';
import '../places.dart';
import '../theme/metrics.dart';

/// The places: a labelled column down the left edge on wide windows, a row of icons along the bottom on narrow ones.
class Spine extends StatelessWidget {
  const Spine({super.key, required this.axis, required this.current});

  final Axis axis;
  final Place? current;

  @override
  Widget build(BuildContext context) {
    final compact = axis == Axis.horizontal;
    final items = [
      for (final p in Place.values)
        _NavItem(
          label: p.label,
          icon: p == current ? p.activeIcon : p.icon,
          selected: p == current,
          compact: compact,
          onTap: () => context.go(p.path),
        ),
    ];
    final settings = _NavItem(
      label: 'Settings',
      icon: Icons.tune,
      selected: false,
      compact: compact,
      onTap: () => showSettings(context),
    );
    if (!compact) {
      return SizedBox(
        width: Metrics.spineWidth,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 12, 4, 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            spacing: 2,
            children: [...items, const Spacer(), settings],
          ),
        ),
      );
    }
    return SafeArea(
      top: false,
      child: SizedBox(
        height: Metrics.spineHeight,
        child: Row(mainAxisAlignment: MainAxisAlignment.spaceEvenly, children: [...items, settings]),
      ),
    );
  }
}

class _NavItem extends StatelessWidget {
  const _NavItem({
    required this.label,
    required this.icon,
    required this.selected,
    required this.compact,
    required this.onTap,
  });

  final String label;
  final IconData icon;
  final bool selected;
  final bool compact;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final color = selected ? scheme.primary : scheme.onSurfaceVariant;
    return PxPress(
      onTap: onTap,
      builder: (context, hovered) => Tooltip(
        message: compact ? label : '',
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: selected
                ? scheme.primary.withValues(alpha: 0.14)
                : hovered
                ? scheme.onSurface.withValues(alpha: 0.07)
                : const Color(0x00000000),
            borderRadius: BorderRadius.circular(10),
          ),
          child: SizedBox(
            height: 40,
            width: compact ? 56 : null,
            child: Row(
              mainAxisAlignment: compact ? MainAxisAlignment.center : MainAxisAlignment.start,
              children: [
                if (!compact) const SizedBox(width: 12),
                Icon(icon, size: 20, color: color),
                if (!compact) ...[
                  const SizedBox(width: 12),
                  Text(
                    label,
                    style: TextStyle(fontSize: 14, fontWeight: selected ? FontWeight.w600 : FontWeight.w500, color: color),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
