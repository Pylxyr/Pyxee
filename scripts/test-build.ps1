<#
.SYNOPSIS
  One-command test build of Pyxee on Windows.

.DESCRIPTION
  1. Checks for yt-dlp and downloads it (cargo xtask tools) if it is missing.
  2. Checks for the minimal static FFmpeg libraries and builds them (cargo xtask ffmpeg) if they are missing.
  3. Builds the Rust bridge (cargo build -p pyxee-bridge --release), the library the Flutter app loads.
  4. Gets the Flutter packages and launches the app (flutter run -d windows). Close the app window to end the run.
  Stops at the first failure and prints a summary.

.PARAMETER Installer
  Also build the installer afterwards (cargo xtask installer; slow; needs NSIS).

.PARAMETER NoRun
  Build and check only; do not launch the app at the end.

.PARAMETER Release
  Run the app in Flutter's release mode instead of debug (the bridge is always built in release mode).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\test-build.ps1
#>
param(
  [switch]$Installer,
  [switch]$NoRun,
  [switch]$Release
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
$root = (Get-Location).Path
$results = New-Object System.Collections.ArrayList

function Add-Result($status, $name, $detail) {
  [void]$results.Add([pscustomobject]@{ Status = $status; Step = $name; Detail = $detail })
  $color = switch ($status) { 'PASS' { 'Green' } 'FAIL' { 'Red' } default { 'Yellow' } }
  Write-Host ("[{0}] {1}: {2}" -f $status, $name, $detail) -ForegroundColor $color
}

function Finish {
  Write-Host ''
  Write-Host '---- Summary ----'
  $results | Format-Table -AutoSize -Wrap | Out-String | Write-Host
  if (@($results | Where-Object { $_.Status -eq 'FAIL' }).Count -gt 0) { exit 1 }
  exit 0
}

# Runs a command; records PASS or FAIL and stops the script on failure.
function Step($name, [scriptblock]$cmd) {
  Write-Host ''
  Write-Host ("==> {0}" -f $name) -ForegroundColor Cyan
  $sw = [Diagnostics.Stopwatch]::StartNew()
  & $cmd
  $code = $LASTEXITCODE
  $secs = [int]$sw.Elapsed.TotalSeconds
  if ($code -ne 0) {
    Add-Result 'FAIL' $name ("exit code $code after ${secs}s")
    Finish
  }
  Add-Result 'PASS' $name ("done in ${secs}s")
}

# ---- Prerequisites on PATH
foreach ($tool in @('cargo', 'rustc', 'flutter')) {
  if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
    Add-Result 'FAIL' "prereq-$tool" "$tool was not found on PATH"
    Finish
  }
}
Add-Result 'PASS' 'prerequisites' 'cargo, rustc and flutter found'

# ---- yt-dlp
$ytdlpName = 'yt-dlp-x86_64-pc-windows-msvc.exe'
$ytdlpPath = Join-Path $root "tools\$ytdlpName"
function Find-Ytdlp {
  if ($env:PYXEE_YTDLP -and (Test-Path -LiteralPath $env:PYXEE_YTDLP -PathType Leaf)) { return $env:PYXEE_YTDLP }
  if (Test-Path -LiteralPath $ytdlpPath -PathType Leaf) { return $ytdlpPath }
  $g = Get-Command yt-dlp -ErrorAction SilentlyContinue
  if ($g) { return $g.Source }
  return $null
}
$found = Find-Ytdlp
if ($found) {
  Add-Result 'PASS' 'yt-dlp' "found: $found"
} else {
  Add-Result 'INFO' 'yt-dlp' 'not found, downloading it'
  Step 'cargo xtask tools' { cargo xtask tools }
  $found = Find-Ytdlp
  if (-not $found) { Add-Result 'FAIL' 'yt-dlp' 'still not found after cargo xtask tools'; Finish }
  Add-Result 'PASS' 'yt-dlp' "installed: $found"
}
# Run it to completion (no early pipeline stop, which leaves a wrong exit code) and keep stderr warnings out of the way.
$ErrorActionPreference = 'Continue'
$ytOut = @(& $found --version 2>&1 | ForEach-Object { "$_" })
$ytCode = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
$ver = ($ytOut | Where-Object { $_ -match '^\d{4}\.\d{2}\.\d{2}' } | Select-Object -First 1)
if ($ytCode -eq 0 -and $ver) { Add-Result 'PASS' 'yt-dlp-runs' "version $ver" }
else { Add-Result 'FAIL' 'yt-dlp-runs' ("exit code $ytCode; output: " + (($ytOut | Select-Object -First 3) -join ' | ')); Finish }

