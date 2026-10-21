<#
.SYNOPSIS
  Pyxee Windows check: builds, runs every test, proves no ffmpeg process is ever started, and runs live YouTube
  checks of the real search/resolve/decode/seek/gapless path. Writes one pasteable report.

.DESCRIPTION
  Run it from anywhere:   scripts\check-windows.cmd        (or:  powershell -File scripts\check-windows.ps1)
  Needs: Rust (rustup, with rustfmt + clippy), Node.js, git, internet, plus MSYS2 (make, nasm, diffutils,
  pkgconf) and the Visual Studio C++ build tools to build the minimal static FFmpeg libraries the decoder links
  in-process (`cargo xtask ffmpeg`, built once). No ffmpeg.exe is used or wanted: the tripwire below proves it.
  Output: pyxee-check\report-<time>.txt  (paste its contents back). Full logs: pyxee-check\logs\.
  The report never contains stream URLs (only hosts), cookie paths/values, or your user name.

.PARAMETER Ytdlp          Path to yt-dlp.exe (default: PYXEE_YTDLP, crates\app\binaries, then PATH).
.PARAMETER Query1/2       Search queries used for the two live tracks (default: two CC-licensed songs).
.PARAMETER CookieFile     Netscape cookies.txt to use (sets PYXEE_COOKIES for this run only).
.PARAMETER CookieBrowser  e.g. firefox (sets PYXEE_COOKIES_FROM_BROWSER for this run only).
.PARAMETER SkipTests      Skip fmt/clippy/cargo test (only do static checks + live checks).
.PARAMETER SkipLive       NOT RECOMMENDED. Skip everything that talks to YouTube. Without it the live checks always run, and
                          a live phase that could not run (no FFmpeg libraries, no yt-dlp, rate limited) is a FAIL.
.PARAMETER SkipTools      Do not run `cargo xtask tools` (the yt-dlp download flow).
.PARAMETER Ui             Kept for old command lines. The UI checks (npm ci / check / test / build, the named tests of the UI rebuild and the UI size report) always run now; the static UI facts run even with -SkipTests.
.PARAMETER SelfTest       Only run the redaction self-test (it also runs first in every full run) and exit.
.PARAMETER Installer      Also build the installer and the portable zip (`cargo xtask installer` / `portable`, slow) and inspect them.
.PARAMETER Ci            Replay the release workflow's "Build the portable zip" step the way a clean GitHub runner meets it: the
                          FFmpeg libraries already built (a cache hit), and MSYS2 starting for the very first time (a fresh home
                          folder, so its one-time "Copying skeleton files" output appears), then unpack the zip and run what it
                          carries, as release.yml does. Slow (builds the app); catches bugs that only show on a clean machine.
.PARAMETER UpdateYtdlp    Run `yt-dlp -U` first (an old yt-dlp is the most common cause of YouTube failures).
.PARAMETER Manual         After the automatic part, ask you to confirm things in the real app (y/n), including the UI items (each window and view, light and dark, keyboard only, narrow and wide, high DPI).
.PARAMETER AnalyzeLog     Summarise the app's own log file (run after using the app).
.PARAMETER FlakeRuns      Repeats of the known timing-sensitive pause test (default 10).
.PARAMETER KeepBuild      Keep target\debug and the FFmpeg build tree (default: delete them before exiting; the small
                          static FFmpeg libraries in target\ffmpeg are always kept, so later runs skip that build).
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
  [switch]$SelfTest,
  [switch]$Installer,
  [switch]$Ci,
  [switch]$UpdateYtdlp,
  [switch]$Manual,
  [switch]$AnalyzeLog,
  [int]$FlakeRuns = 10,
  [switch]$KeepBuild
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
# Live checks are part of the default run. Only an explicit -SkipLive makes skipping them acceptable.
$script:LiveRequested = -not $SkipLive
$script:Step = 0
# Planned number of sections, shown as "Section N of M". An estimate: it is lowered if the live checks get skipped later.
$script:StepTotal = 4 + [int](-not $SkipTests) + [int][bool]$Installer + [int][bool]$Ci + [int](-not $SkipLive) + [int][bool]$Manual + [int]([bool]$AnalyzeLog -or [bool]$Manual)
$script:SectionTitle = 'starting'
$script:RunSw = [Diagnostics.Stopwatch]::StartNew()

# ---------------------------------------------------------------- output helpers

