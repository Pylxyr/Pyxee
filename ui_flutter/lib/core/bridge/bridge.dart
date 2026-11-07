import 'dart:async';
import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'bridge_native.dart';

/// Must equal `ABI_VERSION` in crates/bridge/src/lib.rs.
const _expectedAbi = 2;

class BridgeException implements Exception {
  const BridgeException(this.message);

  final String message;

  @override
  String toString() => 'BridgeException: $message';
}

typedef BridgeEvent = ({String name, Object? data});

/// The Rust core as the shell sees it: JSON requests in, JSON events out.
class Bridge {
  Bridge._(this._listener, this._events);

  final NativeCallable<EventCallbackNative> _listener;
  final StreamController<BridgeEvent> _events;

  factory Bridge.open() {
    final abi = pyxeeBridgeAbiVersion();
    if (abi != _expectedAbi) {
      throw BridgeException(
        'the library speaks abi $abi, the app expects $_expectedAbi',
      );
    }
    final events = StreamController<BridgeEvent>.broadcast();
    final listener = NativeCallable<EventCallbackNative>.listener((
      Pointer<Utf8> text,
    ) {
      try {
        final decoded = jsonDecode(text.toDartString()) as Map<String, Object?>;
        events.add((name: decoded['event']! as String, data: decoded['data']));
      } finally {
        pyxeeBridgeFree(text);
      }
    });
    if (pyxeeBridgeStart(listener.nativeFunction) != 1) {
      listener.close();
      throw const BridgeException('the library did not start');
    }
    return Bridge._(listener, events);
  }

  /// `state` (the whole player state) and `position` (only the playback position) events, in the shapes of
  /// `PlayerStateDto` and `PositionDto`.
  Stream<BridgeEvent> get events => _events.stream;

  Object? call(String method, [Object? params]) {
    final request = jsonEncode({
      'method': method,
      'params': ?params,
    }).toNativeUtf8();
    try {
      final reply = pyxeeBridgeCall(request);
      if (reply == nullptr) throw const BridgeException('no reply');
      try {
        final decoded = jsonDecode(reply.toDartString()) as Map<String, Object?>;
        if (decoded['ok'] != true) {
          throw BridgeException(decoded['error'] as String? ?? 'unknown error');
        }
        return decoded['data'];
      } finally {
        pyxeeBridgeFree(reply);
      }
    } finally {
      malloc.free(request);
    }
  }

  /// Sends a `CommandDto`, for example `{'type': 'toggle'}`.
  void dispatch(Map<String, Object?> command) => call('dispatch', command);

  void close() {
    pyxeeBridgeStop();
    _listener.close();
    unawaited(_events.close());
  }
}
