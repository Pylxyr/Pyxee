import 'package:material_ui/material_ui.dart';

/// The three places of the app. Queue and lyrics belong to Orbit, the full-screen player, not to a place.
enum Place {
  discover('/discover', 'Discover', Icons.auto_awesome_outlined, Icons.auto_awesome),
  collection('/collection', 'Collection', Icons.bookmarks_outlined, Icons.bookmarks),
  search('/search', 'Search', Icons.search, Icons.search);

  const Place(this.path, this.label, this.icon, this.activeIcon);

  final String path;
  final String label;
  final IconData icon;
  final IconData activeIcon;

  static Place? of(String path) {
    for (final p in values) {
      if (path.startsWith(p.path)) return p;
    }
    return null;
  }
}
