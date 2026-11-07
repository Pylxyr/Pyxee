import 'package:material_ui/material_ui.dart';

class Destination {
  const Destination(this.path, this.label, this.icon);

  final String path;
  final String label;
  final IconData icon;
}

/// The four top-level places. The tabs of wide windows and the bar of narrow ones show the same list.
const destinations = <Destination>[
  Destination('/home', 'Home', Icons.home_outlined),
  Destination('/explore', 'Explore', Icons.explore_outlined),
  Destination('/library', 'Library', Icons.library_music_outlined),
  Destination('/queue', 'Queue', Icons.queue_music_outlined),
];
