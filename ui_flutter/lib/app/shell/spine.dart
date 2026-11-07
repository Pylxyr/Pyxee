import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../features/settings/settings_dialog.dart';
import '../places.dart';
import '../theme/metrics.dart';

/// The places as a row of icons: a column down the left edge on wide windows, a row along the bottom on narrow ones.
class Spine extends StatelessWidget {
  const Spine({super.key, required this.axis, required this.current});

  final Axis axis;
  final Place? current;

  @override
  Widget build(BuildContext context) {
    final items = [
      for (final p in Place.values) _SpineItem(place: p, selected: p == current),
      IconButton(
        onPressed: () => showSettings(context),
        tooltip: 'Settings',
        icon: const Icon(Icons.tune),
      ),
    ];
    if (axis == Axis.vertical) {
      return SizedBox(
        width: Metrics.spineWidth,
        child: Column(mainAxisAlignment: MainAxisAlignment.center, spacing: 8, children: items),
      );
    }
    return SafeArea(
      top: false,
      child: SizedBox(
        height: Metrics.spineHeight,
        child: Row(mainAxisAlignment: MainAxisAlignment.spaceEvenly, children: items),
      ),
    );
  }
}

class _SpineItem extends StatelessWidget {
  const _SpineItem({required this.place, required this.selected});

  final Place place;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    void open() => context.go(place.path);
    return selected
        ? IconButton.filledTonal(onPressed: open, tooltip: place.label, icon: Icon(place.activeIcon))
        : IconButton(onPressed: open, tooltip: place.label, icon: Icon(place.icon));
  }
}
