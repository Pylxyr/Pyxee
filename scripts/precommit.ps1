<#
.SYNOPSIS
  Fast local version of the CI gates (.github/workflows/ci.yml). Run it before every push.

.DESCRIPTION
  Same commands, same order and same flags as CI, but each step prints one PASS/FAIL line and, on failure, only
  the few lines that matter. Needs the FFmpeg libraries built once (`cargo xtask ffmpeg`), like any cargo step.

    scripts\precommit.ps1              full: UI check/test/build, fmt, clippy, tests, bindings, version files
    scripts\precommit.ps1 -Quick       fmt + clippy only (about a minute once built)
    scripts\precommit.ps1 -Fix         run `cargo fmt --all` first, then everything else
    scripts\precommit.ps1 -InstallHook install a git pre-push hook that runs -Quick (skip once: git push --no-verify)
#>
[CmdletBinding()]
param([switch]$Quick, [switch]$Fix, [switch]$InstallHook)

$ErrorActionPreference = 'Continue'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($InstallHook) {
  $hook = Join-Path $Root '.git/hooks/pre-push'
  $text = "#!/bin/sh`npowershell -NoProfile -ExecutionPolicy Bypass -File scripts/precommit.ps1 -Quick || exit 1`n"
  [IO.File]::WriteAllText($hook, $text)
  Write-Host 'Installed .git/hooks/pre-push (runs fmt + clippy before each push; skip once with: git push --no-verify)'
  exit 0
}

$script:Failed = @()

# Runs one command through cmd (so stderr needs no special handling) and prints only what matters.
#   Mode 'first' = first N lines (fmt diffs), 'last' = last N lines (clippy, npm), 'grep' = the failing lines only.
function Step([string]$Name, [string]$Cmd, [string]$Mode = 'last', [int]$Lines = 20) {
  Write-Host ("> {0} ..." -f $Name) -ForegroundColor DarkGray
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $out = @(cmd /c "$Cmd 2>&1")
  $code = $LASTEXITCODE
  if ($code -eq 0) {
    Write-Host ("PASS {0} ({1}s)" -f $Name, [int]$sw.Elapsed.TotalSeconds) -ForegroundColor Green
    return
  }
  $script:Failed += $Name
  Write-Host ("FAIL {0} (exit {1})" -f $Name, $code) -ForegroundColor Red
  $show = $out
  if ($Mode -eq 'grep') {
    $show = @($out | Where-Object { $_ -match 'FAILED|panicked|^error|failures:|test result: FAILED' })
    if ($show.Count -eq 0) { $show = $out }
  }
  if ($Mode -eq 'first') { $show = @($show | Select-Object -First $Lines) } else { $show = @($show | Select-Object -Last $Lines) }
  foreach ($l in $show) { Write-Host ("    " + $l) }
}

function Ensure-Ui {
  if (-not (Test-Path 'ui/node_modules')) { Step 'npm ci' 'npm ci --prefix ui' 'last' 15 }
}

if ($Fix) {
  $null = cmd /c "cargo fmt --all 2>&1"
  Write-Host 'cargo fmt --all: done (commit the formatting changes)' -ForegroundColor Cyan
}

# CI's Rust job builds the UI first because Tauri embeds ui/dist at compile time.
if (-not $Quick) {
  Ensure-Ui
  Step 'ui: type check' 'npm run check --prefix ui' 'last' 25
  Step 'ui: tests' 'npm test --prefix ui' 'last' 25
  Step 'ui: build' 'npm run build --prefix ui' 'last' 15
} elseif (-not (Test-Path 'ui/dist/index.html')) {
  Ensure-Ui
  Step 'ui: build (needed by cargo)' 'npm run build --prefix ui' 'last' 15
}

Step 'cargo fmt --check' 'cargo fmt --all -- --check' 'first' 30
Step 'cargo clippy' 'cargo clippy --workspace --all-targets --locked -- -D warnings' 'last' 25

if (-not $Quick) {
  Step 'cargo test' 'cargo test --workspace --locked' 'grep' 25

  # The tests regenerate the TypeScript bindings; they must be committed up to date (same check as CI).
  $null = cmd /c "git add --intent-to-add -- ui/src/lib/bindings 2>&1"
  Step 'bindings committed' 'git diff --exit-code --stat -- ui/src/lib/bindings' 'first' 15

  if (Get-Command python -ErrorAction SilentlyContinue) {
    $cur = @(cmd /c "python scripts/bump_version.py --current 2>&1") | Select-Object -First 1
    Step 'version files agree' "python scripts/bump_version.py --check v$cur" 'first' 10
  } else {
    Write-Host 'SKIP version files agree (python not on PATH)' -ForegroundColor Yellow
  }
}

Write-Host ''
if ($script:Failed.Count -eq 0) {
  Write-Host 'ALL GREEN: CI should pass.' -ForegroundColor Green
  exit 0
}
Write-Host ('FAILED: ' + ($script:Failed -join ', ')) -ForegroundColor Red
if ($script:Failed -contains 'cargo fmt --check') { Write-Host 'Fix formatting with: cargo fmt --all' -ForegroundColor Yellow }
if ($script:Failed -contains 'bindings committed') { Write-Host "Regenerate with: cargo test -p pyxee-app   then commit ui/src/lib/bindings" -ForegroundColor Yellow }
exit 1
