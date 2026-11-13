<#
.SYNOPSIS
  Measures Claude Buddy against the performance budget in docs/scope.md
  (idle CPU < 1 %, RAM < 80 MB) and prints a table.

.DESCRIPTION
  Starts the release build with its own bridge.json and config.json in a
  temporary folder (CLAUDE_BUDDY_BRIDGE_FILE), so it never touches a real
  installation. Then it plays five Claude Code sessions through the real
  bridge, like buddy-hook would, and samples the app's whole process tree
  (the app plus its WebView2 processes):

    idle       five buddies on screen, no events
    animating  tool events every 400 ms, so buddies sit down, type, read
               and stand up all the time

  CPU is the share of the whole machine, as Task Manager shows it. RAM is
  the private working set (Task Manager's "Memory" column), summed over the
  tree. The worst sample of each phase is reported.

  Needs the release executable: `pnpm tauri build --no-bundle`.
  Windows only. Close any running Claude Buddy first: a second instance
  hands over to the first one and exits.

.EXAMPLE
  ./scripts/perf-budget.ps1
  ./scripts/perf-budget.ps1 -Seconds 60 -Strict
#>
[CmdletBinding()]
param(
  # The release executable.
  [string]$Exe = (Join-Path $PSScriptRoot '..\target\release\claude-buddy.exe'),
  # Length of each measured phase.
  [int]$Seconds = 30,
  # Exit with code 1 when a budget is exceeded (off by default: the CI job
  # only reports).
  [switch]$Strict
)

$ErrorActionPreference = 'Stop'

$BudgetCpuPercent = 1.0
$BudgetRamMB = 80
$Sessions = 5

if (-not (Test-Path $Exe)) {
  throw "Release executable not found: $Exe. Build it with: pnpm tauri build --no-bundle"
}
$Exe = (Resolve-Path $Exe).Path

# --- Process tree sampling ----------------------------------------------------

function Get-Tree([int]$RootId) {
  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId
  $children = @{}
  foreach ($p in $all) {
    $parent = [int]$p.ParentProcessId
    if (-not $children.ContainsKey($parent)) { $children[$parent] = @() }
    $children[$parent] += [int]$p.ProcessId
  }
  $tree = @($RootId)
  $queue = New-Object System.Collections.Queue
  $queue.Enqueue($RootId)
  while ($queue.Count -gt 0) {
    $id = $queue.Dequeue()
    if ($children.ContainsKey($id)) {
      foreach ($child in $children[$id]) {
        if ($tree -notcontains $child) { $tree += $child; $queue.Enqueue($child) }
      }
    }
  }
  return $tree
}

function Get-CpuTimes([int[]]$Ids) {
  $times = @{}
  foreach ($p in (Get-Process -Id $Ids -ErrorAction SilentlyContinue)) {
    $times[$p.Id] = $p.TotalProcessorTime.TotalMilliseconds
  }
  return $times
}

function Get-PrivateMB([int[]]$Ids) {
  $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Property IDProcess, WorkingSetPrivate
  $bytes = 0
  foreach ($p in $perf) {
    if ($Ids -contains [int]$p.IDProcess) { $bytes += [double]$p.WorkingSetPrivate }
  }
  return $bytes / 1MB
}

# --- Bridge client ------------------------------------------------------------

$script:Bridge = $null

function Send-Event([hashtable]$Body) {
  $Body.v = 1
  $Body.at = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
  $json = $Body | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:$($script:Bridge.port)/v1/events" `
    -Headers @{ Authorization = "Bearer $($script:Bridge.token)" } `
    -ContentType 'application/json' -Body $json | Out-Null
}

function Session-Id([int]$Index) { "00000000-0000-4000-8000-00000000000$Index" }

# --- Phases -------------------------------------------------------------------

function Measure-Phase([string]$Name, [int]$RootId, [scriptblock]$Tick) {
  $cores = [Environment]::ProcessorCount
  $tree = Get-Tree $RootId
  $start = Get-CpuTimes $tree
  $clock = [Diagnostics.Stopwatch]::StartNew()
  $last = $clock.Elapsed
  $lastTimes = $start
  $peakCpu = 0.0
  $peakRam = 0.0
  $processes = 0
  while ($clock.Elapsed.TotalSeconds -lt $Seconds) {
    if ($Tick) { & $Tick }
    Start-Sleep -Milliseconds 1000
    $tree = Get-Tree $RootId
    $now = $clock.Elapsed
    $times = Get-CpuTimes $tree
    $used = 0.0
    foreach ($id in $times.Keys) {
      $before = 0.0
      if ($lastTimes.ContainsKey($id)) { $before = $lastTimes[$id] }
      $used += $times[$id] - $before
    }
    $wall = ($now - $last).TotalMilliseconds
    $cpu = 100.0 * $used / ($wall * $cores)
    if ($cpu -gt $peakCpu) { $peakCpu = $cpu }
    $ram = Get-PrivateMB $tree
    if ($ram -gt $peakRam) { $peakRam = $ram }
    if ($tree.Count -gt $processes) { $processes = $tree.Count }
    $last = $now
    $lastTimes = $times
  }
  $end = Get-CpuTimes $tree
  $total = 0.0
  foreach ($id in $end.Keys) {
    $before = 0.0
    if ($start.ContainsKey($id)) { $before = $start[$id] }
    $total += $end[$id] - $before
  }
  $avgCpu = 100.0 * $total / ($clock.Elapsed.TotalMilliseconds * $cores)
  [pscustomobject]@{
    Phase      = $Name
    AvgCpu     = [math]::Round($avgCpu, 2)
    PeakCpu    = [math]::Round($peakCpu, 2)
    PeakRamMB  = [math]::Round($peakRam, 1)
    Processes  = $processes
  }
}

# --- Run ----------------------------------------------------------------------

$dir = Join-Path ([IO.Path]::GetTempPath()) "claude-buddy-perf-$PID"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$bridgeFile = Join-Path $dir 'bridge.json'
$env:CLAUDE_BUDDY_BRIDGE_FILE = $bridgeFile

$app = Start-Process -FilePath $Exe -PassThru
try {
  $deadline = (Get-Date).AddSeconds(30)
  while (-not (Test-Path $bridgeFile)) {
    if ($app.HasExited) {
      throw "Claude Buddy exited at startup (code $($app.ExitCode)). Is another instance running? Quit it first."
    }
    if ((Get-Date) -gt $deadline) { throw "No bridge.json after 30 s." }
    Start-Sleep -Milliseconds 250
  }
  $script:Bridge = Get-Content $bridgeFile -Raw | ConvertFrom-Json

  # Five sessions: spawn, then settle into idle.
  for ($i = 1; $i -le $Sessions; $i++) {
    Send-Event @{ event = 'SessionStart'; session_id = (Session-Id $i); cwd = "C:\perf\project-$i"; start_source = 'startup' }
  }
  Start-Sleep -Seconds 10

  $results = @()
  $results += Measure-Phase 'idle (5 buddies, no events)' $app.Id $null

  $tools = @('Edit', 'Read', 'Grep', 'Bash', 'WebSearch')
  $script:step = 0
  $tick = {
    # Two rounds of events per sampling second.
    for ($k = 0; $k -lt 2; $k++) {
      $i = ($script:step % $Sessions) + 1
      $tool = $tools[$script:step % $tools.Count]
      Send-Event @{ event = 'PreToolUse'; session_id = (Session-Id $i); cwd = "C:\perf\project-$i"; tool_name = $tool; target = 'file.ts' }
      if ($script:step % 3 -eq 0) {
        Send-Event @{ event = 'PostToolUse'; session_id = (Session-Id $i); cwd = "C:\perf\project-$i"; tool_name = $tool }
        Send-Event @{ event = 'Stop'; session_id = (Session-Id $i); cwd = "C:\perf\project-$i" }
      }
      $script:step++
      Start-Sleep -Milliseconds 400
    }
  }
  $results += Measure-Phase 'animating (tool events)' $app.Id $tick
}
finally {
  if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
  Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
}

# --- Report -------------------------------------------------------------------

$idle = $results[0]
$peakRam = ($results | Measure-Object PeakRamMB -Maximum).Maximum
$cpuOk = $idle.AvgCpu -lt $BudgetCpuPercent
$ramOk = $peakRam -lt $BudgetRamMB
$verdict = { param($ok) if ($ok) { 'within budget' } else { 'OVER BUDGET' } }

$cpuName = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
$machine = "$([Environment]::ProcessorCount) logical CPUs, $cpuName"

$lines = @()
$lines += '### Claude Buddy performance budget'
$lines += ''
$lines += "Machine: $machine. $Seconds s per phase, CPU as a share of the whole machine, RAM = private working set of the app and its WebView2 processes."
$lines += ''
$lines += '| Phase | Avg CPU % | Peak CPU % (1 s) | Peak RAM (MB) | Processes |'
$lines += '| --- | ---: | ---: | ---: | ---: |'
foreach ($r in $results) {
  $lines += "| $($r.Phase) | $($r.AvgCpu) | $($r.PeakCpu) | $($r.PeakRamMB) | $($r.Processes) |"
}
$lines += ''
$lines += "- Idle CPU: $($idle.AvgCpu) % (budget < $BudgetCpuPercent %): $(& $verdict $cpuOk)"
$lines += "- RAM: $peakRam MB peak (budget < $BudgetRamMB MB): $(& $verdict $ramOk)"

$report = $lines -join "`n"
Write-Output $report
if ($env:GITHUB_STEP_SUMMARY) { Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value $report }

if ($Strict -and -not ($cpuOk -and $ramOk)) { exit 1 }
