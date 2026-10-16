## Installation

Pyxee is for **Windows 10 (22H2) and Windows 11**.

Download **`Pyxee_x.y.z_x64-setup.exe`** below and run it. It installs yt-dlp and ffmpeg alongside the app, and downloads the WebView2 runtime if your PC does not have it yet. If you prefer an MSI, stable releases also include `Pyxee_x.y.z_x64_en-US.msi`.

**Windows SmartScreen:** the installer is not code-signed yet, so Windows may show "Windows protected your PC". Choose **More info**, then **Run anyway**. To check the file first, compare its SHA-256 with `SHA256SUMS.txt` (`Get-FileHash <file>` in PowerShell).

If something misbehaves, the log is in **Settings -> Diagnostics -> Open log folder** (`%APPDATA%\dev.pyxee.app\logs\pyxee.log`).
