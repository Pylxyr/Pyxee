<#
.SYNOPSIS
  Pyxee Windows check: builds, runs every test, proves ffmpeg is not used, and runs live YouTube
  checks of the real search/resolve/decode/seek/gapless path. Writes one pasteable report.

.DESCRIPTION
  Run it from anywhere:   scripts\check-windows.cmd        (or:  powershell -File scripts\check-windows.ps1)
  Needs: Rust (rustup, with rustfmt + clippy), Node.js, git, internet. No ffmpeg, and none is wanted.
  Output: pyxee-check\report-<time>.txt  (paste its contents back). Full logs: pyxee-check\logs\.
  The report never contains stream URLs (only hosts), cookie paths/values, or your user name.

.PARAMETER Ytdlp          Path to yt-dlp.exe (default: PYXEE_YTDLP, crates\app\binaries, then PATH).
.PARAMETER Query1/2       Search queries used for the two live tracks (default: two CC-licensed songs).
.PARAMETER CookieFile     Netscape cookies.txt to use (sets PYXEE_COOKIES for this run only).
.PARAMETER CookieBrowser  e.g. firefox (sets PYXEE_COOKIES_FROM_BROWSER for this run only).
.PARAMETER SkipTests      Skip fmt/clippy/cargo test (only do static checks + live checks).
.PARAMETER SkipLive       Skip everything that talks to YouTube.
.PARAMETER SkipTools      Do not run `cargo xtask tools` (the yt-dlp download flow).
.PARAMETER Ui             Also run the UI checks from CI (npm ci / check / test / build).
.PARAMETER Installer      Also build the installer (`cargo xtask installer`, slow) and inspect it.
.PARAMETER UpdateYtdlp    Run `yt-dlp -U` first (an old yt-dlp is the most common cause of YouTube failures).
.PARAMETER Manual         After the automatic part, ask you to confirm things in the real app (y/n).
.PARAMETER AnalyzeLog     Summarise the app's own log file (run after using the app).
.PARAMETER FlakeRuns      Repeats of the known timing-sensitive pause test (default 10).
#>
[CmdletBinding()]
param(
  [string]$Ytdlp,
  [string]$Query1 = 'Kevin MacLeod Sneaky Snitch',
  [string]$Query2 = 'Kevin MacLeod Monkeys Spinning Monkeys',
  [string]$CookieFile,
  [string]$CookieBrowser,
  [switch]$SkipTests,
  [switch]$SkipLive,
  [switch]$SkipTools,
  [switch]$Ui,
  [switch]$Installer,
  [switch]$UpdateYtdlp,
  [switch]$Manual,
  [switch]$AnalyzeLog,
  [int]$FlakeRuns = 10
)

$ErrorActionPreference = 'Continue'
$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root 'Cargo.toml'))) { Write-Host "Run this from the Pyxee repo (scripts\ folder not under the repo root)."; exit 2 }
Set-Location $Root

$Stamp  = Get-Date -Format 'yyyyMMdd-HHmmss'
$OutDir = Join-Path $Root 'pyxee-check'
$LogDir = Join-Path $OutDir "logs-$Stamp"
$Report = Join-Path $OutDir "report-$Stamp.txt"
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

$script:Counts = @{ PASS = 0; FAIL = 0; WARN = 0; INFO = 0 }
$script:LiveBlocked = $false
$script:Step = 0

# ---------------------------------------------------------------- output helpers

function Scrub([string]$s) {
  if ($null -eq $s) { return '' }
  $s = [regex]::Replace($s, 'https?://([^/\s"''<>)]+)[^\s"''<>)]*', '<url host=$1>')
  if ($env:USERPROFILE) { $s = $s.Replace($env:USERPROFILE, '%USERPROFILE%') }
  if ($env:USERNAME -and $env:USERNAME.Length -gt 2) { $s = $s.Replace($env:USERNAME, '<user>') }
  return $s
}

function Write-Report([string]$text, [string]$color = '') {
  $t = Scrub $text
  Add-Content -Path $Report -Value $t -Encoding UTF8
  if ($color) { Write-Host $t -ForegroundColor $color } else { Write-Host $t }
}

function Section([string]$title) {
  $script:Step++
  Write-Report ''
  Write-Report ("=== {0}. {1} ===" -f $script:Step, $title) 'Cyan'
}

