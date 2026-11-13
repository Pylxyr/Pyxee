<#
.SYNOPSIS
  Pyxee Windows check: builds, runs every test, proves no ffmpeg process is ever started, and runs live YouTube
  checks of the real search/resolve/decode/seek/gapless path. Writes one pasteable report.

.DESCRIPTION
  Run it from anywhere:   scripts\check-windows.cmd        (or:  powershell -File scripts\check-windows.ps1)
  Needs: Rust (rustup, with rustfmt + clippy), the Flutter SDK, git, internet, plus MSYS2 (make, nasm, diffutils,
  pkgconf) and the Visual Studio C++ build tools to build the minimal static FFmpeg libraries the decoder links
  in-process (`cargo xtask ffmpeg`, built once). No ffmpeg.exe is used or wanted: the tripwire below proves it.
  Output: pyxee-check\report-<time>.txt  (paste its contents back). Full logs: pyxee-check\logs\.
  The report never contains stream URLs (only hosts), cookie paths/values, or your user name.

.PARAMETER Ytdlp          Path to yt-dlp.exe (default: PYXEE_YTDLP, tools, then PATH).
.PARAMETER Query1/2       Search queries used for the two live tracks (default: two CC-licensed songs).
.PARAMETER CookieFile     Netscape cookies.txt to use (sets PYXEE_COOKIES for this run only).
.PARAMETER CookieBrowser  e.g. firefox (sets PYXEE_COOKIES_FROM_BROWSER for this run only).
.PARAMETER SkipTests      Skip fmt/clippy/cargo test (only do static checks + live checks).
.PARAMETER SkipLive       NOT RECOMMENDED. Skip everything that talks to YouTube. Without it the live checks always run, and
                          a live phase that could not run (no FFmpeg libraries, no yt-dlp, rate limited) is a FAIL.
