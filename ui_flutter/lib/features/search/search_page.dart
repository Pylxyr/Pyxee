import 'package:material_ui/material_ui.dart';

import '../../shared/widgets/page_placeholder.dart';

class SearchPage extends StatelessWidget {
  const SearchPage({super.key, required this.query});

  final String query;

  @override
  Widget build(BuildContext context) => PagePlaceholder('Search: $query');
}
