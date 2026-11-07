import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Whether Orbit, the full-screen player, is open.
class OrbitOpen extends Notifier<bool> {
  @override
  bool build() => false;

  void toggle() => state = !state;

  void close() => state = false;
}

final orbitOpenProvider = NotifierProvider<OrbitOpen, bool>(OrbitOpen.new);
