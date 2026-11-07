import 'dart:ffi';

// The asset id of these bindings is this file's own library URI, which hook/build.dart registers the library under.

@Native<Uint32 Function()>(symbol: 'pyxee_bridge_abi_version', isLeaf: true)
external int pyxeeBridgeAbiVersion();

@Native<Uint32 Function()>(symbol: 'pyxee_bridge_core_probe', isLeaf: true)
external int pyxeeBridgeCoreProbe();
