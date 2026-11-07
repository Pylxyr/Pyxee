import 'package:material_ui/material_ui.dart';

class PagePlaceholder extends StatelessWidget {
  const PagePlaceholder(this.label, {super.key});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(label, style: Theme.of(context).textTheme.headlineMedium),
    );
  }
}