function Add-Result([string]$Status, [string]$Name, [string]$Detail) {
  $script:Counts[$Status]++
  $color = switch ($Status) { 'PASS' { 'Green' } 'FAIL' { 'Red' } 'WARN' { 'Yellow' } default { 'Gray' } }
  Write-Report ("[{0}] {1} - {2}" -f $Status, $Name, $Detail) $color
}

function Show-LogTail([string]$LogFile, [int]$Lines = 40) {
  if (Test-Path $LogFile) {
    Write-Report "--- last $Lines lines of $(Split-Path -Leaf $LogFile) ---"
    foreach ($l in (Get-Content $LogFile -Tail $Lines)) { Write-Report ("    " + $l) }
  }
}

# Runs a command line through a temp .cmd so stderr/stdout go to a log without PowerShell wrapping them.
function Invoke-Logged([string]$Name, [string]$CommandLine, [string]$WorkDir = $Root) {
  $log = Join-Path $LogDir ("{0}.log" -f $Name)
  $bat = Join-Path $LogDir ("{0}.cmd" -f $Name)
  # `call` matters: npm is a .cmd, and a batch file that runs another one without it never gets control back.
  $content = "@echo off`r`ncd /d `"$WorkDir`"`r`ncall $CommandLine > `"$log`" 2>&1`r`nexit /b %errorlevel%`r`n"
  Set-Content -Path $bat -Value $content -Encoding OEM
  $sw = [Diagnostics.Stopwatch]::StartNew()
  & $bat
  $code = $LASTEXITCODE
  $sw.Stop()
  return [pscustomobject]@{ Code = $code; Log = $log; Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1) }
}

function Have([string]$cmd) { return [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }

function Get-FirstLine([string]$cmdline) {
  $r = Invoke-Logged ("probe-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) $cmdline
  if (Test-Path $r.Log) { return ((Get-Content $r.Log -TotalCount 1) -join '') }
  return ''
}

# ---------------------------------------------------------------- cookies (kind only is ever reported)

if ($CookieFile)    { $env:PYXEE_COOKIES = (Resolve-Path $CookieFile).Path }
if ($CookieBrowser) { $env:PYXEE_COOKIES_FROM_BROWSER = $CookieBrowser }
$cookieKind = 'none'
if ($env:PYXEE_COOKIES) { $cookieKind = 'cookie file' } elseif ($env:PYXEE_COOKIES_FROM_BROWSER) { $cookieKind = 'browser cookies' }

# ---------------------------------------------------------------- ffmpeg tripwire
# A fake ffmpeg.exe/ffprobe.exe first on PATH. If anything starts either, it is logged and fails.

$TripDir = Join-Path $OutDir 'tripwire'
$TripLog = Join-Path $TripDir 'calls.log'
function Initialize-Tripwire {
  New-Item -ItemType Directory -Force -Path $TripDir | Out-Null
  if (Test-Path $TripLog) { Remove-Item $TripLog -Force }
  $src = Join-Path $TripDir 'tripwire.rs'
  @'
use std::io::Write;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if let Ok(p) = std::env::var("PYXEE_TRIPWIRE_LOG") {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{}", a.join(" "));
        }
    }
    eprintln!("ffmpeg tripwire: ffmpeg is not available here");
    std::process::exit(127);
}
'@ | Set-Content -Path $src -Encoding ASCII
  $exe = Join-Path $TripDir 'ffmpeg.exe'
  $r = Invoke-Logged 'tripwire-build' "rustc -O `"$src`" -o `"$exe`""
  if ($r.Code -ne 0 -or -not (Test-Path $exe)) { return $false }
  Copy-Item $exe (Join-Path $TripDir 'ffprobe.exe') -Force
  $env:PYXEE_TRIPWIRE_LOG = $TripLog
  $env:PATH = "$TripDir;$env:PATH"
  return $true
}
function Get-TripCount { if (Test-Path $TripLog) { return @(Get-Content $TripLog).Count } else { return 0 } }
function Add-TripResult([string]$phase, [int]$before) {
  $now = Get-TripCount
  if ($now -eq $before) {
    Add-Result 'PASS' "no-ffmpeg-in-$phase" 'nothing started ffmpeg or ffprobe'
  } else {
    $calls = @(Get-Content $TripLog | Select-Object -Skip $before -First 5) -join ' || '
    Add-Result 'FAIL' "no-ffmpeg-in-$phase" "$($now - $before) attempt(s) to start ffmpeg/ffprobe: $calls  (if these look like yt-dlp's own probes such as -version or -bsfs, they come from yt-dlp itself, not from Pyxee)"
  }
}

