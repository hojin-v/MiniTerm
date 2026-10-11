param([Parameter(Mandatory = $true)][int]$ProcessId, [ValidateRange(10,60)][int]$Seconds = 30, [string]$OutputPath = 'artifacts/idle.json')
$ErrorActionPreference = 'Stop'
$taskProcess = Get-Process -Id $ProcessId
$taskExecutable = $taskProcess.Path
$taskStartCpu = $taskProcess.TotalProcessorTime.TotalSeconds
$taskWatch = [System.Diagnostics.Stopwatch]::StartNew()
$taskSamples = @()
for ($taskIndex = 0; $taskIndex -lt $Seconds; $taskIndex++) {
    Start-Sleep -Seconds 1
    $taskProcess.Refresh()
    if ($taskProcess.HasExited) { throw '앱이 측정 도중 종료되었습니다. 충분한 실행 시간을 확보한 뒤 다시 측정하세요.' }
    $taskSamples += [pscustomobject]@{ workingSetBytes = $taskProcess.WorkingSet64; privateBytes = $taskProcess.PrivateMemorySize64 }
}
$taskWatch.Stop()
$taskCpuSeconds = $taskProcess.TotalProcessorTime.TotalSeconds - $taskStartCpu
if ($taskCpuSeconds -lt 0) { throw '프로세스가 종료되어 CPU 측정이 유효하지 않습니다.' }
$taskResult = [pscustomobject]@{
    processId = $ProcessId
    executable = $taskExecutable
    timestamp = [DateTime]::UtcNow.ToString('o')
    durationSeconds = $taskWatch.Elapsed.TotalSeconds
    logicalProcessors = [Environment]::ProcessorCount
    cpuPercent = [Math]::Round(100 * $taskCpuSeconds / $taskWatch.Elapsed.TotalSeconds / [Environment]::ProcessorCount,3)
    cpuSingleCorePercent = [Math]::Round(100 * $taskCpuSeconds / $taskWatch.Elapsed.TotalSeconds,3)
    averageWorkingSetMiB = [Math]::Round(($taskSamples | Measure-Object workingSetBytes -Average).Average / 1MB,2)
    maximumWorkingSetMiB = [Math]::Round(($taskSamples | Measure-Object workingSetBytes -Maximum).Maximum / 1MB,2)
    averagePrivateMiB = [Math]::Round(($taskSamples | Measure-Object privateBytes -Average).Average / 1MB,2)
    maximumWorkingSetBytes = ($taskSamples | Measure-Object workingSetBytes -Maximum).Maximum
    memoryTargetBytes = 200000000
    withinMemoryTarget = ($taskSamples | Measure-Object workingSetBytes -Maximum).Maximum -le 200000000
    note = 'MiB = 1048576 bytes. Application process only; GPU, Bash, agents and WSL are separate. Zero CPU delta is limited by the Windows process-time measurement resolution.'
}
$taskOutput = if ([System.IO.Path]::IsPathRooted($OutputPath)) {
    [System.IO.Path]::GetFullPath($OutputPath)
} else {
    [System.IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $PSScriptRoot) $OutputPath))
}
New-Item -ItemType Directory -Path (Split-Path -Parent $taskOutput) -Force | Out-Null
$taskJson = $taskResult | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($taskOutput, $taskJson, [System.Text.UTF8Encoding]::new($false))
$taskJson
