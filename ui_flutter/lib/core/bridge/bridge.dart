import 'dart:async';
import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'bridge_native.dart';

/// Must equal `ABI_VERSION` in crates/bridge/src/lib.rs.
const _expectedAbi = 3;

class BridgeException implements Exception {
  const BridgeException(this.message, [this.kind]);

  final String message;

  /// `offline`, `rateLimited`, `toolMissing`, `unsupported` or `other` for source errors, else null.
  final String? kind;

  @override
  String toString() => message;
}

typedef BridgeEvent = ({String name, Object? data});

/// The Rust core as the shell sees it: JSON requests in, JSON events out. Requests that return at once use [call];
/// anything that can take a moment uses [callAsync], which runs on a Rust thread and never blocks the UI isolate.
class Bridge {
  Bridge._(this._listener, this._events);

  final NativeCallable<EventCallbackNative> _listener;
  final StreamController<BridgeEvent> _events;
  final Map<int, Completer<Object?>> _pending = {};
  int _nextId = 1;

  factory Bridge.open() {
    final abi = pyxeeBridgeAbiVersion();
    if (abi != _expectedAbi) {
      throw BridgeException(
        'the library speaks abi $abi, the app expects $_expectedAbi',
      );
    }
    final events = StreamController<BridgeEvent>.broadcast();
    late final Bridge bridge;
    final listener = NativeCallable<EventCallbackNative>.listener((
      Pointer<Utf8> text,
    ) {
      try {
        final decoded = jsonDecode(text.toDartString()) as Map<String, Object?>;
        final name = decoded['event']! as String;
        if (name == 'reply') {
          bridge._complete(decoded['data']! as Map<String, Object?>);
        } else {
          events.add((name: name, data: decoded['data']));
        }
      } finally {
        pyxeeBridgeFree(text);
      }
    });
    if (pyxeeBridgeStart(listener.nativeFunction) != 1) {
      listener.close();
      throw const BridgeException('the library did not start');
    }
    bridge = Bridge._(listener, events);
    return bridge;
  }

  /// `state`, `position`, `library`, `notice`, `sleep` and `window` events (see crates/bridge/src/channel.rs).
  Stream<BridgeEvent> get events => _events.stream;

  Object? _unwrap(Map<String, Object?> reply) {
    if (reply['ok'] != true) {
      throw BridgeException(
        reply['error'] as String? ?? 'unknown error',
        reply['kind'] as String?,
      );
    }
    return reply['data'];
  }

  void _complete(Map<String, Object?> data) {
    final completer = _pending.remove(data['id']! as int);
    if (completer == null) return;
    try {
      completer.complete(_unwrap(data['reply']! as Map<String, Object?>));
    } on BridgeException catch (e) {
      completer.completeError(e);
    }
  }

  /// For requests that return at once (state, dispatch, volume).
  Object? call(String method, [Object? params]) {
    final request = jsonEncode({
      'method': method,
      'params': ?params,
    }).toNativeUtf8();
    try {
      final reply = pyxeeBridgeCall(request);
      if (reply == nullptr) throw const BridgeException('no reply');
      try {
        return _unwrap(jsonDecode(reply.toDartString()) as Map<String, Object?>);
      } finally {
        pyxeeBridgeFree(reply);
      }
    } finally {
      malloc.free(request);
    }
  }

  /// For everything else: the reply arrives later, and the UI isolate stays free meanwhile.
  Future<Object?> callAsync(String method, [Object? params]) {
    final id = _nextId++;
    final completer = Completer<Object?>();
    _pending[id] = completer;
    final request = jsonEncode({
      'method': method,
      'params': ?params,
    }).toNativeUtf8();
    try {
      pyxeeBridgeCallAsync(request, id);
    } finally {
      malloc.free(request);
    }
    return completer.future;
  }

  /// Sends a `CommandDto`, for example `{'type': 'toggle'}`.
  void dispatch(Map<String, Object?> command) => call('dispatch', command);

  void close() {
    pyxeeBridgeStop();
    _listener.close();
    unawaited(_events.close());
  }
}
