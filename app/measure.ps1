param(
    [ValidateSet('pill', 'tray-only', 'panel')]
    [string]$Mode = 'pill',
    [ValidateSet('none', 'five', 'five-snoozed')]
    [string]$Sessions = 'none',
    [int]$WarmupSeconds = 60,
    [int]$SampleSeconds = 60,
    [int]$IntervalSeconds = 5,
    [string]$Exe
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class BeaconTray
{
    delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

    [DllImport("user32.dll")]
    static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll")]
    static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetClassName(IntPtr hwnd, StringBuilder name, int capacity);

    [DllImport("user32.dll")]
    static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);

    public static bool PostCommand(int processId, int commandId)
    {
        IntPtr tray = IntPtr.Zero;
        EnumWindows((hwnd, unused) =>
        {
            uint owner;
            GetWindowThreadProcessId(hwnd, out owner);
            var name = new StringBuilder(64);
            GetClassName(hwnd, name, name.Capacity);
            if (owner == processId && name.ToString() == "tray_icon_app")
            {
                tray = hwnd;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return tray != IntPtr.Zero && PostMessage(tray, 0x0111, (IntPtr)commandId, IntPtr.Zero);
    }
}
'@

$trayCommandIds = @{ snooze5 = 1002; quit = 1005 }
$modeArguments = @{ 'pill' = @(); 'tray-only' = @('--tray-only'); 'panel' = @('--open-panel') }
$sceneB = @(
    @{ Id = 'measure-1'; Project = 'atlas'; State = 'working' },
    @{ Id = 'measure-2'; Project = 'beacon'; State = 'needs_permission' },
    @{ Id = 'measure-3'; Project = 'cobalt'; State = 'done' },
    @{ Id = 'measure-4'; Project = 'delta'; State = 'error' },
    @{ Id = 'measure-5'; Project = 'ember'; State = 'idle' }
)

function Get-BeaconTree([int]$AppId) {
    $processes = Get-CimInstance Win32_Process
    $tree = @($processes | Where-Object { $_.ProcessId -eq $AppId } | ForEach-Object {
            [pscustomobject]@{ Id = [int]$_.ProcessId; Role = 'app' }
        })
    $parents = @($AppId)
    while ($parents.Count -gt 0) {
        $children = @($processes | Where-Object { $parents -contains $_.ParentProcessId })
        foreach ($child in $children | Where-Object { $_.Name -eq 'msedgewebview2.exe' }) {
            $role = 'webview browser'
            if ($child.CommandLine -match '--type=(\S+)') { $role = "webview $($Matches[1])" }
            if ($child.CommandLine -match '--utility-sub-type=(\S+)') { $role = "$role $($Matches[1])" }
            $tree += [pscustomobject]@{ Id = [int]$child.ProcessId; Role = $role }
        }
        $parents = @($children | ForEach-Object { [int]$_.ProcessId })
    }
    $tree
}

function Wait-Until([Diagnostics.Stopwatch]$Clock, [double]$Seconds) {
    $remaining = $Seconds - $Clock.Elapsed.TotalSeconds
    if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int]($remaining * 1000)) }
}

function Send-TrayCommand([int]$AppId, [string]$Command) {
    if (-not [BeaconTray]::PostCommand($AppId, $trayCommandIds[$Command])) {
        throw "Could not post '$Command' to the Beacon tray window of process $AppId"
    }
}

function Invoke-WithBeaconEnvironment([scriptblock]$Action) {
    $saved = @{ BEACON_HOME = $env:BEACON_HOME; PATH = $env:PATH; CLAUDE_CODE_SESSION_ATTENDED = $env:CLAUDE_CODE_SESSION_ATTENDED; CLAUDE_CODE_ENTRYPOINT = $env:CLAUDE_CODE_ENTRYPOINT }
    $env:BEACON_HOME = $beaconHome
    $env:PATH = $cleanPath
    $env:CLAUDE_CODE_SESSION_ATTENDED = '1'
    $env:CLAUDE_CODE_ENTRYPOINT = 'cli'
    try { & $Action }
    finally { foreach ($name in $saved.Keys) { Set-Item -Path "env:$name" -Value $saved[$name] -ErrorAction SilentlyContinue; if ($null -eq $saved[$name]) { Remove-Item -Path "env:$name" -ErrorAction SilentlyContinue } } }
}