# ---------------------------------------------------------------- header

Set-Content -Path $Report -Value '' -Encoding UTF8
Write-Report '############ Pyxee Windows check ############' 'Cyan'
Write-Report ("started   : {0}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz'))
Write-Report ("windows   : {0}   powershell {1}" -f [Environment]::OSVersion.VersionString, $PSVersionTable.PSVersion)
Write-Report ("git       : {0}  ({1} changed file(s) in working tree)" -f (Get-FirstLine 'git rev-parse --short HEAD'), @(& git status --porcelain 2>$null | Where-Object { $_ -notmatch 'pyxee-check' }).Count)
Write-Report ("rustc     : {0}" -f (Get-FirstLine 'rustc --version'))
Write-Report ("cargo     : {0}" -f (Get-FirstLine 'cargo --version'))
Write-Report ("node      : {0}" -f (Get-FirstLine 'node --version'))
Write-Report ("cookies   : {0}" -f $cookieKind)
$envset = @('PYXEE_YTDLP', 'PYXEE_COOKIES', 'PYXEE_COOKIES_FROM_BROWSER', 'PYXEE_LOG', 'CARGO_TARGET_DIR') | ForEach-Object { $v = [Environment]::GetEnvironmentVariable($_); if ($v) { "$_=set" } }
Write-Report ("env       : {0}" -f $(if ($envset) { $envset -join ', ' } else { '(none of the PYXEE_* / CARGO_TARGET_DIR variables are set)' }))
Write-Report ("options   : SkipTests=$SkipTests SkipLive=$SkipLive SkipTools=$SkipTools Ui=$Ui Installer=$Installer Manual=$Manual AnalyzeLog=$AnalyzeLog")

foreach ($tool in @('cargo', 'rustc', 'git')) {
  if (-not (Have $tool)) { Add-Result 'FAIL' "tool-$tool" "$tool is not on PATH; install it and re-run"; Write-Report 'Cannot continue.'; exit 2 }
}
$tripOk = Initialize-Tripwire
if ($tripOk) { Add-Result 'INFO' 'tripwire' 'fake ffmpeg.exe/ffprobe.exe placed first on PATH for every step below' }
else { Add-Result 'WARN' 'tripwire' 'could not build the ffmpeg tripwire; the no-ffmpeg runtime proof is skipped'; Show-LogTail (Join-Path $LogDir 'tripwire-build.log') 10 }
$realFfmpeg = @(Get-Command ffmpeg.exe -All -ErrorAction SilentlyContinue | Where-Object { $_.Source -notlike "$TripDir*" })
if ($realFfmpeg.Count -gt 0) { Add-Result 'INFO' 'system-ffmpeg' 'a real ffmpeg is installed on this PC; it is shadowed by the tripwire during this run' }

# ---------------------------------------------------------------- 1. static: ffmpeg is gone from the repo

Section 'ffmpeg is gone from code, build and packaging'
$bin = Join-Path $Root 'crates\app\binaries'
$ff = @(Get-ChildItem $bin -Filter 'ffmpeg*' -ErrorAction SilentlyContinue)
if ($ff.Count -eq 0) { Add-Result 'PASS' 'no-ffmpeg-sidecar-file' 'crates\app\binaries has no ffmpeg*' } else { Add-Result 'FAIL' 'no-ffmpeg-sidecar-file' ($ff.Name -join ', ') }
try {
  $conf = Get-Content (Join-Path $Root 'crates\app\tauri.bundle.conf.json') -Raw | ConvertFrom-Json
  $bins = @($conf.bundle.externalBin)
  if (($bins -join ' ') -match 'ffmpeg') { Add-Result 'FAIL' 'bundle-externalBin' ($bins -join ', ') } else { Add-Result 'PASS' 'bundle-externalBin' ("sidecars: " + ($bins -join ', ')) }
} catch { Add-Result 'WARN' 'bundle-externalBin' "could not parse tauri.bundle.conf.json: $($_.Exception.Message)" }
$code = @(& git grep -n -i -E 'FfmpegPipe|ffmpeg\.exe|PYXEE_FFMPEG|Command::new\(.*ffmpeg|externalBin.*ffmpeg|AudioError::Ffmpeg' -- '*.rs' '*.toml' '*.json' '*.cmd' '*.yml' ':!scripts' 2>$null)
if ($code.Count -eq 0) { Add-Result 'PASS' 'no-ffmpeg-in-code' 'no ffmpeg process/decoder/config references in .rs .toml .json .cmd .yml' }
else { Add-Result 'FAIL' 'no-ffmpeg-in-code' "$($code.Count) reference(s)"; $code | Select-Object -First 10 | ForEach-Object { Write-Report "    $_" } }
$any = @(& git grep -n -i ffmpeg -- . ':!Cargo.lock' ':!scripts' 2>$null)
Add-Result 'INFO' 'ffmpeg-mentions-left' "$($any.Count) line(s) mention the word (expected: comments/docs saying it was removed, plus the fixture-provenance note)"
$any | Select-Object -First 25 | ForEach-Object { Write-Report ("    " + $_.Substring(0, [math]::Min($_.Length, 170))) }

# ---------------------------------------------------------------- 2. yt-dlp (tools flow)

Section 'yt-dlp: download flow and location'
if (-not $SkipTools) {
  $before = Get-TripCount
  $r = Invoke-Logged 'xtask-tools' 'cargo xtask tools'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-xtask-tools' "finished in $($r.Seconds)s" } else { Add-Result 'FAIL' 'cargo-xtask-tools' "exit code $($r.Code)"; Show-LogTail $r.Log 25 }
  $ff2 = @(Get-ChildItem $bin -Filter 'ffmpeg*' -ErrorAction SilentlyContinue)
  if ($ff2.Count -eq 0) { Add-Result 'PASS' 'tools-flow-downloads-only-ytdlp' ("files: " + ((Get-ChildItem $bin -File | Where-Object { $_.Name -notmatch 'README' } | ForEach-Object { $_.Name }) -join ', ')) } else { Add-Result 'FAIL' 'tools-flow-downloads-only-ytdlp' ($ff2.Name -join ', ') }
  Add-TripResult 'tools-flow' $before
} else { Add-Result 'INFO' 'cargo-xtask-tools' 'skipped (-SkipTools)' }

function Find-Ytdlp {
  if ($Ytdlp -and (Test-Path $Ytdlp)) { return (Resolve-Path $Ytdlp).Path }
  if ($env:PYXEE_YTDLP -and (Test-Path $env:PYXEE_YTDLP)) { return (Resolve-Path $env:PYXEE_YTDLP).Path }
  $cand = @(Get-ChildItem $bin -Filter 'yt-dlp*.exe' -ErrorAction SilentlyContinue | Sort-Object Name)
  if ($cand.Count -gt 0) { return $cand[0].FullName }
  $g = Get-Command yt-dlp -ErrorAction SilentlyContinue
  if ($g) { return $g.Source }
  return $null
}
$YtdlpPath = Find-Ytdlp
if (-not $YtdlpPath) {
  Add-Result 'FAIL' 'ytdlp-found' 'no yt-dlp.exe found (run without -SkipTools, or pass -Ytdlp <path>)'
  $SkipLive = $true
} else {
  Add-Result 'PASS' 'ytdlp-found' (Split-Path -Leaf $YtdlpPath)
  if ($UpdateYtdlp) { $r = Invoke-Logged 'ytdlp-update' "`"$YtdlpPath`" -U"; Add-Result 'INFO' 'ytdlp-update' ((Get-Content $r.Log -ErrorAction SilentlyContinue | Select-Object -Last 2) -join ' | ') }
  Add-Result 'INFO' 'ytdlp-version' (Get-FirstLine "`"$YtdlpPath`" --version")
}

# ---------------------------------------------------------------- 3. build + tests

function Test-UiBuilt { return (Test-Path (Join-Path $Root 'ui\dist\index.html')) }
function Invoke-UiSteps([bool]$full) {
  if (-not (Have 'npm')) { Add-Result 'FAIL' 'npm' 'npm is not on PATH'; return }
  $r = Invoke-Logged 'ui-npm-ci' 'npm ci --prefix ui'
  if ($r.Code -ne 0) { Add-Result 'FAIL' 'ui-npm-ci' "exit $($r.Code)"; Show-LogTail $r.Log 20; return } else { Add-Result 'PASS' 'ui-npm-ci' "$($r.Seconds)s" }
  if ($full) {
    foreach ($s in @(@('ui-check', 'npm run check --prefix ui'), @('ui-test', 'npm test --prefix ui'))) {
      $r = Invoke-Logged $s[0] $s[1]
      if ($r.Code -eq 0) { Add-Result 'PASS' $s[0] "$($r.Seconds)s" } else { Add-Result 'FAIL' $s[0] "exit $($r.Code)"; Show-LogTail $r.Log 25 }
    }
  }
  $r = Invoke-Logged 'ui-build' 'npm run build --prefix ui'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'ui-build' "$($r.Seconds)s" } else { Add-Result 'FAIL' 'ui-build' "exit $($r.Code)"; Show-LogTail $r.Log 25 }
}

if (-not $SkipTests) {
  Section 'Rust build and tests (same steps as CI)'
  $before = Get-TripCount
  if ($Ui) { Invoke-UiSteps $true } elseif (-not (Test-UiBuilt)) {
    Add-Result 'INFO' 'ui-dist' 'ui\dist is missing; building the UI first (Tauri embeds it at compile time)'
    Invoke-UiSteps $false
  }

  $r = Invoke-Logged 'cargo-fmt' 'cargo fmt --all -- --check'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-fmt' 'formatting is clean' } else { Add-Result 'FAIL' 'cargo-fmt' "exit $($r.Code) (run: cargo fmt --all)"; Show-LogTail $r.Log 20 }

  $r = Invoke-Logged 'cargo-clippy' 'cargo clippy --workspace --all-targets -- -D warnings'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-clippy' "no warnings ($($r.Seconds)s)" }
  elseif ((Get-Content $r.Log -Raw) -match "no such command: .clippy") { Add-Result 'WARN' 'cargo-clippy' 'clippy is not installed (rustup component add clippy)' }
  else { Add-Result 'FAIL' 'cargo-clippy' "exit $($r.Code)"; Show-LogTail $r.Log 50 }

  $r = Invoke-Logged 'cargo-test' 'cargo test --workspace --no-fail-fast'
  $txt = Get-Content $r.Log -Raw
  $p = 0; $f = 0; $ig = 0
  foreach ($m in [regex]::Matches($txt, 'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored')) { $p += [int]$m.Groups[1].Value; $f += [int]$m.Groups[2].Value; $ig += [int]$m.Groups[3].Value }
  $failedNames = @([regex]::Matches($txt, '(?m)^test (\S+) \.\.\. FAILED') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
  if ($r.Code -eq 0 -and $f -eq 0) { Add-Result 'PASS' 'cargo-test' "$p passed, $f failed, $ig ignored in $($r.Seconds)s" }
  else {
    Add-Result 'FAIL' 'cargo-test' "$p passed, $f failed (exit $($r.Code)); failed: $($failedNames -join ', ')"
    if ($failedNames.Count -eq 0) { Show-LogTail $r.Log 40 }
    foreach ($n in ($failedNames | Select-Object -First 5)) {
      $re = Invoke-Logged ("rerun-" + ($n -replace '[^A-Za-z0-9]', '_')) "cargo test --workspace -- $n --exact"
      if ($re.Code -eq 0) { Add-Result 'WARN' "rerun-$n" 'passed when re-run alone (timing flake?)' } else { Add-Result 'FAIL' "rerun-$n" 'failed again when re-run alone'; Show-LogTail $re.Log 25 }
    }
  }
  Add-Result 'INFO' 'windows-note' 'the fake-yt-dlp tests in pyxee-sources are unix-only; the live phase "fake429" covers the same cooldown logic on this PC'

  # Known timing-sensitive test (also flaky on the original code: 6 of 40 failures there).
  if ($FlakeRuns -gt 0) {
    $bad = 0
    for ($i = 1; $i -le $FlakeRuns; $i++) {
      $re = Invoke-Logged "flake-$i" 'cargo test -p pyxee-app pause_freezes_the_position_and_resume_continues'
      if ($re.Code -ne 0) { $bad++ }
    }
    Add-Result 'INFO' 'known-flake-pause-test' "failed $bad of $FlakeRuns runs (original code before this work: about 6 of 40; not related to ffmpeg/yt-dlp)"
  }

  Add-TripResult 'build-and-tests' $before

  $dirty = @(& git status --porcelain 2>$null | Where-Object { $_ -notmatch 'pyxee-check' })
  $bind = @($dirty | Where-Object { $_ -match 'ui/src/lib/bindings' })
  if ($bind.Count -eq 0) { Add-Result 'PASS' 'bindings-up-to-date' 'ts-rs bindings unchanged by the tests (CI: git diff --exit-code)' } else { Add-Result 'FAIL' 'bindings-up-to-date' ($bind -join '; ') }
  Add-Result 'INFO' 'git-status-after-tests' "$($dirty.Count) changed/untracked path(s) (the first lines are the files this work changed or added)"
}

# ---------------------------------------------------------------- 4. optional installer

if ($Installer) {
  Section 'Installer build (slow)'
  $before = Get-TripCount
  $r = Invoke-Logged 'xtask-installer' 'cargo xtask installer'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-xtask-installer' "built in $($r.Seconds)s" } else { Add-Result 'FAIL' 'cargo-xtask-installer' "exit $($r.Code)"; Show-LogTail $r.Log 30 }
  $rel = Join-Path $Root 'target\release'
  $leak = @(Get-ChildItem $rel -Recurse -Include 'ffmpeg*' -ErrorAction SilentlyContinue | Where-Object { $_.FullName -notlike "*\deps\*" -and $_.FullName -notlike "*\build\*" })
  if ($leak.Count -eq 0) { Add-Result 'PASS' 'installer-has-no-ffmpeg' 'no ffmpeg* file under target\release' } else { Add-Result 'FAIL' 'installer-has-no-ffmpeg' (($leak | ForEach-Object { $_.Name }) -join ', ') }
  $setup = @(Get-ChildItem (Join-Path $rel 'bundle') -Recurse -Include '*.exe', '*.msi' -ErrorAction SilentlyContinue)
  foreach ($s in $setup) { Add-Result 'INFO' 'installer-file' ("{0}  {1:N1} MB" -f $s.Name, ($s.Length / 1MB)) }
  Add-TripResult 'installer-build' $before
}

# ---------------------------------------------------------------- 5. live checks

function Read-CheckLines([string]$file, [string]$prefix = '') {
  if (-not (Test-Path $file)) { return }
  $logLines = 0
  foreach ($line in (Get-Content $file)) {
    if ($line -match '^CHECK (PASS|FAIL|WARN|INFO) (\S+) :: (.*)$') {
      $st = $Matches[1]; $nm = $prefix + $Matches[2]; $dt = $Matches[3]
      Add-Result $st $nm $dt
      if ($dt -match 'rate.?limit|limiting requests|Too Many Requests|HTTP Error 429|sign in to confirm') { if ($st -eq 'FAIL' -or $st -eq 'WARN') { $script:LiveBlocked = $true } }
    } elseif ($line -match '^LOG (WARN|ERROR) ' -or ($line -match '^LOG INFO ' -and $logLines -lt 25)) {
      if ($line -match '^LOG INFO ') { $logLines++ }
      Write-Report ("    " + $line)
    }
  }
}

function Get-YtdlpRoots {
  $procs = @(Get-CimInstance Win32_Process -Filter "Name LIKE 'yt-dlp%'" -ErrorAction SilentlyContinue)
  if ($procs.Count -eq 0) { return 0 }
  $ids = @{}
  foreach ($p in $procs) { $ids[[int]$p.ProcessId] = $true }
  $roots = 0
  foreach ($p in $procs) { if (-not $ids.ContainsKey([int]$p.ParentProcessId)) { $roots++ } }
  return $roots
}

if (-not $SkipLive) {
  Section 'Live YouTube checks (real search / resolve / decode / seek / gapless)'
  Write-Report 'This sends roughly 25 yt-dlp requests, paced by the app itself. If YouTube starts rate limiting, the script stops the live part and says so.'
  $before = Get-TripCount

  Write-Report 'building the live-check tool (release build; the first time takes a few minutes)...'
  $r = Invoke-Logged 'build-livecheck' 'cargo build --release -p pyxee-spike --bin livecheck'
  $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
  $Live = Join-Path $targetDir 'release\livecheck.exe'
  if ($r.Code -ne 0 -or -not (Test-Path $Live)) {
    Add-Result 'FAIL' 'build-livecheck' "exit $($r.Code)"; Show-LogTail $r.Log 30
  } else {
    Add-Result 'PASS' 'build-livecheck' "$($r.Seconds)s"
    $common = "--ytdlp `"$YtdlpPath`" --q1 `"$Query1`" --q2 `"$Query2`""

    function Invoke-Phase([string]$name, [string]$prefix = '') {
      $r = Invoke-Logged "live-$name$prefix" "`"$Live`" $name $common"
      Read-CheckLines $r.Log $prefix
      if ($r.Code -ne 0 -and (Select-String -Path $r.Log -Pattern '^CHECK' -Quiet) -eq $false) { Add-Result 'FAIL' "live-$name" "no result lines (exit $($r.Code))"; Show-LogTail $r.Log 20 }
    }

    # Offline: proves the rate-limit cooldown on this PC without touching the real service.
    Invoke-Phase 'fake429'

    Invoke-Phase 'formats'
    if (($env:PYXEE_COOKIES -or $env:PYXEE_COOKIES_FROM_BROWSER) -and -not $script:LiveBlocked) {
      $saveF = $env:PYXEE_COOKIES; $saveB = $env:PYXEE_COOKIES_FROM_BROWSER
      $env:PYXEE_COOKIES = $null; $env:PYXEE_COOKIES_FROM_BROWSER = $null
      Write-Report 'same format list WITHOUT cookies, to compare what the cookies change:'
      Invoke-Phase 'formats' 'nocookies-'
      $env:PYXEE_COOKIES = $saveF; $env:PYXEE_COOKIES_FROM_BROWSER = $saveB
    }

    foreach ($ph in @('search', 'stream')) {
      if ($script:LiveBlocked) { Add-Result 'WARN' "live-$ph" 'skipped: YouTube is rate limiting this IP right now (try again later, or use cookies)'; continue }
      Invoke-Phase $ph
    }

    # Stress: 9 simultaneous calls. Watch from outside how many yt-dlp processes run at once.
    if ($script:LiveBlocked) { Add-Result 'WARN' 'live-stress' 'skipped: rate limited' }
    else {
      $out = Join-Path $LogDir 'live-stress.log'; $err = Join-Path $LogDir 'live-stress.err'
      $argl = "stress $common"
      $proc = Start-Process -FilePath $Live -ArgumentList $argl -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
      $max = 0; $samples = 0; $busy = 0
      while (-not $proc.HasExited) {
        $n = Get-YtdlpRoots
        $samples++
        if ($n -gt 0) { $busy++ }
        if ($n -gt $max) { $max = $n }
        Start-Sleep -Milliseconds 80
      }
      $proc.WaitForExit()
      Read-CheckLines $out
      if ($max -eq 1) { Add-Result 'PASS' 'stress-yt-dlp-one-at-a-time' "at most 1 yt-dlp run at any moment ($busy of $samples samples had one running)" }
      elseif ($max -eq 0) { Add-Result 'WARN' 'stress-yt-dlp-one-at-a-time' "no yt-dlp process was observed in $samples samples (sampling missed it)" }
      else { Add-Result 'FAIL' 'stress-yt-dlp-one-at-a-time' "saw $max yt-dlp runs at the same time" }
    }
  }
  Add-TripResult 'live-checks' $before
  if ($script:LiveBlocked) { Add-Result 'WARN' 'live-summary' 'YouTube rate limiting was hit. That is the situation the guard handles; paste this report, and re-run in ~15 minutes or with -CookieFile.' }
}

# ---------------------------------------------------------------- 6. manual checks in the real app

if ($Manual) {
  Section 'Manual checks in the real app (about 10 minutes)'
  Write-Report 'Start the app in another window:  dev.cmd   (wait for it to open). Answer each question: y = yes/works, n = no, s = skip.'
  $items = @(
    @('app-search', "Search '$Query1' on the Songs filter: results appear within ~10 s?"),
    @('app-play', 'Click a result: sound starts within ~10 s and the progress bar moves?'),
    @('app-seek-middle', 'Drag the progress bar to the middle: sound continues from there within ~3 s?'),
    @('app-seek-end', 'Drag to ~10 s before the end: the next song (add 2 songs to the queue first) starts with no audible gap?'),
    @('app-next-hammer', 'Press Next 6 times quickly: you end up on the last song and it plays (no endless spinner, no error pile-up)?'),
    @('app-filters', 'Switch the search filter 6 times quickly: results show each time, no "rate limited" message?'),
    @('app-pause', 'Pause for 20 s, resume: it continues without a glitch?'),
    @('app-quality', 'Does the sound quality seem acceptable (it is ~128 kbps AAC now, was ~130-160 kbps Opus)?'),
    @('app-opus-skip', "Optional: a song that cannot be played shows a 'skipped' notice and the next one plays (skip if you never see one)?")
  )
  foreach ($it in $items) {
    $a = ''
    while ($a -notin @('y', 'n', 's')) { $a = (Read-Host ("  {0}  [y/n/s]" -f $it[1])).Trim().ToLower() }
    if ($a -eq 'y') { Add-Result 'PASS' $it[0] 'confirmed by you' }
    elseif ($a -eq 'n') { $note = Read-Host '    what happened? (one line)'; Add-Result 'FAIL' $it[0] "reported by you: $note" }
    else { Add-Result 'INFO' $it[0] 'skipped' }
  }
  $AnalyzeLog = $true
}

# ---------------------------------------------------------------- 7. the app's own log

if ($AnalyzeLog) {
  Section "The app's own log"
  $logFile = Join-Path $env:APPDATA 'dev.pyxee.app\logs\pyxee.log'
  if (-not (Test-Path $logFile)) { Add-Result 'WARN' 'app-log' 'log file not found (start the app once, or PYXEE_LOG may point elsewhere)' }
  else {
    $lines = @(Get-Content $logFile -Tail 4000)
    Add-Result 'INFO' 'app-log' "last $($lines.Count) lines of pyxee.log"
    $count = { param($re) @($lines | Where-Object { $_ -match $re }).Count }
    $fmt = @($lines | Where-Object { $_ -match 'resolved .*ext=(\S+) acodec=(\S+)' } | ForEach-Object { if ($_ -match 'ext=(\S+) acodec=(\S+)') { "$($Matches[1])/$($Matches[2])" } } | Group-Object | Sort-Object Count -Descending)
    if ($fmt.Count -gt 0) {
      Add-Result 'INFO' 'app-log-formats-played' (($fmt | ForEach-Object { "$($_.Name) x$($_.Count)" }) -join ', ')
      $odd = @($fmt | Where-Object { $_.Name -notmatch '^m4a/.*mp4a\.40\.2' })
      if ($odd.Count -eq 0) { Add-Result 'PASS' 'app-log-only-aac-lc-m4a' 'every resolved stream was AAC-LC in M4A' } else { Add-Result 'FAIL' 'app-log-only-aac-lc-m4a' (($odd | ForEach-Object { $_.Name }) -join ', ') }
    } else { Add-Result 'INFO' 'app-log-formats-played' 'no "resolved ... ext= acodec=" lines yet (play a few songs first)' }
    Add-Result 'INFO' 'app-log-gate-waits' ("{0} waits logged by the yt-dlp gate" -f (& $count 'yt-dlp .* waited'))
    Add-Result 'INFO' 'app-log-cooldowns' ("{0} rate-limit cooldown start(s), {1} refusal(s)" -f (& $count 'was rate limited'), (& $count 'refused: rate-limit'))
    $nff = & $count '(?i)ffmpeg'
    if ($nff -eq 0) { Add-Result 'PASS' 'app-log-no-ffmpeg' 'no ffmpeg lines' } else { Add-Result 'FAIL' 'app-log-no-ffmpeg' "$nff line(s) mention ffmpeg" }
    $cant = @($lines | Where-Object { $_ -match 'cannot decode this stream|seeking to .* failed|could not start|panicked|ERROR' })
    if ($cant.Count -eq 0) { Add-Result 'PASS' 'app-log-no-decode-errors' 'no decode/seek/spawn errors or panics' }
    else { Add-Result 'WARN' 'app-log-notable-lines' "$($cant.Count) notable line(s); first 15 below"; $cant | Select-Object -First 15 | ForEach-Object { Write-Report ("    " + $_.Substring(0, [math]::Min($_.Length, 220))) } }
    $ck = @($lines | Where-Object { $_ -match 'cookie' } | Select-Object -Last 2)
    foreach ($c in $ck) { Write-Report ("    " + $c.Substring(0, [math]::Min($c.Length, 200))) }
  }
}

# ---------------------------------------------------------------- summary

Write-Report ''
Write-Report '=== SUMMARY ===' 'Cyan'
Write-Report ("PASS {0}   FAIL {1}   WARN {2}   INFO {3}" -f $script:Counts.PASS, $script:Counts.FAIL, $script:Counts.WARN, $script:Counts.INFO) $(if ($script:Counts.FAIL -eq 0) { 'Green' } else { 'Red' })
Write-Report ("finished  : {0}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'))
Write-Report ("report    : {0}" -f $Report)
Write-Report ("full logs : {0}" -f $LogDir)
try { Get-Content $Report -Raw | Set-Clipboard; Write-Host 'The report was copied to your clipboard: paste it into the chat.' -ForegroundColor Cyan } catch { Write-Host "Paste the contents of $Report into the chat." -ForegroundColor Cyan }
exit $(if ($script:Counts.FAIL -eq 0) { 0 } else { 1 })
