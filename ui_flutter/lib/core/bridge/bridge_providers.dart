import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'bridge_native.dart';

/// A line saying whether the Rust library loaded and the core answered; shown in the deck until the real player state
/// replaces it.
final bridgeStatusProvider = Provider<String>((ref) {
  try {
    final abi = pyxeeBridgeAbiVersion();
    final core = pyxeeBridgeCoreProbe() == 1;
    return 'bridge ok - abi $abi - core ${core ? 'ok' : 'FAILED'}';
  } on Object catch (e) {
    return 'bridge FAILED: $e';
  }
});
