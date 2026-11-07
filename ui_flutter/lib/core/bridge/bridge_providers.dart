import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'bridge.dart';

final bridgeProvider = Provider<Bridge>((ref) {
  final bridge = Bridge.open();
  ref.onDispose(bridge.close);
  return bridge;
});

/// The few fields of the player state that the capsule shows for now. The full model comes with the state wiring.
class PlayerHead {
  const PlayerHead({
    required this.seq,
    required this.transport,
    required this.queueLength,
    required this.positionMs,
  });

  factory PlayerHead.fromState(Map<String, Object?> json) => PlayerHead(
    seq: json['seq']! as int,
    transport: (json['transport']! as Map<String, Object?>)['state']! as String,
    queueLength: (json['queue']! as List<Object?>).length,
    positionMs: json['positionMs']! as int,
  );

  final int seq;
  final String transport;
  final int queueLength;
  final int positionMs;

  PlayerHead withPosition(Map<String, Object?> json) => PlayerHead(
    seq: json['seq']! as int,
    transport: transport,
    queueLength: queueLength,
    positionMs: json['positionMs']! as int,
  );
}

class PlayerHeadNotifier extends AsyncNotifier<PlayerHead> {
  @override
  Future<PlayerHead> build() async {
    final bridge = ref.watch(bridgeProvider);
    // Listen before asking, so that nothing that happens in between is missed; the `seq` sorts out any overlap.
    final subscription = bridge.events.listen(_onEvent);
    ref.onDispose(subscription.cancel);
    return PlayerHead.fromState(
      bridge.call('getState')! as Map<String, Object?>,
    );
  }

  void _onEvent(BridgeEvent event) {
    final current = state.value;
    final data = event.data! as Map<String, Object?>;
    switch (event.name) {
      case 'state':
        final next = PlayerHead.fromState(data);
        if (current == null || next.seq > current.seq) {
          state = AsyncData(next);
        }
      case 'position':
        if (current != null && (data['seq']! as int) > current.seq) {
          state = AsyncData(current.withPosition(data));
        }
    }
  }
}

final playerHeadProvider =
    AsyncNotifierProvider<PlayerHeadNotifier, PlayerHead>(
      PlayerHeadNotifier.new,
    );
