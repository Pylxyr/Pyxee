import 'dart:ffi';

import 'package:ffi/ffi.dart';

// The asset id of these bindings is this file's own library URI, which hook/build.dart registers the library under.

typedef EventCallbackNative = Void Function(Pointer<Utf8>);

@Native<Uint32 Function()>(symbol: 'pyxee_bridge_abi_version', isLeaf: true)
external int pyxeeBridgeAbiVersion();

@Native<Uint32 Function(Pointer<NativeFunction<EventCallbackNative>>)>(
  symbol: 'pyxee_bridge_start',
)
external int pyxeeBridgeStart(Pointer<NativeFunction<EventCallbackNative>> onEvent);

@Native<Void Function()>(symbol: 'pyxee_bridge_stop')
external void pyxeeBridgeStop();

@Native<Pointer<Utf8> Function(Pointer<Utf8>)>(symbol: 'pyxee_bridge_call')
external Pointer<Utf8> pyxeeBridgeCall(Pointer<Utf8> request);

/// Returns at once; the reply comes back as a `reply` event carrying `id`.
@Native<Void Function(Pointer<Utf8>, Uint64)>(
  symbol: 'pyxee_bridge_call_async',
  isLeaf: true,
)
external void pyxeeBridgeCallAsync(Pointer<Utf8> request, int id);

@Native<Void Function(Pointer<Utf8>)>(symbol: 'pyxee_bridge_free')
external void pyxeeBridgeFree(Pointer<Utf8> text);
