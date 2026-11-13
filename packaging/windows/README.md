# installer.nsi

The Windows installer, a plain NSIS 3 script (no template, no Tauri). `cargo xtask installer` builds the app,
gathers it in `target\stage\Pyxee`, and runs `makensis` on this file with the defines listed at the top of it
(version, the staged folder, where to write the installer, the generated uninstall list, the artwork, and, when a
certificate is configured, the path of `xtask` so the uninstaller is signed too).

- Installs for the current user into `%LOCALAPPDATA%\Pyxee`, under the same uninstall entry as earlier versions, so a
  new version installs over an old one; a running Pyxee makes it stop and ask (silent installs fail with exit code 2).
- The uninstaller deletes only the files that were installed (the list is generated from the staged folder, never a
  recursive delete) and keeps `%LOCALAPPDATA%\dev.pyxee.app` (library and settings) unless the person says otherwise.
- Needs NSIS (`makensis.exe`; set `PYXEE_MAKENSIS` if it is not in the default folder). To check the script on any
  machine: `makensis -V2 -INPUTCHARSET UTF8 -DVERSION=... ` as `xtask` does.
