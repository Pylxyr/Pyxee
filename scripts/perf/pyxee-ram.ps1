# Samples the memory of Pyxee and its WebView2 processes, by kind of process, with the state of Pyxee's window
# (visible / minimized / hidden), so you can see (1) what holds the memory while the window is in view and (2) how many
# seconds the memory takes to fall after you minimize it.
#
#   powershell -ExecutionPolicy Bypass -File D:\Pyxee\scripts\perf\pyxee-ram.ps1
#   powershell -ExecutionPolicy Bypass -File D:\Pyxee\scripts\perf\pyxee-ram.ps1 -Interval 0.5
#
# Play the song, leave the window in view for a minute or two, minimize it for a minute, restore it, repeat. Ctrl+C
# prints a summary per state and kind. Every sample is written to the CSV as it is taken (one row per process), in a new
# file named with the start time, so nothing is lost if you stop it any way at all. Do not open that file in Excel while
# it runs: if it is locked, the script carries on in a second file and says so.
#
# "PrivateWS" is the private working set: what Task Manager's Memory column shows. "WorkingSet" also counts pages shared
# with other processes. All numbers are in MB.
param(
  [double]$Interval = 1,
  [string]$Csv = (Join-Path $env:USERPROFILE ('Desktop\pyxee-ram-{0:yyyyMMdd-HHmmss}.csv' -f (Get-Date)))
)

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class PyxeeWin {
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  // The state of the top-level window titled "Pyxee" that belongs to process `pid` (not the mini player or the flyout).
  public static string State(uint pid) {
    bool vis = false, icon = false;
    EnumWindows(delegate (IntPtr h, IntPtr l) {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid) return true;
      StringBuilder t = new StringBuilder(256); GetWindowText(h, t, 256);
      if (t.ToString() != "Pyxee") return true;
      if (IsIconic(h)) icon = true; else if (IsWindowVisible(h)) vis = true;
      return true;
    }, IntPtr.Zero);
    return icon ? "minimized" : vis ? "visible" : "hidden";
  }
}
'@

function Get-State {
  $p = Get-Process pyxee-app -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $p) { return $null }
  return [PyxeeWin]::State([uint32]$p.Id)
}

# What kind of process a WebView2 process is, from its command line.
function Get-Kind([string]$cmd) {
  if ($cmd -notmatch '--type=(\S+)') { return 'manager (browser)' }
  $t = $Matches[1]
  if ($t -eq 'utility' -and $cmd -match '--utility-sub-type=(\S+)') { return 'utility: ' + ($Matches[1] -replace '"', '') }
  if ($t -eq 'renderer' -and $cmd -match '--extension-process') { return 'renderer (extension)' }
  return $t -replace '"', ''
}

$cores = [Environment]::ProcessorCount
$rows = New-Object System.Collections.ArrayList
$lastState = $null; $stateSince = Get-Date
Write-Host "Sampling every $Interval s. Ctrl+C for the summary. CSV: $Csv"
try {
  while ($true) {
    $t0 = Get-Date
    $state = Get-State
    if (-not $state) { Write-Host 'Pyxee is not running.'; Start-Sleep 2; continue }
    if ($state -ne $lastState) { $lastState = $state; $stateSince = $t0; Write-Host ("--- window is now {0} ---" -f $state) -ForegroundColor Yellow }
    $procs = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe' OR Name='pyxee-app.exe'" |
      Where-Object { $_.Name -eq 'pyxee-app.exe' -or $_.CommandLine -like '*dev.pyxee.app*' })
    $perf = @{}
    Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "Name LIKE 'msedgewebview2%' OR Name LIKE 'pyxee-app%'" |
      ForEach-Object { $perf[[int]$_.IDProcess] = $_ }
    $tick = @{}
    $batch = New-Object System.Collections.ArrayList
    foreach ($p in $procs) {
      $pp = $perf[[int]$p.ProcessId]
      if (-not $pp) { continue }
      $kind = if ($p.Name -eq 'pyxee-app.exe') { 'pyxee-app (Rust)' } else { Get-Kind $p.CommandLine }
      $row = [pscustomobject]@{
        Time = $t0.ToString('s'); SecInState = [math]::Round(($t0 - $stateSince).TotalSeconds, 1); State = $state
        Kind = $kind; Pid = [int]$p.ProcessId
        PrivateWS = [math]::Round($pp.WorkingSetPrivate / 1MB, 1); WorkingSet = [math]::Round($pp.WorkingSet / 1MB, 1)
        PrivateBytes = [math]::Round($pp.PrivateBytes / 1MB, 1)
      }
      [void]$rows.Add($row); [void]$batch.Add($row); $tick[$kind] += $row.PrivateWS
    }
    try { $batch | Export-Csv $Csv -Append -NoTypeInformation -ErrorAction Stop }
    catch {
      $Csv = $Csv -replace '\.csv$', ('-b{0}.csv' -f (Get-Random -Maximum 1000))
      Write-Host "Could not write the CSV (open in another program?). Carrying on in $Csv" -ForegroundColor Red
      try { $batch | Export-Csv $Csv -Append -NoTypeInformation -ErrorAction Stop } catch { }
    }
    $total = 0; foreach ($v in $tick.Values) { $total += $v }
    $detail = ($tick.GetEnumerator() | Sort-Object Value -Descending | ForEach-Object { '{0} {1:N0}' -f $_.Key, $_.Value }) -join ' | '
    Write-Host ("{0} {1,-9} +{2,5:N1}s  total {3,6:N0} MB  [{4}]" -f $t0.ToString('HH:mm:ss'), $state, ($t0 - $stateSince).TotalSeconds, $total, $detail)
    $left = $Interval - ((Get-Date) - $t0).TotalSeconds
    if ($left -gt 0) { Start-Sleep -Milliseconds ([int]($left * 1000)) }
  }
}
finally {
  if ($rows.Count -gt 0) {
    Write-Host "`nSamples are in $Csv"
    Write-Host "`nPer process kind and window state: average and peak private working set, MB (per process, summed per sample):"
    $rows | Group-Object State, Time, Kind | ForEach-Object {
      [pscustomobject]@{ State = $_.Group[0].State; Kind = $_.Group[0].Kind; Sum = ($_.Group | Measure-Object PrivateWS -Sum).Sum }
    } | Group-Object State, Kind | ForEach-Object {
      $g = $_.Group
      [pscustomobject]@{ State = $g[0].State; Kind = $g[0].Kind; Samples = $g.Count
        Avg = [math]::Round(($g | Measure-Object Sum -Average).Average, 1); Peak = [math]::Round(($g | Measure-Object Sum -Maximum).Maximum, 1) }
    } | Sort-Object State, @{ Expression = 'Avg'; Descending = $true } | Format-Table -AutoSize
    Write-Host 'Seconds to drop after minimizing: look at the first rows after "window is now minimized" (+N s column).'
  }
}
