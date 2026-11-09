import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../desktop/desktop_host.dart';
import '../shared/widgets.dart';
import 'shell/shell.dart';
import 'theme/px_theme.dart';

class PyxeeApp extends ConsumerWidget {
  const PyxeeApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final themes = ref.watch(appThemeProvider);
    return MaterialApp(
      title: 'Pyxee',
      debugShowCheckedModeBanner: false,
      theme: themes.light,
      darkTheme: themes.dark,
      themeMode: themes.mode,
      scrollBehavior: const PxScrollBehavior(),
      // A new cover's colours glide in over this; once they have, nothing animates.
      themeAnimationDuration: const Duration(milliseconds: 500),
      builder: (context, child) => DesktopHost(child: child ?? const SizedBox.shrink()),
      home: const Material(type: MaterialType.transparency, child: PxShell()),
    );
  }
}
