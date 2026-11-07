import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import 'router.dart';
import 'theme/theme.dart';

class PyxeeApp extends ConsumerWidget {
  const PyxeeApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return MaterialApp.router(
      title: 'Pyxee',
      debugShowCheckedModeBanner: false,
      theme: pyxeeTheme(),
      routerConfig: ref.watch(routerProvider),
    );
  }
}