# Values that identify this PC or this user, each with the label that replaces it. Paths are matched in every spelling
# they show up in (backslashes, forward slashes, JSON-doubled backslashes), ignoring case, longest first so that
# %LOCALAPPDATA% wins over %USERPROFILE%.
$script:ScrubRules = $null
function Get-ScrubRules {
  if ($script:ScrubRules) { return $script:ScrubRules }
  $list = New-Object System.Collections.Generic.List[object]
  $paths = [ordered]@{ LOCALAPPDATA = $env:LOCALAPPDATA; APPDATA = $env:APPDATA; TEMP = $env:TEMP; TMP = $env:TMP; USERPROFILE = $env:USERPROFILE; HOME = $env:HOME; HOMEPATH = $env:HOMEPATH }
  foreach ($k in $paths.Keys) {
    $v = [string]$paths[$k]
    if ($v.Length -lt 4) { continue }
    $v = $v.TrimEnd('\', '/')
    foreach ($form in @($v, $v.Replace('\', '/'), $v.Replace('\', '\\'))) { $list.Add(@{ Pattern = [regex]::Escape($form); To = "%$k%" }) }
  }
  foreach ($k in @('USERNAME', 'COMPUTERNAME', 'USERDOMAIN')) {
    $v = [string][Environment]::GetEnvironmentVariable($k)
    if ($v.Length -ge 3) { $list.Add(@{ Pattern = '(?<![A-Za-z0-9])' + [regex]::Escape($v) + '(?![A-Za-z0-9])'; To = "<$($k.ToLower())>" }) }
  }
  $script:ScrubRules = @($list | Sort-Object { $_.Pattern.Length } -Descending)
  return $script:ScrubRules
}

function Scrub([string]$s) {
  if ($null -eq $s) { return '' }
  $ci = [Text.RegularExpressions.RegexOptions]::IgnoreCase
  # Netscape cookie-file lines (host, flag, path, secure, expiry, name, value) and cookie/authorization headers.
  $s = [regex]::Replace($s, '(?m)^\.?[A-Za-z0-9.-]*(youtube|google|googlevideo)\.com\t[^\r\n]*$', '<cookie line removed>', $ci)
  $s = [regex]::Replace($s, '(?i)\b((?:set-)?cookie|authorization|proxy-authorization)(\s*[:=]\s*)[^\r\n]+', '$1$2<redacted>')
  # Stream and other URLs: only the host stays; the signed query string and path (which identify the stream and the
  # session) never reach the report. Google's stream hosts carry a per-session name, so even that is shortened.
  $s = [regex]::Replace($s, 'https?://([^/\s"''<>)]+)[^\s"''<>)]*', {
      param($m)
      $h = $m.Groups[1].Value
      if ($h -match '(?i)googlevideo\.com$') { $h = '*.googlevideo.com' }
      "<url host=$h>"
    })
  # Google's stream hosts name the session (rr5---sn-xxxx.googlevideo.com), also when printed without a scheme.
  $s = [regex]::Replace($s, '(?i)\b[a-z0-9.-]*googlevideo\.com\b', '*.googlevideo.com')
  # key=value / key: value secrets, also inside JSON ("token":"...") and command lines (--password x).
  $s = [regex]::Replace($s, '(?i)(["'']?\b(?:access_token|refresh_token|id_token|token|api[_-]?key|password|passwd|secret|signature|sig|sapisid|apisid|hsid|ssid|sid|psid|visitor_data|po_?token|session_?id|cookie_?value)\b["'']?\s*[=:]\s*["'']?)[^\s&;,"''<>]+', '$1<redacted>')
  $s = [regex]::Replace($s, '(?i)(--(?:password|video-password|cookies-from-browser|username)[ =])\S+', '$1<redacted>')
  $s = [regex]::Replace($s, '(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}', 'Bearer <redacted>')
  $s = [regex]::Replace($s, '\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b', '<github-token>')
  $s = [regex]::Replace($s, '\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*', '<jwt>')
  foreach ($r in (Get-ScrubRules)) { $s = [regex]::Replace($s, $r.Pattern, $r.To, $ci) }
  return $s
}

# Proves the redaction above on made-up secrets plus this PC's real user, computer and folders. Runs first in every
# run: if it fails, nothing is written to the report and the run stops.
function Test-Scrub {
  $bad = New-Object System.Collections.Generic.List[string]
  $check = {
    param([string]$name, [string]$text, [string[]]$mustNotContain, [string[]]$mustContain = @())
    $out = Scrub $text
    foreach ($m in $mustNotContain) { if ($m -and $out.IndexOf($m, [StringComparison]::OrdinalIgnoreCase) -ge 0) { $bad.Add("$name leaked '$m' -> $out") } }
    foreach ($m in $mustContain) { if ($out.IndexOf($m, [StringComparison]::OrdinalIgnoreCase) -lt 0) { $bad.Add("$name lost '$m' -> $out") } }
  }
  & $check 'stream-url' 'GET https://rr3---sn-abc123.googlevideo.com/videoplayback?expire=1&sig=SECRETSIG&lsig=AAA&id=o-XYZ done' @('SECRETSIG', 'o-XYZ', 'rr3---sn-abc123', 'videoplayback') @('googlevideo.com', 'done')
  & $check 'bare-stream-host' 'resolved host=rr5---sn-bavcx-jg3z.googlevideo.com duration=137' @('rr5---sn-bavcx-jg3z') @('googlevideo.com', 'duration=137')
  & $check 'cookie-header' 'Cookie: SAPISID=abc123SECRET; HSID=xyz' @('abc123SECRET', 'xyz')
  & $check 'set-cookie' 'set-cookie: LOGIN_INFO=SECRETCOOKIE; Path=/' @('SECRETCOOKIE')
  & $check 'authorization' 'authorization: Bearer abcdefSECRETtoken123' @('abcdefSECRETtoken123')
  & $check 'json-token' '{"access_token":"SECRETJSONTOKEN","x":1}' @('SECRETJSONTOKEN')
  & $check 'cli-password' 'yt-dlp --password hunter2SECRET --no-warnings' @('hunter2SECRET') @('--no-warnings')
  & $check 'github-token' 'push with ghp_abcdefghijklmnopqrstuvwxyz0123456789 ok' @('ghp_abcdefghijklmnopqrstuvwxyz0123456789')
  & $check 'jwt' 'id eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpM end' @('eyJhbGciOiJIUzI1NiJ9')
  & $check 'netscape-cookie' ".youtube.com`tTRUE`t/`tTRUE`t1893456000`tSID`tSECRETNETSCAPE" @('SECRETNETSCAPE')
  foreach ($k in @('USERPROFILE', 'LOCALAPPDATA', 'APPDATA', 'TEMP')) {
    $v = [string][Environment]::GetEnvironmentVariable($k)
    if ($v.Length -lt 4) { continue }
    $v = $v.TrimEnd('\', '/')
    & $check "path-$k" "in $v\x\y.log" @($v)
    & $check "path-$k-lower" ("in " + $v.ToLowerInvariant() + "\x\y.log") @($v)
    & $check "path-$k-slashes" ("in " + $v.Replace('\', '/') + "/x/y.log") @($v.Replace('\', '/'))
    & $check "path-$k-json" ("in " + $v.Replace('\', '\\') + "\\x\\y.log") @($v.Replace('\', '\\'))
  }
  foreach ($k in @('USERNAME', 'COMPUTERNAME')) {
    $v = [string][Environment]::GetEnvironmentVariable($k)
    if ($v.Length -ge 3) { & $check "name-$k" "owner $v ran it" @($v) }
  }
  return ,$bad
}

function Format-Elapsed([TimeSpan]$t) { return ('{0:00}:{1:00}:{2:00}' -f [int][math]::Floor($t.TotalHours), $t.Minutes, $t.Seconds) }

function Write-Report([string]$text, [string]$color = '') {
  $t = Scrub $text
  Add-Content -Path $Report -Value $t -Encoding UTF8
  if ($color) { Write-Host $t -ForegroundColor $color } else { Write-Host $t }
}

function Section([string]$title) {
  $script:Step++
  $script:SectionTitle = $title
  Write-Report ''
  Write-Report ("=== {0} of {1}. {2} (total elapsed {3}) ===" -f $script:Step, $script:StepTotal, $title, (Format-Elapsed $script:RunSw.Elapsed)) 'Cyan'
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

# Last non-empty line of a log that another process is still writing (console progress only; never goes in the report).
function Get-LogLastLine([string]$LogFile) {
  try {
    if (-not (Test-Path $LogFile)) { return '' }
    $l = Get-Content $LogFile -Tail 3 -ErrorAction Stop | Where-Object { $_.Trim() } | Select-Object -Last 1
    if ($null -eq $l) { return '' }
    $l = (Scrub $l).Trim()
    return $l.Substring(0, [math]::Min($l.Length, 90))
  } catch { return '' }
}

# Runs a command line through a temp .cmd so stderr/stdout go to a log without PowerShell wrapping them.
# While it runs, the console shows a Write-Progress bar plus a heartbeat line every 30 s, so long steps are never silent.
# Console only: the redacted report file is unchanged.
function Invoke-Logged([string]$Name, [string]$CommandLine, [string]$WorkDir = $Root, [string]$Label = '') {
  $log = Join-Path $LogDir ("{0}.log" -f $Name)
  $bat = Join-Path $LogDir ("{0}.cmd" -f $Name)
  # `call` matters: npm is a .cmd, and a batch file that runs another one without it never gets control back.
  $content = "@echo off`r`ncd /d `"$WorkDir`"`r`ncall $CommandLine > `"$log`" 2>&1`r`nexit /b %errorlevel%`r`n"
  Set-Content -Path $bat -Value $content -Encoding OEM
  if (-not $Label) { $Label = $Name }
  $activity = if ($script:Step -eq 0) { 'Setup' } else { "Section {0} of {1}: {2}" -f $script:Step, $script:StepTotal, $script:SectionTitle }
  # Version/path probes finish instantly and stay quiet; every other step announces itself before it can go silent.
  $quiet = $Name -like 'probe-*'
  if (-not $quiet) { Write-Host ("  > {0} ..." -f $Label) -ForegroundColor DarkGray }
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process -FilePath $env:ComSpec -ArgumentList ('/d /c ""{0}""' -f $bat) -NoNewWindow -PassThru
  $null = $p.Handle   # cache the process handle so ExitCode stays readable after exit
  $nextBeat = 30
  $lastPoll = -1
  while (-not $p.HasExited) {
    Start-Sleep -Milliseconds 250
    $sec = [int]$sw.Elapsed.TotalSeconds
    if (-not $quiet -and $sec -ne $lastPoll -and ($sec % 2) -eq 0) {
      $lastPoll = $sec
      Write-Progress -Id 1 -Activity $activity -Status ("{0}  elapsed {1}  | {2}" -f $Label, (Format-Elapsed $sw.Elapsed), (Get-LogLastLine $log)) -CurrentOperation ("total elapsed {0}" -f (Format-Elapsed $script:RunSw.Elapsed))
    }
    if (-not $quiet -and $sw.Elapsed.TotalSeconds -ge $nextBeat) {
      Write-Host ("    ... {0} still running, {1} elapsed (total {2})" -f $Label, (Format-Elapsed $sw.Elapsed), (Format-Elapsed $script:RunSw.Elapsed)) -ForegroundColor DarkGray
      $nextBeat += 30
    }
  }
  $p.WaitForExit()
  $code = $p.ExitCode
  $sw.Stop()
  Write-Progress -Id 1 -Activity $activity -Completed
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
# Phase 0: the redaction must work before anything is written to the report.
$scrubBad = Test-Scrub
if ($scrubBad.Count -gt 0) {
  Write-Host 'REDACTION SELF-TEST FAILED: nothing was written to the report. The output below is NOT safe to share.' -ForegroundColor Red
  foreach ($b in $scrubBad) { Write-Host "  $b" -ForegroundColor Red }
  exit 3
}
if ($SelfTest) { Write-Host 'Redaction self-test passed.' -ForegroundColor Green; exit 0 }

Write-Report ("windows   : {0}   powershell {1}" -f [Environment]::OSVersion.VersionString, $PSVersionTable.PSVersion)
Write-Report ("git       : {0}  ({1} changed file(s) in working tree)" -f (Get-FirstLine 'git rev-parse --short HEAD'), @(& git status --porcelain 2>$null | Where-Object { $_ -notmatch 'pyxee-check' }).Count)
Write-Report ("rustc     : {0}" -f (Get-FirstLine 'rustc --version'))
Write-Report ("cargo     : {0}" -f (Get-FirstLine 'cargo --version'))
Write-Report ("node      : {0}" -f (Get-FirstLine 'node --version'))
Write-Report ("cookies   : {0}" -f $cookieKind)
$envset = @('PYXEE_YTDLP', 'PYXEE_COOKIES', 'PYXEE_COOKIES_FROM_BROWSER', 'PYXEE_LOG', 'CARGO_TARGET_DIR') | ForEach-Object { $v = [Environment]::GetEnvironmentVariable($_); if ($v) { "$_=set" } }
Write-Report ("env       : {0}" -f $(if ($envset) { $envset -join ', ' } else { '(none of the PYXEE_* / CARGO_TARGET_DIR variables are set)' }))
Write-Report ("options   : SkipTests=$SkipTests SkipLive=$SkipLive SkipTools=$SkipTools Ui=$true Installer=$Installer Ci=$Ci Manual=$Manual AnalyzeLog=$AnalyzeLog")

foreach ($tool in @('cargo', 'rustc', 'git')) {
  if (-not (Have $tool)) { Add-Result 'FAIL' "tool-$tool" "$tool is not on PATH; install it and re-run"; Write-Report 'Cannot continue.'; exit 2 }
}
$tripOk = Initialize-Tripwire
if ($tripOk) { Add-Result 'INFO' 'tripwire' 'fake ffmpeg.exe/ffprobe.exe placed first on PATH for every step below' }
else { Add-Result 'WARN' 'tripwire' 'could not build the ffmpeg tripwire; the no-ffmpeg runtime proof is skipped'; Show-LogTail (Join-Path $LogDir 'tripwire-build.log') 10 }
$realFfmpeg = @(Get-Command ffmpeg.exe -All -ErrorAction SilentlyContinue | Where-Object { $_.Source -notlike "$TripDir*" })
if ($realFfmpeg.Count -gt 0) { Add-Result 'INFO' 'system-ffmpeg' 'a real ffmpeg is installed on this PC; it is shadowed by the tripwire during this run' }

# The tests added or changed by the review fixes. The full `cargo test` run asks for everything; this asks for each of
# these by name, and fails when one did not run (renamed, removed, or its crate did not build) or did not pass. The
# pyxee-audio and pyxee-app ones can only run on Windows: their crates need the FFmpeg libraries and WebView2.
function Test-EditedTests {
  $groups = @(
    @{ Crate = 'pyxee-sources'; Names = @(
      'the_cookie_file_path_is_replaced_in_either_slash_style_and_case',
      'a_browser_profile_is_replaced',
      'user_profile_folders_become_their_variable_names',
      'the_value_of_a_cookies_argument_is_replaced',
      'errors_are_classified_for_the_ui',
      'a_403_or_a_5xx_from_youtube_is_transient_and_a_missing_video_is_not',
      'empty_queries_search_nothing_and_non_youtube_keys_are_refused') },
    @{ Crate = 'pyxee-core'; Names = @(
      'a_prefetch_that_fails_while_the_player_waits_for_it_at_the_boundary_does_not_leave_loading_forever',
      'removing_the_song_that_is_loading_at_the_boundary_moves_on_to_the_one_after_it',
      'clearing_while_waiting_for_radio_ignores_the_answer_when_it_comes',
      'pausing_at_the_end_of_the_queue_cancels_the_wait_for_radio',
      'a_radio_answer_that_arrives_after_the_queue_was_replaced_is_ignored',
      'a_broken_or_missing_tool_is_not_the_songs_fault_so_nothing_is_skipped',
      'a_failed_radio_request_does_not_mark_radio_as_dry_and_ends_the_wait',
      'sound_from_an_old_stream_does_not_revive_a_stopped_or_failed_player',
      'an_expired_url_while_paused_unloads_the_song_and_keeps_the_position',
      'something_the_listener_asks_for_gets_a_fresh_url_retry_budget',
      'a_song_the_listener_plays_again_or_that_becomes_audible_is_no_longer_skipped',
      'playing_an_empty_list_changes_nothing',
      'adding_after_a_clear_forgets_the_clear_so_undo_cannot_overwrite_the_new_songs') },
    @{ Crate = 'pyxee-audio'; Names = @('flush_returns_at_once_while_the_producer_waits_for_a_chunk_that_never_arrives') },
    @{ Crate = 'pyxee-app'; Names = @(
      'a_seek_after_pausing_and_starting_another_song_is_not_silent',
      'a_second_install_is_refused_until_the_first_is_done',
      'only_the_build_time_id_counts_and_nothing_means_off') }
  )
  foreach ($g in $groups) {
    $r = Invoke-Logged ('edited-tests-' + $g.Crate) ('cargo test --locked -p {0} -- {1}' -f $g.Crate, ($g.Names -join ' '))
    $txt = if (Test-Path $r.Log) { Get-Content $r.Log -Raw } else { '' }
    foreach ($n in $g.Names) {
      $esc = [regex]::Escape($n)
      if ($txt -match ('(?m)^test \S*' + $esc + ' \.\.\. ok\r?$')) { Add-Result 'PASS' "edited-test-$n" 'ran and passed' }
      elseif ($txt -match ('(?m)^test \S*' + $esc + ' \.\.\. FAILED')) { Add-Result 'FAIL' "edited-test-$n" 'FAILED'; Show-LogTail $r.Log 30 }
      else { Add-Result 'FAIL' "edited-test-$n" 'did not run (renamed, removed, or its crate did not build): see the log' }
    }
  }
}

# Static facts about the review fixes F1-F10: cheap, and they run even with -SkipTests, so a later edit that undoes one of
# them is noticed. The behaviour itself is covered by Test-EditedTests (cargo) and by the manual items below.
function Test-ReviewFixes {
  $read = { param([string]$rel) $p = Join-Path $Root $rel; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw } else { '' } }
  $expect = { param([string]$name, [bool]$ok, [string]$detail) if ($ok) { Add-Result 'PASS' $name $detail } else { Add-Result 'FAIL' $name $detail } }
  $ytdlp = & $read 'crates\sources\src\ytdlp.rs'
  $http = & $read 'crates\audio\src\http.rs'
  & $expect 'fix-f1-no-args-in-logs' ($ytdlp -notmatch '\{args:\?\}') 'the yt-dlp timeout log does not print the argument list (cookie path, browser profile)'
  & $expect 'fix-f1-scrub-wired' ((((& $read 'crates\sources\src\lib.rs') -match 'pub use scrub::scrub') -and ((& $read 'crates\app\src\logging.rs') -match 'pyxee_sources::scrub'))) 'pyxee_sources::scrub exists and the file logger runs every message through it'
  & $expect 'fix-f2-cancel-token' (((& $read 'crates\audio\src\pipeline.rs') -match 'pub fn cancel_token') -and ($http -match 'pub fn open_with_cancel') -and ((& $read 'crates\app\src\engine.rs') -match 'open_with_cancel')) 'the pipeline has a cancel token and the engine opens streams with it'
  $fullLimit = $http -match 'FULL_LIMIT: u64 = 64 \* 1024 \* 1024'
  & $expect 'fix-f10-http-hardening' ($fullLimit -and ($http -match 'server ignored the range request')) 'FULL_LIMIT is 64 MiB and a reply without 206 + a matching Content-Range is refused'
  & $expect 'fix-f10-ignore-config' ($ytdlp -match '--ignore-config') 'every yt-dlp run starts with --ignore-config'
  $envvars = & $read 'ui\src\lib\envvars.ts'
  $readme = & $read 'README.md'
  $release = & $read '.github\workflows\release.yml'
  $discord = & $read 'crates\app\src\shell\discord.rs'
  $envfile = & $read 'crates\shell\src\envfile.rs'
  $f7 = ($envvars -notmatch 'DISCORD') -and ($envfile -notmatch 'DISCORD') -and ($readme -notmatch 'DISCORD_CLIENT_ID') -and ((& $read 'crates\app\src\advanced.rs') -notmatch 'DISCORD')
  & $expect 'fix-f7-discord-id-not-user-settable' $f7 'the Discord application ID is not in Settings > Advanced, the .env loader or the README: users cannot change it'
  & $expect 'fix-f7-discord-id-is-a-secret-in-ci' (($release -match 'secrets\.PYXEE_DISCORD_CLIENT_ID') -and ($release -notmatch 'vars\.PYXEE_DISCORD_CLIENT_ID')) 'release.yml reads the ID from a repository secret, not a variable'
  & $expect 'fix-f7-discord-log-text' (($discord -match 'this build has no Discord application') -and ($discord -notmatch 'is not set\)')) 'the Discord log line no longer asks for a variable'
  try {
    $mode = (Get-Content -LiteralPath (Join-Path $Root 'crates\app\tauri.conf.json') -Raw | ConvertFrom-Json).bundle.windows.webviewInstallMode
    & $expect 'fix-f8-webview2-bootstrapper' (($mode.type -eq 'embedBootstrapper') -and ($mode.silent -eq $false)) 'WebView2 is installed by the embedded bootstrapper, not silently (it shows its own progress)'
  } catch { Add-Result 'FAIL' 'fix-f8-webview2-bootstrapper' "could not read crates\app\tauri.conf.json: $($_.Exception.Message)" }
  & $expect 'fix-f8-progress-binding' ((Test-Path -LiteralPath (Join-Path $Root 'ui\src\lib\bindings\UpdateProgressDto.ts')) -and ((& $read 'crates\app\src\updater.rs') -match 'update-progress')) 'UpdateProgressDto.ts exists and the updater emits update-progress'
}

# ---------------------------------------------------------------- UI rebuild: static facts, named tests, report
# What svelte-check and the unit tests do not see about the UI rebuild ("Rail, Page, Stage", see docs\ui.md): remote
# resources, removed components, the windows and entry points, the test ids the tests rely on, the dependency list.
# The before figures were measured on the commit that holds the review fixes, just before the rebuild.
$script:UiBefore = @{ Components = 35; SvelteLines = 2528; CssLines = 252; TsLines = 2516; ClassTokens = 338; Tests = 330 }

# Tests the rebuild added or changed (their vitest titles). The full `npm test` run asks for everything; this asks for
# each of these by name and fails when one did not run (renamed or removed) or did not pass.
$script:UiNamedTests = @(
  @{ File = 'App.test.ts'; Names = @(
    'uses the bottom bar when narrow, with the four destinations',
    'wide layout puts the wordmark, the four destinations and Settings in a side rail, and the player on the right',
    'the wide layout shows what plays on the stage: transport, a preview of Up next, and a way to play from it',
    'marks the song that is playing in every list it appears in') },
  @{ File = 'motion.test.ts'; Names = @(
    'keeps the given lengths when the system does not ask for less motion',
    'drops lengths and distances to nothing when it does',
    'leaving is quicker than arriving') }
)
# 330 before; two App.test.ts tests were rewritten (same count) and 2 + 3 were added.
$script:UiExpectedTests = 335

function Get-UiSources([string[]]$Ext = @('.svelte', '.ts', '.css', '.html')) {
  $ui = Join-Path $Root 'ui'
  $files = @(Get-ChildItem -LiteralPath (Join-Path $ui 'src') -Recurse -File -ErrorAction SilentlyContinue | Where-Object { ($Ext -contains $_.Extension) -and ($_.Name -notmatch '\.test\.ts$') -and ($_.FullName -notmatch '\\bindings\\') })
  $index = Join-Path $ui 'index.html'
  if (($Ext -contains '.html') -and (Test-Path -LiteralPath $index)) { $files += Get-Item -LiteralPath $index }
  return ,$files
}

function Get-LineCount($files) { $n = 0; foreach ($f in $files) { $n += @(Get-Content -LiteralPath $f.FullName).Count }; return $n }

function Get-GzipSize([string]$path) {
  $bytes = [IO.File]::ReadAllBytes($path)
  $ms = New-Object IO.MemoryStream
  $gz = New-Object IO.Compression.GZipStream($ms, [IO.Compression.CompressionLevel]::Optimal)
  $gz.Write($bytes, 0, $bytes.Length)
  $gz.Close()
  return $ms.ToArray().Length
}

function Test-UiRebuild {
  $expect = { param([string]$name, [bool]$ok, [string]$detail) if ($ok) { Add-Result 'PASS' $name $detail } else { Add-Result 'FAIL' $name $detail } }
  $ui = Join-Path $Root 'ui'
  $files = Get-UiSources
  if ($files.Count -eq 0) { Add-Result 'FAIL' 'ui-sources' 'ui\src was not found'; return }
  $text = @{}
  foreach ($f in $files) { $text[$f.FullName] = Get-Content -LiteralPath $f.FullName -Raw }
  $rel = { param($f) $f.FullName.Substring($Root.Length + 1) }
  $hits = { param([string]$pattern) @($files | Where-Object { $text[$_.FullName] -match $pattern } | ForEach-Object { & $rel $_ }) }
  $read = { param([string]$relPath) $p = Join-Path $Root $relPath; if (Test-Path -LiteralPath $p) { Get-Content -LiteralPath $p -Raw } else { '' } }

  # (b1) Nothing remote: the CSP blocks remote scripts, styles, fonts and images, so one that sneaks in fails silently on the PC.
  $remote = & $hits '(?i)@import\s+url\(|url\(\s*[''"]?https?:|<link[^>]+href=["'']?https?:|<script[^>]+src=["'']?https?:|fonts\.googleapis|fonts\.gstatic|unpkg\.com|cdn\.jsdelivr|cdnjs\.'
  & $expect 'ui-no-remote-resources' ($remote.Count -eq 0) $(if ($remote.Count -eq 0) { 'no remote stylesheet, script, @import or font host in ui\src or index.html' } else { 'remote resource in: ' + ($remote -join ', ') })
  $fontRef = & $hits '(?i)@font-face|\.(woff2?|ttf|otf)\b'
  $fontFiles = @(foreach ($d in @('src', 'public')) { $p = Join-Path $ui $d; if (Test-Path -LiteralPath $p) { Get-ChildItem -LiteralPath $p -Recurse -File | Where-Object { $_.Extension -match '^\.(woff2?|ttf|otf)$' } } })
  & $expect 'ui-system-fonts-only' (($fontRef.Count -eq 0) -and ($fontFiles.Count -eq 0)) $(if (($fontRef.Count -eq 0) -and ($fontFiles.Count -eq 0)) { 'no @font-face and no font file: the text is Segoe UI Variable from Windows' } else { 'font reference or file: ' + (($fontRef + @($fontFiles | ForEach-Object { $_.Name })) -join ', ') })
  $css = & $read 'ui\src\app.css'
  & $expect 'ui-font-stack' (($css -match 'Segoe UI Variable Text') -and ($css -match 'Segoe UI Variable Display') -and ($css -notmatch 'Fredoka')) 'app.css names Segoe UI Variable Text and Display with system fallbacks'

  # (b2) Removed components must stay removed, in code and in docs.
  $gone = @('DeckNav', 'PlayerBar', 'BottomNav')
  $goneFiles = @($gone | Where-Object { Test-Path -LiteralPath (Join-Path $Root ('ui\src\components\' + $_ + '.svelte')) })
  $goneRefs = @(& git grep -n -E '\b(DeckNav|PlayerBar|BottomNav)\b' -- ui/src docs/ui.md 2>$null)
  & $expect 'ui-no-removed-components' (($goneFiles.Count -eq 0) -and ($goneRefs.Count -eq 0)) $(if (($goneFiles.Count -eq 0) -and ($goneRefs.Count -eq 0)) { 'DeckNav, PlayerBar and BottomNav are gone and nothing refers to them' } else { 'still there: ' + (($goneFiles + ($goneRefs | Select-Object -First 3)) -join ' | ') })

  # (b3) Windows and entry points: main, mini player and tray flyout, each with its page or view.
  $main = & $read 'ui\src\main.ts'
  $miniRs = & $read 'crates\app\src\shell\mini.rs'
  $flyRs = & $read 'crates\app\src\shell\flyout.rs'
  & $expect 'ui-entry-points' (($main -match "view === 'flyout'") -and ($main -match "view === 'mini'") -and ($main -match 'Flyout') -and ($main -match 'Mini') -and ($miniRs -match 'index\.html\?view=mini') -and ($flyRs -match 'index\.html\?view=flyout')) 'main.ts picks App, Mini or Flyout from ?view=, and the Rust side opens index.html?view=mini and ?view=flyout'
  $need = @('App.svelte', 'views\Mini.svelte', 'views\Flyout.svelte', 'views\Compact.svelte', 'views\Boot.ts', 'pages\Home.svelte', 'pages\Explore.svelte', 'pages\Library.svelte', 'pages\Playlist.svelte', 'pages\Queue.svelte', 'pages\Search.svelte', 'components\Navigation.svelte', 'components\Stage.svelte', 'components\Dock.svelte', 'components\NowPlaying.svelte', 'components\SettingsSheet.svelte', 'components\ThemeEditor.svelte', 'components\TrackMenu.svelte')
  $missing = @($need | Where-Object { -not (Test-Path -LiteralPath (Join-Path $Root ('ui\src\' + $_))) })
  & $expect 'ui-windows-and-views-present' ($missing.Count -eq 0) $(if ($missing.Count -eq 0) { "all $($need.Count) shell, view, page and sheet files exist" } else { 'missing: ' + ($missing -join ', ') })
  $app = & $read 'ui\src\App.svelte'
  $wired = @('Navigation', 'TopBar', 'Dock', 'Stage', 'NowPlaying', 'Snackbar', 'TrackMenu', 'PlaylistPicker', 'NameSheet', 'ThemeEditor', 'SleepSheet', 'SettingsSheet', 'Home', 'Explore', 'Library', 'Queue', 'Playlist', 'Search')
  $unwired = @($wired | Where-Object { $app -notmatch ('<' + $_ + '\b') })
  & $expect 'ui-app-wires-everything' ($unwired.Count -eq 0) $(if ($unwired.Count -eq 0) { 'App.svelte renders the rail/bar, top bar, dock, stage, Now Playing, every page and every sheet' } else { 'App.svelte does not render: ' + ($unwired -join ', ') })

  # (b4) The test ids, roles and live regions that tests and assistive technology rely on must still be in the code.
  $ids = @('player-bar', 'np-root', 'np-header', 'np-tabs', 'np-art', 'np-controls', 'np-scroll', 'page-header', 'page-scroll', 'drag-handle', 'upnext-handle', 'resume', 'compact-title', 'compact-art', 'mini', 'flyout', 'log-path', 'sleep-chip', 'sleep-status', 'lyrics', 'related')
  $noId = @($ids | Where-Object { (& $hits ('data-testid="' + $_ + '"')).Count -eq 0 })
  & $expect 'ui-test-ids' ($noId.Count -eq 0) $(if ($noId.Count -eq 0) { "all $($ids.Count) data-testid values the tests use are present" } else { 'missing data-testid: ' + ($noId -join ', ') })
  $snack = & $read 'ui\src\components\Snackbar.svelte'
  $sheet = & $read 'ui\src\components\Sheet.svelte'
  $np = & $read 'ui\src\components\NowPlaying.svelte'
  $scroll = & $read 'ui\src\components\ScrollPage.svelte'
  $seek = & $read 'ui\src\components\Seek.svelte'
  $a11y = ($snack -match 'aria-live="polite"') -and ($snack -match 'role="status"') -and ($sheet -match 'role="dialog"') -and ($sheet -match 'aria-modal="true"') -and ($sheet -match "key === 'Escape'") -and ($np -match 'role="alert"') -and ($np -match 'role="tabpanel"') -and ($scroll -match 'data-scroll[^>]*tabindex="0"') -and ($seek -match 'type="range"') -and ($seek -match 'aria-valuetext')
  & $expect 'ui-accessibility-structure' $a11y 'snackbar is a polite live region, sheets are modal dialogs that close on Escape, failures use role=alert, tab panels and scroll areas are reachable, seek is a real range input'
  $icon = & $read 'ui\src\components\IconButton.svelte'
  $clickDiv = & $hits '<(div|span|li|p|section)\b[^>]*\sonclick='
  & $expect 'ui-real-buttons' (($icon -match 'aria-label=\{label\}') -and ($clickDiv.Count -eq 0)) $(if ($clickDiv.Count -eq 0) { 'IconButton always carries an accessible name, and no div, span or li has a click handler' } else { 'click handler on a non-button in: ' + ($clickDiv -join ', ') })
  & $expect 'ui-focus-and-motion-rules' (($css -match ':focus-visible') -and ($css -match 'prefers-reduced-motion') -and ((& $read 'ui\src\lib\motion.ts') -match 'prefers-reduced-motion')) 'a visible focus rule exists, and reduced motion is honoured in CSS (app.css) and in script (lib\motion.ts)'

  # (b5) The design rules of this rebuild: tokens, no tracked capitals, no colour literals in components.
  $tok = @('--layer', '--hover', '--press', '--selected', '--stage-wash', '--r-sm', '--r-md', '--r-lg', '--dur-fast', '--dur-base', '--ease', '--rail-w', '--stage-w', '--page-pad')
  $noTok = @($tok | Where-Object { $css -notmatch ([regex]::Escape($_) + ':') })
  & $expect 'ui-design-tokens' (($noTok.Count -eq 0) -and ($css -match '@layer components') -and ($css -match 'container-type:\s*inline-size')) $(if ($noTok.Count -eq 0) { "$($tok.Count) semantic tokens, a component layer and the list container query are in app.css" } else { 'app.css lacks: ' + ($noTok -join ', ') })
  $caps = & $hits '\buppercase\b|\btracking-(wide|wider|widest)\b|text-transform:\s*uppercase'
  & $expect 'ui-no-tracked-capitals' ($caps.Count -eq 0) $(if ($caps.Count -eq 0) { 'no uppercase or tracked-out labels (the type is sentence case)' } else { 'uppercase or tracking in: ' + ($caps -join ', ') })
  $hex = @($files | Where-Object { ($_.Extension -eq '.svelte') -and ($_.Name -ne 'Logo.svelte') -and ($text[$_.FullName] -match '#[0-9a-fA-F]{6}\b') } | ForEach-Object { & $rel $_ })
  & $expect 'ui-no-colour-literals' ($hex.Count -eq 0) $(if ($hex.Count -eq 0) { 'components take colours from tokens; only the logo has its own' } else { 'hex colour in: ' + ($hex -join ', ') })

  # (b6) No new npm dependencies: package-lock.json cannot be regenerated by the rebuild, so `npm ci` must keep working.
  try {
    $pkg = (& $read 'ui\package.json') | ConvertFrom-Json
    $deps = @($pkg.dependencies.PSObject.Properties.Name | Sort-Object)
    $dev = @($pkg.devDependencies.PSObject.Properties.Name | Sort-Object)
    $wantDeps = @('@tauri-apps/api')
    $wantDev = @('@sveltejs/vite-plugin-svelte', '@tailwindcss/vite', '@tauri-apps/cli', '@testing-library/svelte', '@types/node', 'jsdom', 'svelte', 'svelte-check', 'tailwindcss', 'typescript', 'vite', 'vitest')
    $extra = @($deps + $dev | Where-Object { ($wantDeps + $wantDev) -notcontains $_ })
    & $expect 'ui-no-new-dependencies' ($extra.Count -eq 0) $(if ($extra.Count -eq 0) { 'ui\package.json has the same packages as before the rebuild' } else { 'new package(s): ' + ($extra -join ', ') + ' (package-lock.json would be out of date)' })
  } catch { Add-Result 'FAIL' 'ui-no-new-dependencies' "could not read ui\package.json: $($_.Exception.Message)" }

  # (a) Every test the rebuild added or changed is still in the test files under its title.
  foreach ($g in $script:UiNamedTests) {
    $tf = Get-ChildItem -LiteralPath (Join-Path $ui 'src') -Recurse -File -Filter $g.File -ErrorAction SilentlyContinue | Select-Object -First 1
    $src = if ($tf) { Get-Content -LiteralPath $tf.FullName -Raw } else { '' }
    foreach ($n in $g.Names) {
      $slug = ($n -replace '[^A-Za-z0-9]+', '-').Trim('-').ToLower()
      if ($slug.Length -gt 60) { $slug = $slug.Substring(0, 60) }
      & $expect ('ui-test-defined-' + $slug) ($src.Contains("'" + $n + "'")) ($g.File + ': ' + $n)
    }
  }

  # (c, static part) Size of the UI source before and after.
  $sv = @($files | Where-Object { $_.Extension -eq '.svelte' })
  $cssFiles = @($files | Where-Object { $_.Extension -eq '.css' })
  $tsFiles = @($files | Where-Object { $_.Extension -eq '.ts' })
  $svLines = Get-LineCount $sv
  Add-Result 'INFO' 'ui-components' ("{0} components (.svelte files), before the rebuild {1}: {2}" -f $sv.Count, $script:UiBefore.Components, $(if ($sv.Count -gt $script:UiBefore.Components) { 'smaller and more focused pieces' } else { 'not split further' }))
  Add-Result 'INFO' 'ui-lines' ("Svelte {0} lines (before {1}), app.css {2} (before {3}), TypeScript without tests {4} (before {5}); the design now lives in app.css tokens and classes instead of in markup" -f $svLines, $script:UiBefore.SvelteLines, (Get-LineCount $cssFiles), $script:UiBefore.CssLines, (Get-LineCount $tsFiles), $script:UiBefore.TsLines)
  $tokens = @{}
  foreach ($f in $sv) { foreach ($m in [regex]::Matches($text[$f.FullName], 'class="([^"]*)"')) { foreach ($t in ($m.Groups[1].Value -split '[\s{}]+')) { if ($t -and ($t -notmatch '^[''$?:]')) { $tokens[$t] = 1 } } } }
  Add-Result 'INFO' 'ui-class-vocabulary' ("{0} distinct class names in the markup (before {1}); fewer one-off utilities, more shared classes" -f $tokens.Count, $script:UiBefore.ClassTokens)
  $big = @($sv | ForEach-Object { [pscustomobject]@{ Name = $_.Name; Lines = @(Get-Content -LiteralPath $_.FullName).Count } } | Sort-Object Lines -Descending | Select-Object -First 3)
  Add-Result 'INFO' 'ui-largest-components' (($big | ForEach-Object { '{0} {1} lines' -f $_.Name, $_.Lines }) -join ', ')
  $huge = @($big | Where-Object { $_.Lines -gt 250 })
  if ($huge.Count -gt 0) { Add-Result 'WARN' 'ui-component-size' ('over 250 lines, consider splitting: ' + (($huge | ForEach-Object { $_.Name }) -join ', ')) }
}

# (a) The named tests, looked up in vitest's JSON report so a renamed, removed or failing one is named in the report.
function Test-UiNamedTests {
  $json = Join-Path $LogDir 'ui-named-tests.json'
  $names = @($script:UiNamedTests | ForEach-Object { $_.File })
  $r = Invoke-Logged 'ui-named-tests' ('npm test --prefix ui -- --reporter=json --outputFile="{0}" {1}' -f $json, ($names -join ' '))
  $all = @()
  if (Test-Path -LiteralPath $json) {
    try {
      $data = Get-Content -LiteralPath $json -Raw | ConvertFrom-Json
      foreach ($tr in $data.testResults) { foreach ($a in $tr.assertionResults) { $all += [pscustomobject]@{ File = (Split-Path $tr.name -Leaf); Title = $a.title; Status = $a.status } } }
    } catch { Add-Result 'FAIL' 'ui-named-tests' "could not read vitest's JSON report: $($_.Exception.Message)"; return }
  } else { Add-Result 'FAIL' 'ui-named-tests' 'vitest wrote no JSON report: see the log'; Show-LogTail $r.Log 20; return }
  foreach ($g in $script:UiNamedTests) {
    foreach ($n in $g.Names) {
      $m = @($all | Where-Object { ($_.File -eq $g.File) -and ($_.Title -eq $n) })
      if ($m.Count -eq 0) { Add-Result 'FAIL' "ui-test-$($g.File)" "did not run (renamed or removed): $n" }
      elseif ($m[0].Status -eq 'passed') { Add-Result 'PASS' "ui-test-$($g.File)" "ran and passed: $n" }
      else { Add-Result 'FAIL' "ui-test-$($g.File)" "$($m[0].Status): $n" }
    }
  }
}

# (c) Size of what the UI build produced; compared with the previous run's figures when there are any.
function Add-UiBundleReport {
  $assets = Join-Path $Root 'ui\dist\assets'
  if (-not (Test-Path -LiteralPath $assets)) { Add-Result 'WARN' 'ui-bundle' 'ui\dist\assets does not exist (the UI build did not run)'; return }
  $now = @{}
  foreach ($kind in @(@('js', '*.js'), @('css', '*.css'))) {
    $fs = @(Get-ChildItem -LiteralPath $assets -Recurse -File -Filter $kind[1])
    $raw = 0; $gz = 0
    foreach ($f in $fs) { $raw += $f.Length; $gz += (Get-GzipSize $f.FullName) }
    $now[$kind[0] + 'Raw'] = $raw; $now[$kind[0] + 'Gzip'] = $gz; $now[$kind[0] + 'Files'] = $fs.Count
    Add-Result 'INFO' ('ui-bundle-' + $kind[0]) ('{0} file(s), {1:N1} KB, {2:N1} KB gzipped' -f $fs.Count, ($raw / 1KB), ($gz / 1KB))
  }
  $other = @(Get-ChildItem -LiteralPath (Join-Path $Root 'ui\dist') -Recurse -File | Where-Object { $_.Extension -notin @('.js', '.css', '.html') })
  Add-Result 'INFO' 'ui-bundle-other-files' ("{0} other file(s) in ui\dist ({1:N1} KB): fonts and images the build copied, expected none but the logo and icons" -f $other.Count, ((($other | Measure-Object Length -Sum).Sum) / 1KB))
  if ($now.cssRaw -gt (200 * 1KB)) { Add-Result 'WARN' 'ui-bundle-css-size' ('the stylesheet is {0:N0} KB: check for utilities or a library that should not be there' -f ($now.cssRaw / 1KB)) }
  $last = Join-Path $OutDir 'ui-bundle-last.json'
  if (Test-Path -LiteralPath $last) {
    try {
      $prev = Get-Content -LiteralPath $last -Raw | ConvertFrom-Json
      Add-Result 'INFO' 'ui-bundle-vs-previous-run' ('JS {0:+0.0;-0.0;0.0} KB gzipped, CSS {1:+0.0;-0.0;0.0} KB gzipped compared with the previous run ({2})' -f (($now.jsGzip - $prev.jsGzip) / 1KB), (($now.cssGzip - $prev.cssGzip) / 1KB), $prev.when)
    } catch { Add-Result 'INFO' 'ui-bundle-vs-previous-run' 'the previous figures could not be read' }
  } else { Add-Result 'INFO' 'ui-bundle-vs-previous-run' 'no earlier run to compare with (run this script on the commit before the rebuild to get a baseline)' }
  try { ([pscustomobject]@{ when = (Get-Date -Format 'yyyy-MM-dd HH:mm'); jsGzip = $now.jsGzip; cssGzip = $now.cssGzip; jsRaw = $now.jsRaw; cssRaw = $now.cssRaw } | ConvertTo-Json) | Set-Content -LiteralPath $last -Encoding UTF8 } catch { }
}

# The number of tests vitest ran, against what the rebuild should have left (330 before, +5).
function Add-UiTestCount([string]$logFile) {
  $txt = if (Test-Path -LiteralPath $logFile) { Get-Content -LiteralPath $logFile -Raw } else { '' }
  $m = [regex]::Match($txt, '(?m)^\s*Tests\s+(?:(\d+) failed \| )?(\d+) passed(?: \| (\d+) skipped)? \((\d+)\)')
  if (-not $m.Success) { Add-Result 'WARN' 'ui-test-count' 'could not read the test count from the vitest log'; return }
  $total = [int]$m.Groups[4].Value
  if ($total -eq $script:UiExpectedTests) { Add-Result 'PASS' 'ui-test-count' "$total tests ($($script:UiBefore.Tests) before + $($script:UiExpectedTests - $script:UiBefore.Tests) added by the rebuild)" }
  else { Add-Result 'WARN' 'ui-test-count' "$total tests ran, the rebuild should leave $($script:UiExpectedTests) ($($script:UiBefore.Tests) before + $($script:UiExpectedTests - $script:UiBefore.Tests)); a test was removed or added since" }
}

# ---------------------------------------------------------------- 1. static: ffmpeg is gone from the repo

Section 'no ffmpeg process in code, build and packaging (libav is linked in-process)'
$bin = Join-Path $Root 'crates\app\binaries'
$ff = @(Get-ChildItem $bin -Filter 'ffmpeg*' -ErrorAction SilentlyContinue)
if ($ff.Count -eq 0) { Add-Result 'PASS' 'no-ffmpeg-sidecar-file' 'crates\app\binaries has no ffmpeg*' } else { Add-Result 'FAIL' 'no-ffmpeg-sidecar-file' ($ff.Name -join ', ') }
try {
  $conf = Get-Content (Join-Path $Root 'crates\app\tauri.bundle.conf.json') -Raw | ConvertFrom-Json
  $bins = @($conf.bundle.externalBin)
  if (($bins -join ' ') -match 'ffmpeg') { Add-Result 'FAIL' 'bundle-externalBin' ($bins -join ', ') } else { Add-Result 'PASS' 'bundle-externalBin' ("sidecars: " + ($bins -join ', ')) }
} catch { Add-Result 'WARN' 'bundle-externalBin' "could not parse tauri.bundle.conf.json: $($_.Exception.Message)" }
$code = @(& git grep -n -i -E 'FfmpegPipe|ffmpeg\.exe|PYXEE_FFMPEG_(PATH|BIN|EXE)|Command::new\(.*ffmpeg|externalBin.*ffmpeg|AudioError::Ffmpeg' -- '*.rs' '*.toml' '*.json' '*.cmd' '*.yml' ':!scripts' ':!xtask/src/release.rs' 2>$null)
if ($code.Count -eq 0) { Add-Result 'PASS' 'no-ffmpeg-in-code' 'no ffmpeg process/sidecar/config references in .rs .toml .json .cmd .yml' }
else { Add-Result 'FAIL' 'no-ffmpeg-in-code' "$($code.Count) reference(s)"; $code | Select-Object -First 10 | ForEach-Object { Write-Report "    $_" } }
$any = @(& git grep -n -i ffmpeg -- . ':!Cargo.lock' ':!scripts' 2>$null)
Add-Result 'INFO' 'ffmpeg-mentions-left' "$($any.Count) line(s) mention the word (expected: the in-process libav build scripts, docs and comments, plus the fixture-provenance note)"
$any | Select-Object -First 25 | ForEach-Object { Write-Report ("    " + $_.Substring(0, [math]::Min($_.Length, 170))) }
Test-ReviewFixes

# ---------------------------------------------------------------- 1b. static: the UI rebuild

Section 'UI rebuild: static facts the compiler and the tests do not see'
Test-UiRebuild

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

# yt-dlp wants an external JavaScript runtime for YouTube (its EJS solver scripts are bundled in yt-dlp.exe, so only the
# runtime is missing). It enables Deno 2.x on PATH by itself; any other runtime is switched on with --js-runtimes in
# a yt-dlp config file. Pyxee itself never needs or starts one, so this is a WARN with the fix, never a FAIL.
function Test-JsRuntime {
  $script:JsRuntimeWarned = $false
  $deno = Get-Command deno -ErrorAction SilentlyContinue
  if ($deno) {
    $v = Get-FirstLine 'deno --version'
    if ($v -match 'deno\s+(\d+)\.') {
      if ([int]$Matches[1] -ge 2) { Add-Result 'PASS' 'ytdlp-js-runtime' "$v on PATH (yt-dlp uses it automatically)"; return }
      Add-Result 'WARN' 'ytdlp-js-runtime' "$v is too old: yt-dlp needs Deno 2.0 or newer (winget upgrade DenoLand.Deno)"
      $script:JsRuntimeWarned = $true
      return
    }
  }
  $configs = @(
    (Join-Path $env:APPDATA 'yt-dlp\config'), (Join-Path $env:APPDATA 'yt-dlp\config.txt'),
    (Join-Path $env:USERPROFILE 'yt-dlp.conf'), (Join-Path (Split-Path -Parent $YtdlpPath) 'yt-dlp.conf')
  )
  $hit = @($configs | Where-Object { $_ -and (Test-Path $_) -and (Select-String -Path $_ -Pattern '^\s*--js-runtimes\b' -Quiet) })
  if ($hit.Count -gt 0) { Add-Result 'PASS' 'ytdlp-js-runtime' "--js-runtimes is set in $(Split-Path -Leaf $hit[0]) (the yt-dlp config file)"; return }
  $script:JsRuntimeWarned = $true
  $nodeOk = Have 'node'
  $alt = if ($nodeOk) { ' Or, since Node is installed: put the line "--js-runtimes node" in %APPDATA%\yt-dlp\config.' } else { '' }
  Add-Result 'WARN' 'ytdlp-js-runtime' ('no JavaScript runtime for yt-dlp. YouTube extraction without one is deprecated by yt-dlp (some formats may vanish later); Pyxee works for now. Fix: winget install DenoLand.Deno, then open a new terminal.' + $alt + ' See README, "yt-dlp and a JavaScript runtime".')
}

function Find-Ytdlp {
  if ($Ytdlp -and (Test-Path $Ytdlp)) { return (Resolve-Path $Ytdlp).Path }
  if ($env:PYXEE_YTDLP -and (Test-Path $env:PYXEE_YTDLP)) { return (Resolve-Path $env:PYXEE_YTDLP).Path }
  $cand = @(Get-ChildItem $bin -Filter 'yt-dlp*.exe' -ErrorAction SilentlyContinue | Sort-Object Name)
  if ($cand.Count -gt 0) { return $cand[0].FullName }
  $g = Get-Command yt-dlp -ErrorAction SilentlyContinue
  if ($g) { return $g.Source }
  return $null
}
if ($SkipLive) { Add-Result 'WARN' 'live-checks' '-SkipLive was given: this run does NOT prove search / resolve / decode / seek / gapless against real YouTube' }
$YtdlpPath = Find-Ytdlp
if (-not $YtdlpPath) {
  Add-Result 'FAIL' 'ytdlp-found' 'no yt-dlp.exe found (run without -SkipTools, or pass -Ytdlp <path>)'
  $SkipLive = $true
} else {
  Add-Result 'PASS' 'ytdlp-found' (Split-Path -Leaf $YtdlpPath)
  if ($UpdateYtdlp) { $r = Invoke-Logged 'ytdlp-update' "`"$YtdlpPath`" -U"; Add-Result 'INFO' 'ytdlp-update' ((Get-Content $r.Log -ErrorAction SilentlyContinue | Select-Object -Last 2) -join ' | ') }
  Add-Result 'INFO' 'ytdlp-version' (Get-FirstLine "`"$YtdlpPath`" --version")
  Test-JsRuntime
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
      if ($s[0] -eq 'ui-test') { Add-UiTestCount $r.Log }
    }
    Test-UiNamedTests
  }
  $r = Invoke-Logged 'ui-build' 'npm run build --prefix ui'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'ui-build' "$($r.Seconds)s"; Add-UiBundleReport } else { Add-Result 'FAIL' 'ui-build' "exit $($r.Code)"; Show-LogTail $r.Log 25 }
}

if (-not $SkipTests) {
  Section 'Rust build and tests (same steps as CI)'
  $before = Get-TripCount
  # The UI checks are part of the full run (-Ui is accepted but no longer needed); the build also gives Tauri its ui\dist.
  Invoke-UiSteps $true

  # The audio crate links a minimal static FFmpeg (built once from a pinned commit, LGPL only).
  if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    # Not in a VS Native Tools prompt: import the MSVC environment the way vcvars64.bat would.
    $vsw = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = if (Test-Path $vsw) { & $vsw -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath } else { $null }
    $vcv = if ($vs) { Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat' } else { $null }
    if ($vcv -and (Test-Path $vcv)) {
      cmd /c "`"$vcv`" >nul && set" | ForEach-Object { if ($_ -match '^([^=]+)=(.*)$') { Set-Item -Path "env:$($Matches[1])" -Value $Matches[2] } }
      Add-Result 'INFO' 'msvc-env' 'imported the Visual Studio build environment (vcvars64)'
    } else { Add-Result 'WARN' 'msvc-env' 'cl.exe not found: install the Visual Studio C++ build tools, or run from an x64 Native Tools prompt' }
  }
  $ffOk = $false
  $bashExe = if ($env:PYXEE_BASH) { $env:PYXEE_BASH } else { 'C:\msys64\usr\bin\bash.exe' }
  if (-not (Test-Path $bashExe)) {
    Add-Result 'FAIL' 'ffmpeg-prereq' ('MSYS2 bash not found at ' + $bashExe + '. One-time setup: winget install --id MSYS2.MSYS2 -e   then   C:\msys64\usr\bin\bash.exe -lc "pacman -S --noconfirm --needed make nasm diffutils pkgconf"   (or set PYXEE_BASH to your MSYS2 bash.exe), then run this script again.')
  } else {
    $r = Invoke-Logged 'ffmpeg-build' 'cargo xtask ffmpeg'
    if ($r.Code -eq 0) { $ffOk = $true; Add-Result 'PASS' 'ffmpeg-build' "minimal static FFmpeg ready ($($r.Seconds)s; unchanged builds are skipped next time)" }
    else { Add-Result 'FAIL' 'ffmpeg-build' "exit $($r.Code): see the log tail below (MSYS2 tools make/nasm/diffutils, the MSVC tools, or the FFmpeg configure/make output)"; Show-LogTail $r.Log 40 }
  }
  if (-not $ffOk) {
    $why = 'SKIPPED: the minimal FFmpeg libraries were not built (see ffmpeg-prereq / ffmpeg-build above), and every cargo step below needs them'
    if (-not $SkipLive) { $script:StepTotal-- }
    $SkipLive = $true
    if ($script:LiveRequested) { Add-Result 'FAIL' 'live-checks' ($why + ' (the live checks are part of the full run, so this is a failure, not a skip)') } else { Add-Result 'INFO' 'live-checks' $why }
  }

  $r = Invoke-Logged 'cargo-fmt' 'cargo fmt --all -- --check'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-fmt' 'formatting is clean' } else { Add-Result 'FAIL' 'cargo-fmt' "exit $($r.Code) (run: cargo fmt --all)"; Show-LogTail $r.Log 20 }

  if ($ffOk) {
  $r = Invoke-Logged 'cargo-clippy' 'cargo clippy --locked --workspace --all-targets -- -D warnings'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-clippy' "no warnings ($($r.Seconds)s)" }
  elseif ((Get-Content $r.Log -Raw) -match "no such command: .clippy") { Add-Result 'WARN' 'cargo-clippy' 'clippy is not installed (rustup component add clippy)' }
  else { Add-Result 'FAIL' 'cargo-clippy' "exit $($r.Code)"; Show-LogTail $r.Log 50 }

  # The tests regenerate the ts-rs bindings: compare their content before and after (not `git status`, which would
  # also flag a binding change that is simply not committed yet).
  $bindDir = Join-Path $Root 'ui\src\lib\bindings'
  $bindHash = { Get-ChildItem $bindDir -Filter '*.ts' -ErrorAction SilentlyContinue | Sort-Object Name | ForEach-Object { '{0}:{1}' -f $_.Name, (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash } }
  $bindBefore = @(& $bindHash)
  $r = Invoke-Logged 'cargo-test' 'cargo test --locked --workspace --no-fail-fast'
  $txt = Get-Content $r.Log -Raw
  $p = 0; $f = 0; $ig = 0
  foreach ($m in [regex]::Matches($txt, 'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored')) { $p += [int]$m.Groups[1].Value; $f += [int]$m.Groups[2].Value; $ig += [int]$m.Groups[3].Value }
  $failedNames = @([regex]::Matches($txt, '(?m)^test (\S+) \.\.\. FAILED') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
  if ($r.Code -eq 0 -and $f -eq 0) { Add-Result 'PASS' 'cargo-test' "$p passed, $f failed, $ig ignored in $($r.Seconds)s" }
  else {
    Add-Result 'FAIL' 'cargo-test' "$p passed, $f failed (exit $($r.Code)); failed: $($failedNames -join ', ')"
    if ($failedNames.Count -eq 0) { Show-LogTail $r.Log 40 }
    foreach ($n in ($failedNames | Select-Object -First 5)) {
      $re = Invoke-Logged ("rerun-" + ($n -replace '[^A-Za-z0-9]', '_')) "cargo test --locked --workspace -- $n --exact"
      if ($re.Code -eq 0) { Add-Result 'WARN' "rerun-$n" 'passed when re-run alone (timing flake?)' } else { Add-Result 'FAIL' "rerun-$n" 'failed again when re-run alone'; Show-LogTail $re.Log 25 }
    }
  }
  Add-Result 'INFO' 'windows-note' 'the fake-yt-dlp tests in pyxee-sources are unix-only; the live phase "fake429" covers the same cooldown logic on this PC'
  Test-EditedTests

  # Known timing-sensitive test (also flaky on the original code: 6 of 40 failures there).
  if ($FlakeRuns -gt 0) {
    $bad = 0
    $firstFailure = $null
    for ($i = 1; $i -le $FlakeRuns; $i++) {
      $re = Invoke-Logged "flake-$i" 'cargo test --locked -p pyxee-app pause_freezes_the_position_and_resume_continues' $Root "pause-test repeat $i of $FlakeRuns"
      if ($re.Code -ne 0) {
        $bad++
        # Keep the first failing assertion: it says whether the position moved or audio reached the device.
        if (-not $firstFailure -and (Test-Path $re.Log)) { $firstFailure = Select-String -Path $re.Log -Pattern 'panicked at' -Context 0,3 | Select-Object -First 1 }
      }
    }
    Add-Result 'INFO' 'known-flake-pause-test' "failed $bad of $FlakeRuns runs (original code before this work: about 6 of 40; not related to ffmpeg/yt-dlp)"
    if ($firstFailure) {
      $post = @($firstFailure.Context.PostContext | ForEach-Object { $_.Trim() } | Where-Object { $_ })
      Add-Result 'INFO' 'known-flake-first-failure' ($firstFailure.Line.Trim() + ' | ' + ($post -join ' | '))
    }
  }
  } else {
    Add-Result 'INFO' 'cargo-clippy' $why
    Add-Result 'INFO' 'cargo-test' $why
    Add-Result 'INFO' 'known-flake-pause-test' $why
  }

  Add-TripResult 'build-and-tests' $before

  $dirty = @(& git status --porcelain 2>$null | Where-Object { $_ -notmatch 'pyxee-check' })
  if ($ffOk) {
    $bindAfter = @(& $bindHash)
    $changed = @(Compare-Object $bindBefore $bindAfter | ForEach-Object { ($_.InputObject -split ':')[0] } | Sort-Object -Unique)
    if ($changed.Count -eq 0) { Add-Result 'PASS' 'bindings-up-to-date' 'the tests regenerated the ts-rs bindings and nothing changed' } else { Add-Result 'FAIL' 'bindings-up-to-date' ('the tests rewrote: ' + ($changed -join ', ') + ' (commit the regenerated files)') }
  } else { Add-Result 'INFO' 'bindings-up-to-date' 'SKIPPED: the tests that regenerate them did not run' }
  Add-Result 'INFO' 'git-status-after-tests' "$($dirty.Count) changed/untracked path(s) (the first lines are the files this work changed or added)"
}

# ---------------------------------------------------------------- 4. optional installer

# The signing config is only used with a certificate, but whether the installed Tauri CLI accepts its shape
# (bundle.windows.signCommand as {cmd, args}) is checked on every run, so a CLI upgrade cannot break it silently.
$tauriCli = Join-Path $Root 'ui\node_modules\.bin\tauri.cmd'
if (Test-Path $tauriCli) {
  $env:PYXEE_SIGN_THUMBPRINT = ('0123456789ABCDEF' * 3).Substring(0, 40)
  try {
    $r = Invoke-Logged 'signing-config' 'cargo xtask signing-config'
    $conf = (Get-Content $r.Log -ErrorAction SilentlyContinue | Where-Object { $_ -match 'tauri\.bundle\.signed\.conf\.json' } | Select-Object -Last 1)
    if ($r.Code -ne 0 -or -not $conf) { Add-Result 'FAIL' 'tauri-signcommand' 'cargo xtask signing-config produced no config' }
    else {
      # `tauri info` takes no --config; the CLI merges the TAURI_CONFIG environment variable into the config it loads.
      $env:TAURI_CONFIG = (Get-Content -Raw -LiteralPath $conf.Trim())
      try { $r2 = Invoke-Logged 'tauri-signcommand' ('"{0}" info' -f $tauriCli) (Join-Path $Root 'crates\app') } finally { Remove-Item Env:\TAURI_CONFIG -ErrorAction SilentlyContinue }
      $txt = (Get-Content $r2.Log -ErrorAction SilentlyContinue) -join "`n"
      if ($txt -match '(?i)signCommand|unknown field|invalid type|did not match any variant') { Add-Result 'FAIL' 'tauri-signcommand' 'the installed Tauri CLI rejected the signing config (see the log)'; Show-LogTail $r2.Log 15 }
      elseif ($r2.Code -eq 0) { Add-Result 'PASS' 'tauri-signcommand' 'the Tauri CLI accepts the signCommand {cmd, args} config' }
      else { Add-Result 'WARN' 'tauri-signcommand' "could not validate (tauri info exited $($r2.Code) for another reason)" }
    }
  } finally { Remove-Item Env:\PYXEE_SIGN_THUMBPRINT -ErrorAction SilentlyContinue }
}

if ($Installer) {
  Section 'Installer build (slow)'
  $before = Get-TripCount
  $r = Invoke-Logged 'xtask-installer' 'cargo xtask installer'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-xtask-installer' "built in $($r.Seconds)s" } else { Add-Result 'FAIL' 'cargo-xtask-installer' "exit $($r.Code)"; Show-LogTail $r.Log 30 }
  $rel = Join-Path $Root 'target\release'
  # An ffmpeg program or a libav DLL (libav is linked in statically). FFmpeg-LGPL-2.1.txt is meant to be there.
  $leak = @(Get-ChildItem $rel -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^(ffmpeg|ffprobe|ffplay)\.exe$|^(avcodec|avformat|avutil|swresample)(-\d+)?\.dll$' -and $_.FullName -notlike "*\deps\*" -and $_.FullName -notlike "*\build\*" })
  if ($leak.Count -eq 0) { Add-Result 'PASS' 'installer-has-no-ffmpeg' 'no ffmpeg program or libav DLL under target\release' } else { Add-Result 'FAIL' 'installer-has-no-ffmpeg' (($leak | ForEach-Object { $_.Name }) -join ', ') }
  $setup = @(Get-ChildItem (Join-Path $rel 'bundle') -Recurse -Include '*.exe', '*.msi' -ErrorAction SilentlyContinue)
  foreach ($s in $setup) { Add-Result 'INFO' 'installer-file' ("{0}  {1:N1} MB" -f $s.Name, ($s.Length / 1MB)) }
  Add-TripResult 'installer-build' $before
  # F8: the installer carries Microsoft's WebView2 bootstrapper in non-silent mode. Its progress window only shows on a PC
  # that has no WebView2 runtime yet, which cannot be arranged from here.
  Add-Result 'INFO' 'webview2-progress-manual' 'MANUAL: run the installer built above on a PC without the WebView2 runtime (a fresh Windows 10 VM): the Microsoft bootstrapper window must show download progress. The portable zip needs no change (the update happens in the browser).'

  # The portable zip: xtask itself refuses a zip without Pyxee.exe / yt-dlp.exe / portable.txt or with a data folder or
  # ffmpeg program in it; here the finished zip is opened again from the outside.
  $r = Invoke-Logged 'xtask-portable' 'cargo xtask portable'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-xtask-portable' "built in $($r.Seconds)s" } else { Add-Result 'FAIL' 'cargo-xtask-portable' "exit $($r.Code)"; Show-LogTail $r.Log 30 }
  $zip = Get-ChildItem (Join-Path $Root 'target\portable') -Filter '*_x64-portable.zip' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $zip) { Add-Result 'FAIL' 'portable-zip' 'no *_x64-portable.zip under target\portable' }
  else {
    Add-Result 'INFO' 'portable-file' ("{0}  {1:N1} MB" -f $zip.Name, ($zip.Length / 1MB))
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $za = [IO.Compression.ZipFile]::OpenRead($zip.FullName)
    try {
      $names = @($za.Entries | ForEach-Object { $_.FullName })
      foreach ($want in @('Pyxee.exe', 'yt-dlp.exe', 'portable.txt', 'README.txt', 'THIRD_PARTY_NOTICES.txt', 'FFmpeg-LGPL-2.1.txt')) {
        if (@($names | Where-Object { $_ -like "*/$want" }).Count -eq 1) { Add-Result 'PASS' "portable-has-$want" 'present' } else { Add-Result 'FAIL' "portable-has-$want" 'missing from the zip' }
      }
      $bad = @($names | Where-Object { $_ -match '(^|/)data(/|$)|(^|/)(ffmpeg|ffprobe|ffplay)\.exe$|(^|/)(avcodec|avformat|avutil|swresample)(-\d+)?\.dll$' })
      if ($bad.Count -eq 0) { Add-Result 'PASS' 'portable-clean' 'no data folder, no ffmpeg program, no libav DLL' } else { Add-Result 'FAIL' 'portable-clean' ($bad -join ', ') }
    } finally { $za.Dispose() }
    foreach ($f in @($setup | Select-Object -First 3)) { Add-Result 'INFO' 'installer-signature' ("{0}: {1} (NotSigned is expected without a certificate; see docs/releasing.md)" -f $f.Name, (Get-AuthenticodeSignature -LiteralPath $f.FullName).Status) }
  }
}

# ---------------------------------------------------------------- 4b. CI simulation

# release.yml's portable step runs on a clean runner: the FFmpeg libraries come from the cache (so the MSYS2 login that
# the FFmpeg step would have made never happens) and the first MSYS2 login therefore happens inside `cargo xtask portable`,
# where MSYS2 prints its first-run notice on stdout. A developer PC has long since passed that first login, which is why
# this only showed up in CI. Here a throwaway HOME makes MSYS2 meet it as new, for the child processes of this section only.
if ($Ci) {
  Section 'CI simulation (release workflow: cached FFmpeg, first MSYS2 login)'
  $before = Get-TripCount
  $ciBash = if ($env:PYXEE_BASH) { $env:PYXEE_BASH } else { 'C:\msys64\usr\bin\bash.exe' }
  $ciFfDir = if ($env:PYXEE_FFMPEG_DIR) { $env:PYXEE_FFMPEG_DIR } else { Join-Path $Root 'target\ffmpeg\x86_64-pc-windows-msvc' }
  $oldHome = $env:HOME
  $ciHomes = New-Object System.Collections.Generic.List[string]
  $newCiHome = { $h = Join-Path ([IO.Path]::GetTempPath()) ('pyxee-ci-home-' + [guid]::NewGuid().ToString('N').Substring(0, 8)); $ciHomes.Add($h); return $h }
  try {
    if (-not (Test-Path $ciBash)) {
      Add-Result 'FAIL' 'ci-msys2' ('MSYS2 bash not found at ' + $ciBash + ' (see ffmpeg-prereq above for the one-time setup)')
    } else {
      # The workflow's cache hit: the libraries exist before the portable step. Build them with the normal home if absent.
      if (-not (Test-Path (Join-Path $ciFfDir 'include\libavformat\avformat.h'))) {
        $r = Invoke-Logged 'ci-ffmpeg-build' 'cargo xtask ffmpeg'
        if ($r.Code -eq 0) { Add-Result 'PASS' 'ci-ffmpeg-build' "libraries built first, like a cache hit ($($r.Seconds)s)" } else { Add-Result 'FAIL' 'ci-ffmpeg-build' "exit $($r.Code)"; Show-LogTail $r.Log 40 }
      }

      # Prove the fresh-home trick works on this PC; if it does not, the portable run below would not be a faithful replay.
      $h = & $newCiHome
      $env:HOME = $h
      $r = Invoke-Logged 'ci-msys2-first-login' ('"{0}" -lc "echo pyxee-ready"' -f $ciBash)
      $loginTxt = Get-Content $r.Log -Raw -ErrorAction SilentlyContinue
      if ($loginTxt -match 'Copying skeleton files') { Add-Result 'PASS' 'ci-first-login-reproduced' 'a fresh HOME makes MSYS2 print its first-run notice, as on a clean runner' }
      else { Add-Result 'WARN' 'ci-first-login-reproduced' 'a fresh HOME did not trigger the MSYS2 first-run notice here, so the replay below is weaker than CI (MSYS2 ignored HOME?)' }
      Remove-Item $h -Recurse -Force -ErrorAction SilentlyContinue

      # A new fresh home for the real run: the probe above used up the first login of the previous one.
      $env:HOME = (& $newCiHome)
      $r = Invoke-Logged 'ci-xtask-portable' 'cargo xtask portable'
      if ($r.Code -eq 0) { Add-Result 'PASS' 'ci-cargo-xtask-portable' "built in $($r.Seconds)s on a first MSYS2 login" } else { Add-Result 'FAIL' 'ci-cargo-xtask-portable' "exit $($r.Code) (what release.yml reports as: cargo xtask portable failed)"; Show-LogTail $r.Log 30 }
    }
  } finally {
    if ($null -eq $oldHome) { Remove-Item Env:\HOME -ErrorAction SilentlyContinue } else { $env:HOME = $oldHome }
    foreach ($h in $ciHomes) { if (Test-Path $h) { Remove-Item $h -Recurse -Force -ErrorAction SilentlyContinue } }
  }

  # The rest of release.yml's step: unpack the zip and run what it carries.
  $ciZip = Get-ChildItem (Join-Path $Root 'target\portable') -Filter '*_x64-portable.zip' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $ciZip) { Add-Result 'FAIL' 'ci-portable-zip' 'the portable zip was not built' }
  else {
    $ciDir = Join-Path ([IO.Path]::GetTempPath()) ('pyxee-ci-portable-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    try {
      Expand-Archive -LiteralPath $ciZip.FullName -DestinationPath $ciDir -Force
      $ciRoot = Get-ChildItem $ciDir -Directory | Select-Object -First 1
      if (-not $ciRoot -or -not (Test-Path (Join-Path $ciRoot.FullName 'portable.txt'))) { Add-Result 'FAIL' 'ci-portable-marker' 'portable.txt is missing from the unpacked zip' }
      else { Add-Result 'PASS' 'ci-portable-marker' 'portable.txt is in the unpacked zip' }
      $ciYt = if ($ciRoot) { Join-Path $ciRoot.FullName 'yt-dlp.exe' } else { $null }
      if ($ciYt -and (Test-Path $ciYt)) {
        $ver = & $ciYt --version 2>&1 | Select-Object -First 1
        if ($LASTEXITCODE -eq 0) { Add-Result 'PASS' 'ci-portable-ytdlp-runs' "yt-dlp $ver" } else { Add-Result 'FAIL' 'ci-portable-ytdlp-runs' 'the portable yt-dlp does not run' }
      } else { Add-Result 'FAIL' 'ci-portable-ytdlp-runs' 'yt-dlp.exe is missing from the unpacked zip' }
    } finally { if (Test-Path $ciDir) { Remove-Item $ciDir -Recurse -Force -ErrorAction SilentlyContinue } }
  }
  Add-TripResult 'ci-simulation' $before
}

# ---------------------------------------------------------------- 5. live checks

function Read-CheckLines([string]$file, [string]$prefix = '') {
  if (-not (Test-Path $file)) { return }
  $logLines = 0
  foreach ($line in (Get-Content $file)) {
    if ($line -match '^CHECK (PASS|FAIL|WARN|INFO) (\S+) :: (.*)$') {
      $st = $Matches[1]; $nm = $prefix + $Matches[2]; $dt = $Matches[3]
      # The live phase repeats yt-dlp's own JS-runtime warning; ytdlp-js-runtime above already explained it once.
      if ($st -eq 'WARN' -and $script:JsRuntimeWarned -and $nm -like '*yt-dlp-warning' -and $dt -match 'JavaScript|\bdeno\b') {
        $st = 'INFO'; $dt = "$dt  (known: see ytdlp-js-runtime above)"
      }
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
  # The live-check tool links the same FFmpeg libraries; without them it cannot be built.
  $ffLib = if ($env:PYXEE_FFMPEG_DIR) { $env:PYXEE_FFMPEG_DIR } else { Join-Path $Root 'target\ffmpeg\x86_64-pc-windows-msvc' }
  if (-not (Test-Path (Join-Path $ffLib 'include\libavformat\avformat.h'))) {
    $SkipLive = $true
    if ($script:LiveRequested) { Add-Result 'FAIL' 'live-checks' 'NOT RUN: the minimal FFmpeg libraries are not built (run without -SkipTests, or `cargo xtask ffmpeg` first). The live checks are part of the full run, so this is a failure.' } else { Add-Result 'INFO' 'live-checks' 'skipped (-SkipLive)' }
  }
}
if (-not $SkipLive) {
  Section 'Live YouTube checks (real search / resolve / decode / seek / gapless)'
  Write-Report 'This sends roughly 25 yt-dlp requests, paced by the app itself. If YouTube starts rate limiting, the script stops the live part and says so.'
  $before = Get-TripCount

  Write-Report 'building the live-check tool (release build; the first time takes a few minutes)...'
  $r = Invoke-Logged 'build-livecheck' 'cargo build --locked --release -p pyxee-spike --bin livecheck'
  $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
  $Live = Join-Path $targetDir 'release\livecheck.exe'
  if ($r.Code -ne 0 -or -not (Test-Path $Live)) {
    Add-Result 'FAIL' 'build-livecheck' "exit $($r.Code)"; Show-LogTail $r.Log 30
  } else {
    Add-Result 'PASS' 'build-livecheck' "$($r.Seconds)s"
    $common = "--ytdlp `"$YtdlpPath`" --q1 `"$Query1`" --q2 `"$Query2`""

    function Invoke-Phase([string]$name, [string]$prefix = '') {
      $r = Invoke-Logged "live-$name$prefix" "`"$Live`" $name $common" $Root "live phase $name$prefix"
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
      if ($script:LiveBlocked) { Add-Result 'FAIL' "live-$ph" 'NOT RUN: YouTube is rate limiting this IP right now (try again in ~15 minutes, or use -CookieFile). A run that could not do its live checks is not a pass.'; continue }
      Invoke-Phase $ph
    }

    # Stress: 9 simultaneous calls. Watch from outside how many yt-dlp processes run at once.
    if ($script:LiveBlocked) { Add-Result 'FAIL' 'live-stress' 'NOT RUN: rate limited' }
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
  if ($script:LiveBlocked) { Add-Result 'FAIL' 'live-summary' 'YouTube rate limiting was hit, so the live proof is incomplete. That is the situation the guard handles; paste this report, and re-run in ~15 minutes or with -CookieFile.' }
}

# ---------------------------------------------------------------- 6. manual checks in the real app

if ($Manual) {
  Section 'Manual checks in the real app (about 25 minutes)'
  Write-Report 'Start the app in another window (see docs/development.md, Run)   (wait for it to open). Answer each question: y = yes/works, n = no, s = skip.'
  $items = @(
    @('app-search', "Search '$Query1' on the Songs filter: results appear within ~10 s?"),
    @('app-play', 'Click a result: sound starts within ~10 s and the progress bar moves?'),
    @('app-seek-middle', 'Drag the progress bar to the middle: sound continues from there within ~3 s?'),
    @('app-seek-end', 'Drag to ~10 s before the end: the next song (add 2 songs to the queue first) starts with no audible gap?'),
    @('app-next-hammer', 'Press Next 6 times quickly: you end up on the last song and it plays (no endless spinner, no error pile-up)?'),
    @('app-filters', 'Switch the search filter 6 times quickly: results show each time, no "rate limited" message?'),
    @('app-pause', 'Pause for 20 s, resume: it continues without a glitch?'),
    @('app-quality', 'Does the sound quality seem acceptable (it is ~128 kbps AAC now, was ~130-160 kbps Opus)?'),
    @('app-opus-skip', "Optional: a song that cannot be played shows a 'skipped' notice and the next one plays (skip if you never see one)?"),
    @('app-seek-after-pause', 'Play a song, pause it, start another song from the queue, then drag the progress bar: sound continues (not silent)?'),
    @('app-stalled-stream-next', 'Play a song, cut the network (unplug or turn Wi-Fi off) and press Next 5 times: the buttons keep responding and the window never freezes (turn the network back on afterwards)?'),
    @('app-clear-then-add', 'Clear the queue, then add a song: Undo no longer offers to bring the cleared list back over the new song?'),
    @('app-autoplay-failure', 'Optional: with Autoplay on, let the last song play out while the network is off: a notice says Autoplay could not load similar songs, and the bar does not show Playing with nothing audible (skip if you cannot try it)?'),
    @('app-update-progress', 'Settings > Updates: Check yt-dlp, then Update yt-dlp if one is offered (otherwise skip): a progress bar and text like "Downloading yt-dlp: 12.3 of 18.4 MB (67%)" show while it installs, then "Updated to ..."?'),
    @('app-no-discord-id-field', 'Settings > Advanced has no Discord application ID field, and (with the Discord desktop app running and the Discord option on) "Listening to Pyxee" still shows your song in Discord?'),
    @('ui-wide', 'UI: maximise the window (1280 px or wider). Is there a rail on the left with the icons AND their names, the search box on top of the page, the player (cover, controls, Up next) on the right, and nothing cut off or overlapping?'),
    @('ui-medium', 'UI: make the window about 1000 px wide. Does the rail show icons only (a tooltip on each), does the player stay on the right, and is the song list still easy to read?'),
    @('ui-narrow', 'UI: make the window about 400 px wide. Are the rail and the player column gone, a one-line player (dock) above the bottom bar, Settings at the top right, and does tapping the dock open Now Playing?'),
    @('ui-smallest', 'UI: make the window as small as it goes (360 x 560). Is there no sideways scroll bar and nothing important cut off, on Home and on a song list?'),
    @('ui-views', 'UI: open Home, Explore, Library (all four tabs), Queue, a playlist and a search. Does every view keep its title at the top while the list scrolls, and do the column names (#, Title, Artist, Time) line up with the rows?'),
    @('ui-playing-mark', 'UI: play a song and look at every list it is in. Does its row show moving bars and an accent-coloured title, and do the bars stand still when you pause?'),
    @('ui-stage', 'UI: in the wide layout, do the player on the right (seek, previous / play / next, like, volume) and its Up next preview work, does clicking an Up next song play it, and does Open queue open the Queue?'),
    @('ui-now-playing', 'UI: open Now Playing (click the cover). At a normal window height do the cover, title, seek bar and buttons fit without scrolling, do the Up next / Lyrics / Related tabs work, and does Close take you back?'),
    @('ui-dark-light', 'UI: in Settings > Appearance switch between a dark and a light theme (and try Match system). Is all text readable, the chosen place in the rail, the playing row, the buttons and the switches all clearly visible in both?'),
    @('ui-mica', 'UI: on Windows 11 does the backdrop show through the rail and the page edge, while menus, sheets, the search suggestions and Now Playing are NOT see-through (on Windows 10: everything solid)?'),
    @('ui-custom-theme', 'UI: Settings > Customize: create a theme, save it, then delete it. Does the whole app (rail, page, player, sheets) follow its colours while it is selected?'),
    @('ui-settings', 'UI: in Settings, does every switch work with the mouse and with Space, can you tell on from off by the position of the knob and not only by colour, and do all five sections show?'),
    @('ui-keyboard-only', 'UI: using only the keyboard, can you reach the rail, the search box, a song row (its like and menu buttons appear when focused), the player buttons and open a sheet (focus stays inside, Esc closes it and focus comes back), with a visible focus ring at every stop? Do Space, arrows, N and P still work?'),
    @('ui-mini-flyout', 'UI: do the mini player and the tray flyout show cover, title, thin progress line and previous / play / next without cutting anything off, and do their buttons work?'),
    @('ui-high-dpi', 'UI: at 150% and at 200% display scaling (Settings > System > Display), is the text sharp, are the thin lines and icons crisp, and is nothing blurry or cut off?'),
    @('ui-reduced-motion', 'UI: turn off Windows Settings > Accessibility > Visual effects > Animation effects. Do sheets and Now Playing now appear without sliding, and do the playing bars stand still?'),
    @('ui-system-font', 'UI: does the text look like Windows 11 system text (Segoe UI Variable) everywhere, with no serif fallback and no empty boxes where an icon should be?')
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
  $logFile = Join-Path $env:LOCALAPPDATA 'dev.pyxee.app\logs\pyxee.log'
  if (-not (Test-Path $logFile)) { $logFile = Join-Path $env:APPDATA 'dev.pyxee.app\logs\pyxee.log' }  # versions up to 0.0.2, or a failed move
  $localLib = Join-Path $env:LOCALAPPDATA 'dev.pyxee.app\library.db'
  $roamLib = Join-Path $env:APPDATA 'dev.pyxee.app\library.db'
  if (Test-Path $localLib) { Add-Result 'PASS' 'data-folder' 'library.db is in %LOCALAPPDATA%\dev.pyxee.app' }
  elseif (Test-Path $roamLib) { Add-Result 'WARN' 'data-folder' 'library.db is still only in %APPDATA% (Roaming): the move did not happen; see the "data folder:" lines in the log' }
  else { Add-Result 'INFO' 'data-folder' 'no library.db yet (start the app once)' }
  if (-not (Test-Path $logFile)) { Add-Result 'WARN' 'app-log' 'log file not found (start the app once, or PYXEE_LOG may point elsewhere)' }
  else {
    $lines = @(Get-Content $logFile -Tail 4000)
    Add-Result 'INFO' 'app-log' "last $($lines.Count) lines of pyxee.log"
    $count = { param($re) @($lines | Where-Object { $_ -match $re }).Count }
    $fmt = @($lines | Where-Object { $_ -match 'resolved .*ext=(\S+) acodec=(\S+)' } | ForEach-Object { if ($_ -match 'ext=(\S+) acodec=(\S+)') { "$($Matches[1])/$($Matches[2])" } } | Group-Object | Sort-Object Count -Descending)
    if ($fmt.Count -gt 0) {
      Add-Result 'INFO' 'app-log-formats-played' (($fmt | ForEach-Object { "$($_.Name) x$($_.Count)" }) -join ', ')
      # The log prints the codec as Rust Debug, e.g. webm/Some("opus"), hence the `.*`.
      $odd = @($fmt | Where-Object { $_.Name -notmatch '^(m4a/.*mp4a\.40\.2|webm/.*opus)' })
      if ($odd.Count -eq 0) { Add-Result 'PASS' 'app-log-only-supported-formats' 'every resolved stream was Opus in WebM or AAC-LC in M4A' } else { Add-Result 'FAIL' 'app-log-only-supported-formats' (($odd | ForEach-Object { $_.Name }) -join ', ') }
    } else { Add-Result 'INFO' 'app-log-formats-played' 'no "resolved ... ext= acodec=" lines yet (play a few songs first)' }
    Add-Result 'INFO' 'app-log-gate-waits' ("{0} waits logged by the yt-dlp gate" -f (& $count 'yt-dlp .* waited'))
    Add-Result 'INFO' 'app-log-cooldowns' ("{0} rate-limit cooldown start(s), {1} refusal(s)" -f (& $count 'was rate limited'), (& $count 'refused: rate-limit'))
    $nff = & $count '(?i)ffmpeg'
    if ($nff -eq 0) { Add-Result 'PASS' 'app-log-no-ffmpeg' 'no ffmpeg lines' } else { Add-Result 'FAIL' 'app-log-no-ffmpeg' "$nff line(s) mention ffmpeg" }
    $cant = @($lines | Where-Object { $_ -match 'cannot decode this stream|seeking to .* failed|could not start|panicked|ERROR' })
    if ($cant.Count -eq 0) { Add-Result 'PASS' 'app-log-no-decode-errors' 'no decode/seek/spawn errors or panics' }
    else { Add-Result 'WARN' 'app-log-notable-lines' "$($cant.Count) notable line(s); first 15 below"; $cant | Select-Object -First 15 | ForEach-Object { Write-Report ("    " + $_.Substring(0, [math]::Min($_.Length, 220))) } }
    # F1: nothing that names the user, the cookie file or the browser profile may reach the log. Lines written by an older
    # version of the app can still be in the file, so a hit is a warning to look at, not proof of a bug.
    $homeDir = [string]$env:USERPROFILE
    if ($homeDir.Length -ge 4) {
      $homeRe = [regex]::Escape($homeDir.TrimEnd('\', '/')).Replace('\\', '[\\/]')
      $leak = @($lines | Where-Object { $_ -match "(?i)$homeRe" })
      if ($leak.Count -eq 0) { Add-Result 'PASS' 'app-log-no-profile-paths' 'no line contains the user profile folder' }
      else { Add-Result 'WARN' 'app-log-no-profile-paths' "$($leak.Count) line(s) contain the user profile folder (clear the log, use the app, check again: older lines may predate the fix)" }
    }
    $ckArgs = @($lines | Where-Object { $_ -match '--cookies(-from-browser)?\s+(?!<redacted>)\S' })
    if ($ckArgs.Count -eq 0) { Add-Result 'PASS' 'app-log-no-cookie-arguments' 'no --cookies / --cookies-from-browser value in the log' } else { Add-Result 'FAIL' 'app-log-no-cookie-arguments' "$($ckArgs.Count) line(s) show a cookie file or browser profile" }
    $ck = @($lines | Where-Object { $_ -match 'cookie' } | Select-Object -Last 2)
    foreach ($c in $ck) { Write-Report ("    " + $c.Substring(0, [math]::Min($c.Length, 200))) }
  }
}

# ---------------------------------------------------------------- cleanup (large build artifacts)

function Get-DirMB([string]$p) { if (-not (Test-Path $p)) { return 0 }; [math]::Round((((Get-ChildItem $p -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum) / 1MB), 0) }
Section 'cleanup'
if ($KeepBuild) { Add-Result 'INFO' 'cleanup' 'skipped (-KeepBuild): target\debug and the FFmpeg build tree were kept' }
else {
  $freed = 0
  $victims = @((Join-Path $Root 'target\debug')) + @(Get-ChildItem (Join-Path $Root 'target\ffmpeg') -Directory -Filter '*.build' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
  foreach ($v in $victims) {
    $mb = Get-DirMB $v
    if (Test-Path $v) { Remove-Item $v -Recurse -Force -ErrorAction SilentlyContinue }
    if (Test-Path $v) { Add-Result 'WARN' 'cleanup' "could not fully delete $($v.Replace($Root, '.')) (antivirus or an open handle?)" } else { $freed += $mb }
  }
  Add-Result 'INFO' 'cleanup' "deleted target\debug and the FFmpeg build tree, about $freed MB (kept: target\ffmpeg\<triple> libraries, logs, report)"
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