.PARAMETER SkipTools      Do not run `cargo xtask tools` (the yt-dlp download flow).
.PARAMETER Ui             Kept for old command lines. The Flutter checks (pub get, analyze, test when there are tests, a release build of the app) always run in a full run.
.PARAMETER SelfTest       Only run the redaction self-test (it also runs first in every full run) and exit.
.PARAMETER Installer      Also build the installer and the portable zip (`cargo xtask installer` / `portable`, slow), inspect them, start the staged app (`cargo xtask smoke`) and run the installer silently into a temp folder and uninstall it again. Needs NSIS (makensis).
.PARAMETER Ci            Replay the release workflow's build the way a clean GitHub runner meets it: the bridge and the Flutter app built in
                          steps of their own, then `cargo xtask installer` and `portable` with PYXEE_BUILD_READY=1, then unpack the zip and
                          start it, install the installer silently, start that, and uninstall it. Slow; catches bugs that only show on a clean machine.
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
Write-Report ("flutter   : {0}" -f (Get-FirstLine 'flutter --version'))
Write-Report ("cookies   : {0}" -f $cookieKind)
$envset = @('PYXEE_YTDLP', 'PYXEE_JS_RUNTIME', 'PYXEE_COOKIES', 'PYXEE_COOKIES_FROM_BROWSER', 'PYXEE_LOG', 'CARGO_TARGET_DIR') | ForEach-Object { $v = [Environment]::GetEnvironmentVariable($_); if ($v) { "$_=set" } }
Write-Report ("env       : {0}" -f $(if ($envset) { $envset -join ', ' } else { '(none of the PYXEE_* / CARGO_TARGET_DIR variables are set)' }))
Write-Report ("options   : SkipTests=$SkipTests SkipLive=$SkipLive SkipTools=$SkipTools Flutter=$true Installer=$Installer Ci=$Ci Manual=$Manual AnalyzeLog=$AnalyzeLog")

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
# pyxee-audio and pyxee-bridge ones can only run on Windows: their crates need the FFmpeg libraries.
function Test-EditedTests {
  $groups = @(
    @{ Crate = 'pyxee-sources'; Names = @(
      'the_cookie_file_path_is_replaced_in_either_slash_style_and_case',
      'a_browser_profile_is_replaced',
      'user_profile_folders_become_their_variable_names',
      'the_value_of_a_cookies_argument_is_replaced',
      'errors_are_classified_for_the_ui',
      'a_403_or_a_5xx_from_youtube_is_transient_and_a_missing_video_is_not',
      'empty_queries_search_nothing_and_non_youtube_keys_are_refused',
      'genres_are_the_buttons_that_open_a_genre_page_each_once_in_the_pages_order',
      'a_genres_page_without_genre_buttons_is_a_parse_error',
      'a_genre_page_gives_playlist_and_song_shelves_and_no_albums',
      'a_genre_page_with_an_unexpected_layout_is_a_parse_error',
      'a_playlist_gives_its_playable_songs_in_order_in_either_page_layout',
      'a_playlist_is_cut_at_its_first_page_and_a_page_without_a_list_is_an_error',
      'genre_and_playlist_requests_name_the_page_and_refuse_an_odd_id',
      'genres_genre_and_playlists_ask_youtube_music_directly_and_need_music_search_on') },
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
    @{ Crate = 'pyxee-bridge'; Names = @(
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

# ---------------------------------------------------------------- 1. static: ffmpeg is gone from the repo

Section 'no ffmpeg process in code, build and packaging (libav is linked in-process)'
$bin = Join-Path $Root 'tools'
$ff = @(Get-ChildItem $bin -Filter 'ffmpeg*' -ErrorAction SilentlyContinue)
if ($ff.Count -eq 0) { Add-Result 'PASS' 'no-ffmpeg-sidecar-file' 'tools has no ffmpeg*' } else { Add-Result 'FAIL' 'no-ffmpeg-sidecar-file' ($ff.Name -join ', ') }
$code = @(& git grep -n -i -E 'FfmpegPipe|ffmpeg\.exe|PYXEE_FFMPEG_(PATH|BIN|EXE)|Command::new\(.*ffmpeg|externalBin.*ffmpeg|AudioError::Ffmpeg' -- '*.rs' '*.toml' '*.json' '*.cmd' '*.yml' ':!scripts' ':!xtask/src/release.rs' 2>$null)
if ($code.Count -eq 0) { Add-Result 'PASS' 'no-ffmpeg-in-code' 'no ffmpeg process/sidecar/config references in .rs .toml .json .cmd .yml' }
else { Add-Result 'FAIL' 'no-ffmpeg-in-code' "$($code.Count) reference(s)"; $code | Select-Object -First 10 | ForEach-Object { Write-Report "    $_" } }
$any = @(& git grep -n -i ffmpeg -- . ':!Cargo.lock' ':!scripts' 2>$null)
Add-Result 'INFO' 'ffmpeg-mentions-left' "$($any.Count) line(s) mention the word (expected: the in-process libav build scripts, docs and comments, plus the fixture-provenance note)"
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

# yt-dlp wants an external JavaScript runtime for YouTube (its EJS solver scripts are bundled in yt-dlp.exe, so only the
# runtime is missing). Pyxee starts yt-dlp with --ignore-config, so a --js-runtimes line in a yt-dlp config file is never
# read by Pyxee and does not count here. What counts is what Pyxee passes itself: PYXEE_JS_RUNTIME if set, otherwise
# deno, then node, found on PATH (crates/sources/src/jsruntime.rs). Pyxee never starts a runtime itself, so a missing one
# is a WARN with the fix, never a FAIL.
function Test-JsRuntime {
  $script:JsRuntimeWarned = $false
  $override = $env:PYXEE_JS_RUNTIME
  if ($override -and $override.Trim()) {
    $o = $override.Trim()
    if ($o -match '^(none|off)$') { Add-Result 'INFO' 'ytdlp-js-runtime' 'PYXEE_JS_RUNTIME=none: Pyxee passes no --js-runtimes (yt-dlp decides)'; return }
    $name = ($o -split ':', 2)[0].Trim().ToLowerInvariant()
    $path = if ($o.Contains(':')) { ($o -split ':', 2)[1].Trim() } else { '' }
    if (@('deno', 'node', 'quickjs', 'bun') -notcontains $name) {
      Add-Result 'WARN' 'ytdlp-js-runtime' "PYXEE_JS_RUNTIME=$o is not understood (use deno, node, quickjs or bun, optionally :path, or none); Pyxee ignores it and looks on PATH"
    } elseif ($path -and -not (Test-Path -LiteralPath $path)) {
      Add-Result 'WARN' 'ytdlp-js-runtime' "PYXEE_JS_RUNTIME points to a path that does not exist ($path); Pyxee ignores it and looks on PATH"
    } else {
      Add-Result 'PASS' 'ytdlp-js-runtime' "PYXEE_JS_RUNTIME=$name is passed to yt-dlp as --js-runtimes"; return
    }
  }
  $deno = Get-Command deno -ErrorAction SilentlyContinue
  if ($deno) {
    $v = Get-FirstLine 'deno --version'
    if ($v -match 'deno\s+(\d+)\.') {
      if ([int]$Matches[1] -ge 2) { Add-Result 'PASS' 'ytdlp-js-runtime' "$v on PATH (Pyxee passes --js-runtimes deno)"; return }
      Add-Result 'WARN' 'ytdlp-js-runtime' "$v is too old: yt-dlp needs Deno 2.0 or newer (winget upgrade DenoLand.Deno)"
      $script:JsRuntimeWarned = $true
      return
    }
  }
  if (Have 'node') {
    Add-Result 'PASS' 'ytdlp-js-runtime' "$(Get-FirstLine 'node --version') on PATH (Pyxee passes --js-runtimes node; no Deno found)"
    return
  }
  $script:JsRuntimeWarned = $true
  Add-Result 'WARN' 'ytdlp-js-runtime' 'no JavaScript runtime for yt-dlp. YouTube extraction without one is deprecated by yt-dlp (some formats may vanish later); Pyxee works for now. Fix: winget install DenoLand.Deno (or install Node), then open a new terminal and restart Pyxee. A --js-runtimes line in a yt-dlp config file does not help: Pyxee runs yt-dlp with --ignore-config. See README, "yt-dlp and a JavaScript runtime".'
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

# The Flutter app (ui_flutter) needs the Rust bridge library built first: the native-assets hook copies it in.
function Invoke-FlutterSteps {
  if (-not (Have 'flutter')) { Add-Result 'FAIL' 'flutter' 'flutter is not on PATH (install the Flutter SDK, see docs/development.md)'; return }
  $r = Invoke-Logged 'bridge-build' 'cargo build --locked -p pyxee-bridge --release'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'bridge-build' "$($r.Seconds)s" } else { Add-Result 'FAIL' 'bridge-build' "exit $($r.Code)"; Show-LogTail $r.Log 30; return }
  $ui = Join-Path $Root 'ui_flutter'
  $steps = @(@('flutter-pub-get', 'flutter pub get'), @('flutter-analyze', 'flutter analyze'))
  if (Test-Path (Join-Path $ui 'test')) { $steps += , @('flutter-test', 'flutter test') }
  $steps += , @('flutter-build', 'flutter build windows --release')
  foreach ($st in $steps) {
    $r = Invoke-Logged $st[0] $st[1] $ui
    if ($r.Code -eq 0) { Add-Result 'PASS' $st[0] "$($r.Seconds)s" } else { Add-Result 'FAIL' $st[0] "exit $($r.Code)"; Show-LogTail $r.Log 30; if ($st[0] -eq 'flutter-pub-get') { return } }
  }
}

if (-not $SkipTests) {
  Section 'Rust build and tests (same steps as CI)'
  $before = Get-TripCount
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
  Invoke-FlutterSteps

  # Known timing-sensitive test (also flaky on the original code: 6 of 40 failures there).
  if ($FlakeRuns -gt 0) {
    $bad = 0
    $firstFailure = $null
    for ($i = 1; $i -le $FlakeRuns; $i++) {
      $re = Invoke-Logged "flake-$i" 'cargo test --locked -p pyxee-bridge pause_freezes_the_position_and_resume_continues' $Root "pause-test repeat $i of $FlakeRuns"
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
  Add-Result 'INFO' 'git-status-after-tests' "$($dirty.Count) changed/untracked path(s) (the first lines are the files this work changed or added)"
}

# ---------------------------------------------------------------- 4. optional installer

# Shared by the installer section and the CI replay: what an app folder must and must not hold, and the proof that
# it starts. `cargo xtask smoke <folder>` runs yt-dlp --version, then starts Pyxee.exe from a portable copy of the
# folder for a few seconds and fails if it stopped by itself.
function Test-AppFolder([string]$dir, [string]$label) {
  foreach ($f in @('Pyxee.exe', 'yt-dlp.exe', 'flutter_windows.dll', 'pyxee_bridge.dll', 'data\icudtl.dat', 'data\app.so', 'THIRD_PARTY_NOTICES.txt', 'FFmpeg-LGPL-2.1.txt', 'GPL-3.0.txt', 'yt-dlp-THIRD_PARTY_LICENSES.txt')) {
    if (Test-Path (Join-Path $dir $f)) { Add-Result 'PASS' "$label-has-$f" 'present' } else { Add-Result 'FAIL' "$label-has-$f" 'missing' }
  }
  $leak = @(Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^(ffmpeg|ffprobe|ffplay)\.exe$|^(avcodec|avformat|avutil|swresample)(-\d+)?\.dll$' })
  if ($leak.Count -eq 0) { Add-Result 'PASS' "$label-has-no-ffmpeg" 'no ffmpeg program or libav DLL (libav is linked in statically)' } else { Add-Result 'FAIL' "$label-has-no-ffmpeg" (($leak | ForEach-Object { $_.Name }) -join ', ') }
  $r = Invoke-Logged "$label-smoke" ('cargo xtask smoke "{0}"' -f $dir)
  if ($r.Code -eq 0) { Add-Result 'PASS' "$label-starts" 'yt-dlp runs and Pyxee.exe stays up' } else { Add-Result 'FAIL' "$label-starts" "exit $($r.Code)"; Show-LogTail $r.Log 20 }
}

# The portable zip, opened again from the outside, then unpacked and started.
function Test-PortableZip {
  $zip = Get-ChildItem (Join-Path $Root 'target\portable') -Filter '*_x64-portable.zip' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $zip) { Add-Result 'FAIL' 'portable-zip' 'no *_x64-portable.zip under target\portable'; return }
  Add-Result 'INFO' 'portable-file' ("{0}  {1:N1} MB" -f $zip.Name, ($zip.Length / 1MB))
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $za = [IO.Compression.ZipFile]::OpenRead($zip.FullName)
  try {
    $names = @($za.Entries | ForEach-Object { $_.FullName })
    foreach ($want in @('Pyxee.exe', 'yt-dlp.exe', 'portable.txt', 'README.txt', 'pyxee_bridge.dll', 'flutter_windows.dll', 'THIRD_PARTY_NOTICES.txt', 'FFmpeg-LGPL-2.1.txt')) {
      if (@($names | Where-Object { $_ -like "*/$want" }).Count -eq 1) { Add-Result 'PASS' "portable-has-$want" 'present' } else { Add-Result 'FAIL' "portable-has-$want" 'missing from the zip' }
    }
    # Flutter's own data\ folder (app.so, icudtl.dat, flutter_assets) is meant to be there; the user's files never are.
    $bad = @($names | Where-Object { $_ -match '(^|/)data/(library\.db|shell\.json|session\.json|ui_prefs\.json|\.env|logs/|tools/)|(^|/)(ffmpeg|ffprobe|ffplay)\.exe$|(^|/)(avcodec|avformat|avutil|swresample)(-\d+)?\.dll$' })
    if ($bad.Count -eq 0) { Add-Result 'PASS' 'portable-clean' 'no user data, no ffmpeg program, no libav DLL' } else { Add-Result 'FAIL' 'portable-clean' ($bad -join ', ') }
  } finally { $za.Dispose() }
  $dir = Join-Path ([IO.Path]::GetTempPath()) ('pyxee-portable-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
  try {
    Expand-Archive -LiteralPath $zip.FullName -DestinationPath $dir -Force
    $root = Get-ChildItem $dir -Directory | Select-Object -First 1
    if ($root) { Test-AppFolder $root.FullName 'portable' } else { Add-Result 'FAIL' 'portable-unpack' 'the zip has no top folder' }
  } finally { if (Test-Path $dir) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue } }
}

# The installer, run for real and silently into a temp folder, started, and uninstalled again: proves the installed layout
# and that the uninstaller removes what the installer wrote (and nothing else).
function Test-InstalledCopy {
  $setup = Get-ChildItem (Join-Path $Root 'target\installer') -Filter '*-setup.exe' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $setup) { Add-Result 'FAIL' 'installer-file' 'no *-setup.exe under target\installer'; return }
  Add-Result 'INFO' 'installer-file' ("{0}  {1:N1} MB" -f $setup.Name, ($setup.Length / 1MB))
  Add-Result 'INFO' 'installer-signature' ("{0}: {1} (NotSigned is expected without a certificate; see docs/releasing.md)" -f $setup.Name, (Get-AuthenticodeSignature -LiteralPath $setup.FullName).Status)
  $dir = Join-Path ([IO.Path]::GetTempPath()) ('pyxee-installed-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
  try {
    $p = Start-Process -FilePath $setup.FullName -ArgumentList '/S', "/D=$dir" -Wait -PassThru
    if ($p.ExitCode -ne 0) { Add-Result 'FAIL' 'installer-silent-install' "exit $($p.ExitCode)"; return }
    Add-Result 'PASS' 'installer-silent-install' 'installed into a temp folder'
    if (Test-Path (Join-Path $dir 'portable.txt')) { Add-Result 'FAIL' 'installed-has-no-portable-marker' 'portable.txt is in the installed app: it would turn portable mode on' } else { Add-Result 'PASS' 'installed-has-no-portable-marker' 'absent' }
    Test-AppFolder $dir 'installed'
    $u = Start-Process -FilePath (Join-Path $dir 'uninstall.exe') -ArgumentList '/S', "_?=$dir" -Wait -PassThru
    $left = @('Pyxee.exe', 'yt-dlp.exe', 'pyxee_bridge.dll', 'data\app.so') | Where-Object { Test-Path (Join-Path $dir $_) }
    if ($u.ExitCode -eq 0 -and $left.Count -eq 0) { Add-Result 'PASS' 'installer-uninstall' 'the uninstaller removed the installed files' } else { Add-Result 'FAIL' 'installer-uninstall' "exit $($u.ExitCode); left behind: $($left -join ', ')" }
  } finally { if (Test-Path $dir) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue } }
}

if ($Installer) {
  Section 'Installer build (slow)'
  $before = Get-TripCount
  $r = Invoke-Logged 'xtask-installer' 'cargo xtask installer'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-xtask-installer' "built in $($r.Seconds)s" } else { Add-Result 'FAIL' 'cargo-xtask-installer' "exit $($r.Code) (NSIS missing? set PYXEE_MAKENSIS)"; Show-LogTail $r.Log 30 }
  # xtask itself refuses a build without Pyxee.exe, yt-dlp.exe, the Flutter files and notices, or with an ffmpeg program,
  # a libav DLL or user data in it; here the results are opened again from the outside.
  Test-InstalledCopy
  $r = Invoke-Logged 'xtask-portable' 'cargo xtask portable'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'cargo-xtask-portable' "built in $($r.Seconds)s" } else { Add-Result 'FAIL' 'cargo-xtask-portable' "exit $($r.Code)"; Show-LogTail $r.Log 30 }
  Test-PortableZip
  Add-TripResult 'installer-build' $before
}

# ---------------------------------------------------------------- 4b. CI simulation

# release.yml builds on a clean runner, in steps: the Rust bridge, the Flutter app, then `cargo xtask installer` and
# `cargo xtask portable` with PYXEE_BUILD_READY=1 (they only gather, package and sign), then it unpacks and starts both
# results. Here the same steps run in the same order, so a mistake in that order or in what the packaging steps assume
# shows on this PC and not only in a release.
if ($Ci) {
  Section 'CI simulation (release workflow: separate build steps, then package with PYXEE_BUILD_READY)'
  $before = Get-TripCount
  $r = Invoke-Logged 'ci-bridge-build' 'cargo build -p pyxee-bridge --release --locked'
  if ($r.Code -eq 0) { Add-Result 'PASS' 'ci-bridge-build' "$($r.Seconds)s" } else { Add-Result 'FAIL' 'ci-bridge-build' "exit $($r.Code)"; Show-LogTail $r.Log 30 }
  $r = Invoke-Logged 'ci-flutter-build' 'flutter build windows --release' (Join-Path $Root 'ui_flutter')
  if ($r.Code -eq 0) { Add-Result 'PASS' 'ci-flutter-build' "$($r.Seconds)s" } else { Add-Result 'FAIL' 'ci-flutter-build' "exit $($r.Code)"; Show-LogTail $r.Log 30 }
  $env:PYXEE_BUILD_READY = '1'
  try {
    $r = Invoke-Logged 'ci-xtask-installer' 'cargo xtask installer'
    if ($r.Code -eq 0) { Add-Result 'PASS' 'ci-cargo-xtask-installer' "built in $($r.Seconds)s" } else { Add-Result 'FAIL' 'ci-cargo-xtask-installer' "exit $($r.Code) (what release.yml reports as: cargo xtask installer failed)"; Show-LogTail $r.Log 40 }
    $r = Invoke-Logged 'ci-xtask-portable' 'cargo xtask portable'
    if ($r.Code -eq 0) { Add-Result 'PASS' 'ci-cargo-xtask-portable' "built in $($r.Seconds)s" } else { Add-Result 'FAIL' 'ci-cargo-xtask-portable' "exit $($r.Code) (what release.yml reports as: cargo xtask portable failed)"; Show-LogTail $r.Log 40 }
  } finally { Remove-Item Env:\PYXEE_BUILD_READY -ErrorAction SilentlyContinue }
  Test-PortableZip
  Test-InstalledCopy
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

    foreach ($ph in @('search', 'genres', 'stream')) {
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
    @('app-no-discord-id-field', 'Settings > Advanced has no Discord application ID field, and (with the Discord desktop app running, the Discord option on, and a build that has an application ID) "Listening to Pyxee" shows your song in Discord?'),
    @('ui-wide', 'UI: maximise the window (1440 px or wider). Is the queue lane beside the page, the player deck at the bottom, and nothing cut off or overlapping?'),
    @('ui-medium', 'UI: make the window about 1000 px wide. Is the player deck at the bottom with no queue lane, and is the song list still easy to read?'),
    @('ui-narrow', 'UI: make the window about 600 px wide. Is there a dock plus a bottom bar instead of the deck, and does tapping the dock open Now Playing?'),
    @('ui-smallest', 'UI: make the window as small as it goes. Is there no sideways scroll bar and nothing important cut off, on Home and on a song list?'),
    @('ui-views', 'UI: open Home, Explore, Library, Queue, a playlist and a search. Does every page load, and do the column names line up with the rows?'),
    @('ui-playing-mark', 'UI: play a song and look at every list it is in. Is the playing row marked, and does the mark stand still when you pause?'),
    @('ui-now-playing', 'UI: open Now Playing (click the cover). Do the cover, title, seek bar and buttons fit, do the Up next / Lyrics / Related tabs work, and does Close take you back?'),
    @('ui-dark-light', 'UI: in Settings switch between a dark and a light theme (and Match system). Is all text readable and every control clearly visible in both?'),
    @('ui-custom-theme', 'UI: Settings > theme editor: create a theme, save it, then delete it. Does the whole app follow its colours while it is selected?'),
    @('ui-keyboard-only', 'UI: using only the keyboard, can you reach the navigation, the search box, a song row, the player buttons and open a sheet (Esc closes it), with a visible focus mark everywhere?'),
    @('ui-tray', 'Tray: with close to tray on, does Close hide the window while the music keeps playing, and does the tray icon bring it back and offer previous / play / next?'),
    @('ui-high-dpi', 'UI: at 150% and at 200% display scaling (Settings > System > Display), is the text sharp and nothing blurry or cut off?'),
    @('ui-system-font', 'UI: does the text look like Windows 11 system text (Segoe UI Variable) everywhere, with no empty boxes where an icon should be?')
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
