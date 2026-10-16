@echo off
rem One click: installs what is missing, starts the UI dev server, then the desktop app.
setlocal
cd /d "%~dp0"

if not exist ui\node_modules (
  echo Installing UI dependencies...
  call npm install --prefix ui || exit /b 1
)
if not exist crates\app\binaries\yt-dlp-x86_64-pc-windows-msvc.exe (
  echo Downloading yt-dlp...
  cargo xtask tools || exit /b 1
)

start "Pyxee UI dev server" cmd /k "npm run dev --prefix ui"
echo Waiting for the UI dev server...
powershell -NoProfile -Command "while (-not (Test-NetConnection localhost -Port 5173 -InformationLevel Quiet -WarningAction SilentlyContinue)) { Start-Sleep -Seconds 1 }"

set "PATH=%CD%\ui\node_modules\.bin;%PATH%"
cd crates\app
call tauri dev
