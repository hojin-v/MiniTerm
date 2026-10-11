# Explicit user-requested restart: closes MiniTerm and its terminal sessions.
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$taskSource = Join-Path $taskRoot 'artifacts/miniterm.exe'
$taskStatusPath = Join-Path $taskRoot 'artifacts/CURRENT_BUILD_STATUS.json'
$taskStatus = Get-Content -LiteralPath $taskStatusPath -Raw -Encoding UTF8 | ConvertFrom-Json
$taskSourceHash = (Get-FileHash -LiteralPath $taskSource -Algorithm SHA256).Hash.ToLowerInvariant()
if ($taskSourceHash -ne $taskStatus.executableSha256) { throw 'Staged executable does not match verified build status.' }
$taskLocal = [Environment]::GetFolderPath('LocalApplicationData')
$taskInstall = [IO.Path]::GetFullPath((Join-Path $taskLocal 'Programs/MiniTerm/miniterm.exe'))
$taskSettingsPath = Join-Path $taskLocal 'MiniTerm/settings.json'
$taskFixture = Join-Path $taskRoot ('artifacts/manual-test/restart-' + [Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $taskFixture -Force | Out-Null
$taskOld = @(Get-Process miniterm -ErrorAction SilentlyContinue | ForEach-Object {
    if (-not $_.Path) { throw 'Cannot verify a running MiniTerm executable.' }
    $taskPath = [IO.Path]::GetFullPath($_.Path)
    $taskWorkspaceBinary = $taskPath.StartsWith($taskRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
    if ($taskPath -ne $taskInstall -and -not $taskWorkspaceBinary) { throw ('Unexpected MiniTerm path: ' + $taskPath) }
    [pscustomobject]@{processId=$_.Id;executable=$taskPath;startedUtc=$_.StartTime.ToUniversalTime().ToString('o');sha256=(Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$taskBackup = Join-Path $taskFixture 'previous-installed.exe'
$taskSettingsHash = $null
if (Test-Path -LiteralPath $taskInstall) { Copy-Item -LiteralPath $taskInstall -Destination $taskBackup }
if (Test-Path -LiteralPath $taskSettingsPath) {
    Copy-Item -LiteralPath $taskSettingsPath -Destination (Join-Path $taskFixture 'previous-settings.json')
    $taskSettingsHash = (Get-FileHash -LiteralPath $taskSettingsPath -Algorithm SHA256).Hash.ToLowerInvariant()
}
New-Item -ItemType Directory -Path (Split-Path -Parent $taskInstall) -Force | Out-Null
$taskNext = Join-Path (Split-Path -Parent $taskInstall) ('miniterm.next-' + [Guid]::NewGuid().ToString() + '.exe')
Copy-Item -LiteralPath $taskSource -Destination $taskNext
if ((Get-FileHash -LiteralPath $taskNext -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskSourceHash) { throw 'Prepared executable copy is corrupt.' }
$taskRecord = [ordered]@{requestedUtc=[DateTime]::UtcNow.ToString('o');executable=$taskInstall;executableSha256=$taskSourceHash;backupDirectory=$taskFixture;previousProcesses=$taskOld;settingsHashBefore=$taskSettingsHash;phase='prepared';processId=$null}
function Save-RestartRecord {
    [IO.File]::WriteAllText((Join-Path $taskFixture 'restart.json'),($taskRecord | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
}
Save-RestartRecord

# WM_CLOSE hides this application. User explicitly authorized closing the old
# versions; terminate verified app PIDs, which releases their shell-owned jobs.
foreach ($taskPrevious in $taskOld) {
    $taskProcess = Get-Process -Id $taskPrevious.processId -ErrorAction SilentlyContinue
    if ($null -eq $taskProcess) { continue }
    if ([IO.Path]::GetFullPath($taskProcess.Path) -ne $taskPrevious.executable -or $taskProcess.StartTime.ToUniversalTime().ToString('o') -ne $taskPrevious.startedUtc) { throw 'Previous PID no longer identifies the same MiniTerm process.' }
    Stop-Process -InputObject $taskProcess -Force
    if (-not $taskProcess.WaitForExit(10000)) { throw 'Previous MiniTerm did not exit.' }
}
if (@(Get-Process miniterm -ErrorAction SilentlyContinue).Count -ne 0) { throw 'A MiniTerm instance is still running; replacement aborted.' }
$taskRecord.phase='old_versions_closed'
Save-RestartRecord
function New-LatestStart([string]$Arguments) {
    $taskStart = New-Object Diagnostics.ProcessStartInfo
    $taskStart.FileName=$taskInstall; $taskStart.Arguments=$Arguments
    $taskStart.WorkingDirectory=$taskRoot; $taskStart.UseShellExecute=$false; $taskStart.CreateNoWindow=$true
    foreach ($taskVariable in @('MINITERM_INSTANCE_ID','MINITERM_DATA_DIR','MINITERM_DIAGNOSTICS','MINITERM_SESSION_ID','MINITERM_NOTIFY_EXE','MINITERM_SMOKE_SECONDS','MINITERM_METRICS_PATH')) { $taskStart.EnvironmentVariables.Remove($taskVariable) }
    return $taskStart
}
function Invoke-Latest([string]$Arguments) {
    $taskStart=New-LatestStart $Arguments
    $taskStart.RedirectStandardOutput=$true; $taskStart.RedirectStandardError=$true
    $taskHelper=[Diagnostics.Process]::Start($taskStart)
    try {
        $taskOutput=$taskHelper.StandardOutput.ReadToEndAsync(); $taskError=$taskHelper.StandardError.ReadToEndAsync()
        if (-not $taskHelper.WaitForExit(10000)) { $taskHelper.Kill(); throw 'Latest helper timed out.' }
        if ($taskHelper.ExitCode -ne 0) { throw $taskError.Result }
        return $taskOutput.Result
    } finally { $taskHelper.Dispose() }
}
$taskApp=$null
try {
    Copy-Item -LiteralPath $taskNext -Destination $taskInstall -Force
    if ((Get-FileHash -LiteralPath $taskInstall -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskSourceHash) { throw 'Installed executable copy does not match the verified build.' }
    Remove-Item -LiteralPath $taskNext
    $taskApp=[Diagnostics.Process]::Start((New-LatestStart ''))
    $taskRecord.processId=$taskApp.Id
    $taskRecord.phase='starting'
    Save-RestartRecord
    $taskDeadline=[DateTime]::UtcNow.AddSeconds(25)
    $taskSnapshot=$null
    do {
        Start-Sleep -Milliseconds 200
        $taskApp.Refresh()
        if ($taskApp.HasExited) { throw 'Latest MiniTerm exited during startup.' }
        if ($taskApp.MainWindowHandle -ne [IntPtr]::Zero) {
            $taskSnapshot=(Invoke-Latest 'sessions') | ConvertFrom-Json
            if ($taskSnapshot.ok -and @($taskSnapshot.sessions).Count -gt 0) { break }
        }
    } while ([DateTime]::UtcNow -lt $taskDeadline)
    if ($null -eq $taskSnapshot -or @($taskSnapshot.sessions).Count -eq 0) { throw 'Latest terminal session did not become ready.' }
    if ('progress-v1' -notin $taskSnapshot.capabilities -or 'progress-v1' -notin $taskSnapshot.helperCapabilities) { throw 'Running application is missing the latest progress capability.' }
    Invoke-Latest '' | Out-Null
    $taskRunning=@(Get-Process miniterm -ErrorAction SilentlyContinue)
    if ($taskRunning.Count -ne 1 -or $taskRunning[0].Id -ne $taskApp.Id) { throw 'More than one MiniTerm remains running.' }
    $taskRecord.phase='ready'
    $taskRecord.launchedUtc=[DateTime]::UtcNow.ToString('o')
    $taskRecord.sessions=@($taskSnapshot.sessions)
    $taskRecord.capabilities=@($taskSnapshot.capabilities)
    $taskRecord.oldVersionsClosed=$true
    $taskRecord.settingsHashAfter=(Get-FileHash -LiteralPath $taskSettingsPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $taskRecord.settingsPreserved=($taskRecord.settingsHashAfter -eq $taskSettingsHash)
    Save-RestartRecord
    [IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/manual-test/latest-running.json'),($taskRecord | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
    $taskStatus.installedSha256=$taskSourceHash
    $taskStatus.recordedUtc=[DateTime]::UtcNow.ToString('o')
    $taskStatus.liveUserInstances=@([pscustomobject]@{processId=$taskApp.Id;executable=$taskInstall;sha256=$taskSourceHash})
    $taskStatus | Add-Member -NotePropertyName latestUserLaunch -NotePropertyValue ([pscustomobject]@{fixture=$taskFixture;oldProcessIds=@($taskOld | ForEach-Object {$_.processId});processId=$taskApp.Id;defaultInstance=$true;installedExecutableUpdated=$true;installerRebuilt=$false;settingsPreserved=$taskRecord.settingsPreserved;progressCapabilityVerified=$true;realCodexLaunched=$false}) -Force
    [IO.File]::WriteAllText($taskStatusPath,($taskStatus | ConvertTo-Json -Depth 24),[Text.UTF8Encoding]::new($false))
    $taskRecord | ConvertTo-Json -Depth 8
} catch {
    $taskRecord.phase='failed'; $taskRecord.failure=$_.Exception.Message
    Save-RestartRecord
    if ($null -ne $taskApp -and -not $taskApp.HasExited) { $taskApp.Kill(); $taskApp.WaitForExit() }
    if (Test-Path -LiteralPath $taskBackup) { Copy-Item -LiteralPath $taskBackup -Destination $taskInstall -Force }
    throw
}