function Invoke-Hook([string]$HookEvent, [hashtable]$Fields) {
    $payload = Get-Content (Join-Path $fixtures "$HookEvent.json") -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json
    if (-not $payload) { $payload = [pscustomobject]@{ hook_event_name = $HookEvent } }
    foreach ($name in $Fields.Keys) { $payload | Add-Member -NotePropertyName $name -NotePropertyValue $Fields[$name] -Force }
    $file = Join-Path $beaconHome "payload-$([guid]::NewGuid().ToString('N')).json"
    [IO.File]::WriteAllText($file, ($payload | ConvertTo-Json -Depth 10))
    $helper = Invoke-WithBeaconEnvironment { Start-Process -FilePath $hook -ArgumentList $HookEvent -RedirectStandardInput $file -RedirectStandardOutput "$file.out" -WindowStyle Hidden -PassThru }
    if ($HookEvent -eq 'PermissionRequest') { $script:helpers += $helper }
    elseif (-not $helper.WaitForExit(5000)) { throw "beacon-hook $HookEvent did not finish" }
}

function Add-Session([hashtable]$Session) {
    $cwd = Join-Path $beaconHome "work\$($Session.Project)"
    New-Item -ItemType Directory -Force -Path $cwd | Out-Null
    $common = @{ session_id = $Session.Id; cwd = $cwd; transcript_path = (Join-Path $beaconHome "transcripts\$($Session.Id).jsonl") }
    $source = 'startup'
    if ($Session.State -eq 'idle') { $source = 'clear' }
    Invoke-Hook 'SessionStart' ($common + @{ source = $source })
    if ($Session.State -eq 'idle') { return }
    Invoke-Hook 'UserPromptSubmit' $common
    Invoke-Hook 'PreToolUse' ($common + @{ tool_use_id = "toolu_$($Session.Id)" })
    switch ($Session.State) {
        'needs_permission' { Invoke-Hook 'PermissionRequest' $common }
        'done' { Invoke-Hook 'Stop' $common }
        'error' { Invoke-Hook 'StopFailure' ($common + @{ error = 'rate_limit' }) }
    }
}

if (Get-Process beacon-app -ErrorAction SilentlyContinue) {
    throw 'beacon-app.exe is already running; its WebView2 processes would be shared with this run'
}

