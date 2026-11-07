import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../desktop/desktop_host.dart';
import 'router.dart';
import 'theme/theme.dart';

class PyxeeApp extends ConsumerWidget {
  const PyxeeApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final themes = ref.watch(appThemeProvider);
    return MaterialApp.router(
      title: 'Pyxee',
      debugShowCheckedModeBanner: false,
      theme: themes.light,
      darkTheme: themes.dark,
      themeMode: themes.mode,
      // A retargeted accent (a new cover) fades in over this; once it has, nothing animates.
      themeAnimationDuration: const Duration(milliseconds: 500),
      routerConfig: ref.watch(routerProvider),
      builder: (context, child) => DesktopHost(child: child ?? const SizedBox.shrink()),
    );
  }
}