# ---- FFmpeg (static libraries the audio crate links)
$hostLine = (rustc -vV | Select-String '^host:').Line
$triple = ($hostLine -replace '^host:\s*', '').Trim()
$prefix = if ($env:PYXEE_FFMPEG_DIR) { $env:PYXEE_FFMPEG_DIR } else { Join-Path $root "target\ffmpeg\$triple" }
function Test-Ffmpeg {
  if (-not (Test-Path -LiteralPath (Join-Path $prefix 'include\libavformat\avformat.h'))) { return 'avformat.h is missing' }
  foreach ($l in @('avformat', 'avcodec', 'swresample', 'avutil')) {
    $hit = Get-ChildItem -LiteralPath (Join-Path $prefix 'lib') -Filter "*$l*" -ErrorAction SilentlyContinue
    if (-not $hit) { return "library $l is missing" }
  }
  return $null
}
$missing = Test-Ffmpeg
if (-not $missing) {
  Add-Result 'PASS' 'ffmpeg' "found: $prefix"
} else {
  Add-Result 'INFO' 'ffmpeg' "not ready ($missing at $prefix), building it (this takes a while the first time)"
  Step 'cargo xtask ffmpeg' { cargo xtask ffmpeg }
  $missing = Test-Ffmpeg
  if ($missing) { Add-Result 'FAIL' 'ffmpeg' "still not usable after the build: $missing"; Finish }
  Add-Result 'PASS' 'ffmpeg' "built: $prefix"
}

# ---- Rust bridge (the library the Flutter app loads). The app's native-assets hook (ui_flutter/hook/build.dart) copies
# it from target\release, so it is always built in release mode; rebuild it after every Rust change.
Step 'cargo build -p pyxee-bridge --release' { cargo build -p pyxee-bridge --release }
$dll = Join-Path $root 'target\release\pyxee_bridge.dll'
if (Test-Path -LiteralPath $dll) {
  $mb = [math]::Round((Get-Item -LiteralPath $dll).Length / 1MB, 1)
  Add-Result 'PASS' 'bridge-library' "$dll ($mb MB)"
} else {
  Add-Result 'FAIL' 'bridge-library' "build finished but $dll does not exist"
  Finish
}

# ---- Flutter packages
Push-Location (Join-Path $root 'ui_flutter')
try {
  Step 'flutter pub get' { flutter pub get }
} finally { Pop-Location }

# ---- Installer (optional)
if ($Installer) {
  Step 'cargo xtask installer' { cargo xtask installer }
  $out = Join-Path $root 'target\installer'
  $files = @(Get-ChildItem -LiteralPath $out -Filter *-setup.exe -ErrorAction SilentlyContinue)
  if ($files.Count -gt 0) { foreach ($f in $files) { Add-Result 'PASS' 'installer' $f.FullName } }
  else { Add-Result 'FAIL' 'installer' "no installer found under $out" }
}

# ---- Launch the app
if (-not $NoRun) {
  Write-Host ''
  Write-Host '==> Launching Pyxee (close the app window to finish)' -ForegroundColor Cyan
  Push-Location (Join-Path $root 'ui_flutter')
  try {
    if ($Release) { & flutter run -d windows --release } else { & flutter run -d windows }
    $appCode = $LASTEXITCODE
  } finally { Pop-Location }
  if ($appCode -eq 0) { Add-Result 'PASS' 'app-run' 'the app ran and was closed normally' }
  else { Add-Result 'FAIL' 'app-run' "flutter run exited with code $appCode" }
}

Finish