$repo = Join-Path $PSScriptRoot '..'
if (-not $Exe) { $Exe = Join-Path $repo 'target\release\beacon-app.exe' }
$Exe = (Resolve-Path $Exe).Path
$hook = (Resolve-Path (Join-Path $repo 'target\release\beacon-hook.exe')).Path
$fixtures = (Resolve-Path (Join-Path $repo 'fixtures\hook-payloads')).Path
$userBin = Join-Path $env:USERPROFILE '.local\bin'
$cleanPath = (($env:PATH -split ';') | Where-Object { $_ -and $_.TrimEnd('\') -ne $userBin }) -join ';'
$beaconHome = Join-Path $env:TEMP "beacon-measure-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $beaconHome | Out-Null
$log = Join-Path $beaconHome 'app-stderr.log'
$script:helpers = @()
$arguments = $modeArguments[$Mode]
$app = Invoke-WithBeaconEnvironment {
    if ($arguments.Count -gt 0) { Start-Process -FilePath $Exe -ArgumentList $arguments -RedirectStandardError $log -PassThru }
    else { Start-Process -FilePath $Exe -RedirectStandardError $log -PassThru }
}
$clock = [Diagnostics.Stopwatch]::StartNew()
$seenIds = @{}

try {
    Write-Output "Started $Exe $($arguments -join ' ') (pid $($app.Id)) with BEACON_HOME $beaconHome"
    $deadline = (Get-Date).AddSeconds(15)
    while (-not (Test-Path (Join-Path $beaconHome 'endpoint')) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100 }
    if ($Sessions -ne 'none') {
        foreach ($session in $sceneB) { Add-Session $session }
        Write-Output "Drove $($sceneB.Count) sessions into scene b at $([math]::Round($clock.Elapsed.TotalSeconds, 1)) s"
    }
    if ($Sessions -eq 'five-snoozed') {
        Start-Sleep -Seconds 1
        Send-TrayCommand $app.Id 'snooze5'
        Write-Output 'Snoozed all sessions for 5 minutes from the tray menu'
    }

    Wait-Until $clock $WarmupSeconds
    $stats = [ordered]@{}
    $totals = @()
    $sampleCount = [int]($SampleSeconds / $IntervalSeconds) + 1
    for ($sample = 0; $sample -lt $sampleCount; $sample++) {
        Wait-Until $clock ($WarmupSeconds + $sample * $IntervalSeconds)
        $workingSet = 0L
        $private = 0L
        $count = 0
        foreach ($member in Get-BeaconTree $app.Id) {
            $process = Get-Process -Id $member.Id -ErrorAction SilentlyContinue
            if (-not $process) { continue }
            $seenIds[$member.Id] = $true
            if (-not $stats.Contains("$($member.Id)")) {
                $stats["$($member.Id)"] = [pscustomobject]@{
                    Id = $member.Id; Role = $member.Role; FirstCpu = $process.TotalProcessorTime
                    LastCpu = $process.TotalProcessorTime; WorkingSets = @(); Privates = @()
                }
            }
            $entry = $stats["$($member.Id)"]
            $entry.LastCpu = $process.TotalProcessorTime
            $entry.WorkingSets += $process.WorkingSet64
            $entry.Privates += $process.PrivateMemorySize64
            $workingSet += $process.WorkingSet64
            $private += $process.PrivateMemorySize64
            $count++
        }
        $totals += [pscustomobject]@{ WorkingSet = $workingSet; Private = $private; Count = $count }
    }

    $megabytes = { param($values, $kind) [math]::Round((($values | Measure-Object -Average -Maximum).$kind) / 1MB, 1) }
    $rows = @($stats.Values | ForEach-Object {
            [pscustomobject]@{
                Pid = $_.Id; Role = $_.Role
                'WS avg MB' = & $megabytes $_.WorkingSets 'Average'; 'WS max MB' = & $megabytes $_.WorkingSets 'Maximum'
                'Private avg MB' = & $megabytes $_.Privates 'Average'; 'Private max MB' = & $megabytes $_.Privates 'Maximum'
                'CPU s' = [math]::Round(($_.LastCpu - $_.FirstCpu).TotalSeconds, 3); Samples = $_.WorkingSets.Count
            }
        })
    $counts = $totals | Measure-Object -Property Count -Minimum -Maximum
    $rows += [pscustomobject]@{
        Pid = 'total'; Role = "$($counts.Minimum)-$($counts.Maximum) processes"
        'WS avg MB' = & $megabytes $totals.WorkingSet 'Average'; 'WS max MB' = & $megabytes $totals.WorkingSet 'Maximum'
        'Private avg MB' = & $megabytes $totals.Private 'Average'; 'Private max MB' = & $megabytes $totals.Private 'Maximum'
        'CPU s' = [math]::Round(($rows | Measure-Object -Property 'CPU s' -Sum).Sum, 3); Samples = $totals.Count
    }
    Write-Output "Mode $Mode, sessions $Sessions; sampled every $IntervalSeconds s for $SampleSeconds s after $WarmupSeconds s"
    $rows | Format-Table -AutoSize | Out-String -Width 200 | Write-Output
}
finally {
    Get-BeaconTree $app.Id | ForEach-Object { $seenIds[$_.Id] = $true }
    if (-not $app.HasExited) {
        Send-TrayCommand $app.Id 'quit'
        if (-not $app.WaitForExit(5000)) { Stop-Process -Id $app.Id -Force }
    }
    foreach ($helper in $script:helpers) { if (-not $helper.HasExited) { Stop-Process -Id $helper.Id -Force -ErrorAction SilentlyContinue } }
    $deadline = (Get-Date).AddSeconds(10)
    do {
        $remaining = @($seenIds.Keys | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -in 'beacon-app', 'msedgewebview2' } })
        if ($remaining.Count -gt 0) { Start-Sleep -Milliseconds 200 }
    } while ($remaining.Count -gt 0 -and (Get-Date) -lt $deadline)
    if ($remaining.Count -gt 0) {
        Write-Output "Force-stopping $($remaining.Count) leftover processes: $($remaining -join ', ')"
        $remaining | ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }
    }
    Write-Output "Endpoint file left behind: $(Test-Path (Join-Path $beaconHome 'endpoint'))"
    $errors = @(Get-Content $log -ErrorAction SilentlyContinue)
    if ($errors.Count -gt 0) { Write-Output 'App stderr:'; $errors | Write-Output }
    Remove-Item -Recurse -Force $beaconHome -ErrorAction SilentlyContinue
}
