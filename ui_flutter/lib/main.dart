import 'package:material_ui/material_ui.dart';

import 'src/bridge_native.dart';

void main() => runApp(const PyxeeApp());

class PyxeeApp extends StatelessWidget {
  const PyxeeApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Pyxee',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF7C5CFF),
          brightness: Brightness.dark,
        ),
      ),
      home: Scaffold(body: Center(child: Text(_bridgeStatus))),
    );
  }
}

String get _bridgeStatus {
  try {
    final abi = pyxeeBridgeAbiVersion();
    final core = pyxeeBridgeCoreProbe() == 1;
    return 'bridge ok - abi $abi - core ${core ? 'ok' : 'FAILED'}';
  } on Object catch (e) {
    return 'bridge FAILED: $e';
  }
}
