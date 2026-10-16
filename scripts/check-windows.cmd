@echo off
rem Runs the Pyxee Windows check. Options are passed on, e.g.:  check-windows.cmd -Manual -CookieBrowser firefox
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0check-windows.ps1" %*
exit /b %errorlevel%
