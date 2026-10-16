# Temporary incident workaround for the existing executable.
# Use Windows' console provider and bypass the AI Battery Windows terminal-row runner.
# Environment changes belong only to the newly launched MiniTerm process.
param([string]$Executable = 'artifacts/miniterm.exe', [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExecutable = if ([IO.Path]::IsPathRooted($Executable)) {
    [IO.Path]::GetFullPath($Executable)
} else {
    [IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
}
if (-not (Test-Path -LiteralPath $taskExecutable -PathType Leaf)) {
    throw "MiniTerm executable does not exist: $taskExecutable"
}
$taskBatteryBin = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'ai-battery/bin')).TrimEnd('\')
$taskPathEntries = @($env:PATH -split ';' | Where-Object {
    $taskEntry = $_.Trim().Trim('"').TrimEnd('\', '/')
    -not $taskEntry.Equals($taskBatteryBin, [StringComparison]::OrdinalIgnoreCase)
})
$taskInfo = [Diagnostics.ProcessStartInfo]::new()
$taskInfo.FileName = $taskExecutable
$taskInfo.WorkingDirectory = $taskRoot
$taskInfo.UseShellExecute = $false
$taskInfo.EnvironmentVariables['PATH'] = $taskPathEntries -join ';'
$taskInfo.EnvironmentVariables['AI_BATTERY_DISABLE_WINDOWS_CODEX_RUNNER'] = '1'
if ($CheckOnly) {
    [pscustomobject]@{
        executable = $taskExecutable
        executableSha256 = (Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
        excludedDllDirectory = $taskBatteryBin
        batteryWindowsRunnerDisabled = $true
        modifiesParentEnvironment = $false
    } | ConvertTo-Json
    return
}
$taskExisting = Get-CimInstance Win32_Process -Filter "Name='miniterm.exe'" |
    Where-Object { $_.ExecutablePath -eq $taskExecutable }
if ($taskExisting) {
    throw 'Close the existing MiniTerm first so the new process receives the workaround environment.'
}
$taskProcess = [Diagnostics.Process]::Start($taskInfo)
Write-Output "MiniTerm PID $($taskProcess.Id): system-console workaround; AI Battery terminal-row runner bypassed."
$taskProcess.Dispose()
