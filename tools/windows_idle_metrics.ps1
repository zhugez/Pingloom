# Read-only idle sample; never sends keyboard/mouse input or changes the target process.
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$Out
)
$ErrorActionPreference = 'Stop'
$process = Get-Process -Id $ProcessId
$startTime = $process.StartTime
$cpuBefore = $process.TotalProcessorTime.TotalMilliseconds
$watch = [System.Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds 10
$process = Get-Process -Id $ProcessId
if ($process.StartTime -ne $startTime) { throw 'Idle sample process identity changed' }
$elapsed = $watch.Elapsed.TotalSeconds
$metrics = [ordered]@{
    idle_cpu_ms_per_10s = ($process.TotalProcessorTime.TotalMilliseconds - $cpuBefore) * 10 / $elapsed
    idle_ws_mib = $process.WorkingSet64 / 1MB
    idle_private_mib = $process.PrivateMemorySize64 / 1MB
}
foreach ($value in $metrics.Values) {
    if ([double]::IsNaN($value) -or [double]::IsInfinity($value) -or $value -lt 0) { throw 'Invalid idle metric' }
}
# UTF-8 without a BOM: Python reads the same artifact on all CI platforms.
[System.IO.File]::WriteAllText($Out, ($metrics | ConvertTo-Json), (New-Object System.Text.UTF8Encoding($false)))
Write-Output ($metrics | ConvertTo-Json -Compress)
