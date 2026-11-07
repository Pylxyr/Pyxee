import 'package:material_ui/material_ui.dart';

import '../../shared/widgets/page_placeholder.dart';

class PlaylistPage extends StatelessWidget {
  const PlaylistPage({super.key, required this.id});

  final int? id;

  @override
  Widget build(BuildContext context) => PagePlaceholder('Playlist $id');
}
