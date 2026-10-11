param([string]$Executable = 'artifacts/miniterm.exe', [string]$WslDistribution, [switch]$SkipIdle)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExecutable = [System.IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
if (Get-Process miniterm -ErrorAction SilentlyContinue) { throw 'Close the running MiniTerm before this isolated probe.' }
$taskFixture = Join-Path $taskRoot ('artifacts/session-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskFixture | Out-Null
$taskProcess = $null
function Start-SessionFixture([string]$Name,[object]$Profile) {
    $taskData = Join-Path $taskFixture $Name
    New-Item -ItemType Directory -Path $taskData | Out-Null
    $taskSettings = @{ schemaVersion=1; theme='light'; notifications=$false; shortcut=''; window=@{ reducedMotion=$true } }
    if ($Profile) { $taskSettings['profile'] = $Profile }
    [System.IO.File]::WriteAllText((Join-Path $taskData 'settings.json'),($taskSettings | ConvertTo-Json -Depth 5),[System.Text.UTF8Encoding]::new($false))
    $taskStart = New-Object System.Diagnostics.ProcessStartInfo
    $taskStart.FileName = $taskExecutable
    $taskStart.UseShellExecute = $false
    $taskStart.CreateNoWindow = $true
    $taskStart.EnvironmentVariables['MINITERM_DATA_DIR'] = $taskData
    $taskStart.EnvironmentVariables['MINITERM_DIAGNOSTICS'] = '1'
    $taskStart.EnvironmentVariables['MINITERM_METRICS_PATH'] = Join-Path $taskData 'metrics.json'
    $taskStart.EnvironmentVariables['MINITERM_QA_PID_PATH'] = Join-Path $taskData 'linux-pid.txt'
    $taskStart.EnvironmentVariables['WSLENV'] = $env:WSLENV + ':MINITERM_QA_PID_PATH/p'
    $taskStarted = [System.Diagnostics.Process]::Start($taskStart)
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(20)
    do { Start-Sleep -Milliseconds 100; $taskStarted.Refresh() } while ($taskStarted.MainWindowHandle -eq [IntPtr]::Zero -and -not $taskStarted.HasExited -and [DateTime]::UtcNow -lt $taskDeadline)
    if ($taskStarted.HasExited) { throw 'MiniTerm exited during startup.' }
    Start-Sleep -Milliseconds 1500
    $taskStarted.Refresh()
    return $taskStarted
}
function Invoke-SessionDiagnostic([string]$Command) {
    & $taskExecutable diagnostic $Command | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Diagnostic failed: $Command" }
}
function Close-SessionFixture {
    Invoke-SessionDiagnostic 'quit'
    if (-not $taskProcess.WaitForExit(10000)) { throw 'Graceful exit failed.' }
}
try {
    $taskProcess = Start-SessionFixture 'bash' $null
    & (Join-Path $PSScriptRoot 'verify-ui.ps1') -ProcessId $taskProcess.Id | Out-Null
    if (-not $SkipIdle) { & (Join-Path $PSScriptRoot 'measure-idle.ps1') -ProcessId $taskProcess.Id -Seconds 30 -OutputPath 'artifacts/idle-window-one.json' | Out-Null }
    for ($taskIndex=1; $taskIndex -lt 4; $taskIndex++) { Invoke-SessionDiagnostic 'new-session'; Start-Sleep -Milliseconds 450 }
    $taskReply = & $taskExecutable sessions | ConvertFrom-Json
    if ($taskReply.sessions.Count -ne 4) { throw 'Four sessions were not registered.' }
    $taskBashChildren = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($taskProcess.Id) AND Name='bash.exe'" | Select-Object -ExpandProperty ProcessId)
    if ($taskBashChildren.Count -ne 4) { throw 'Four Bash children were not created.' }
    if (-not $SkipIdle) { & (Join-Path $PSScriptRoot 'measure-idle.ps1') -ProcessId $taskProcess.Id -Seconds 30 -OutputPath 'artifacts/idle-window-four.json' | Out-Null }
    Close-SessionFixture
    foreach ($taskChild in $taskBashChildren) { if (Get-Process -Id $taskChild -ErrorAction SilentlyContinue) { throw "Owned Bash child is still running: $taskChild" } }
    $taskWslResult = $null
    if ($WslDistribution) {
        $taskProcess = Start-SessionFixture 'wsl' @{ kind='wsl'; distribution=$WslDistribution; directory='~' }
        $taskWindow = $taskProcess.MainWindowHandle
        $taskReply = & $taskExecutable sessions | ConvertFrom-Json
        if ($taskReply.sessions.Count -ne 1) { throw 'WSL session not registered.' }
        $taskId = $taskReply.sessions[0]
        $taskWslChildren = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($taskProcess.Id) AND Name='wsl.exe'" | Select-Object -ExpandProperty ProcessId)
        if ($taskWslChildren.Count -ne 1) { throw 'WSL child not created.' }
        $taskBounds = [MiniTermUiProbe]::Bounds($taskWindow)
        [MiniTermUiProbe]::Click($taskBounds.Left+20,$taskBounds.Top+55)
        Start-Sleep -Milliseconds 150
        [MiniTermUiProbe]::Text($taskWindow,@'
HISTFILE=/dev/null; HISTSIZE=0; printf '%s' "$$" > "$MINITERM_QA_PID_PATH"; sleep 0.8; "$MINITERM_NOTIFY_EXE" notify codex '{"type":"agent-turn-complete","thread-id":"wsl-fixture","turn-id":"wsl-turn-1","cwd":"/home/miniterm-fixture","last-assistant-message":"PRIVATE_FIXTURE"}'
'@)
        [MiniTermUiProbe]::Enter()
        Invoke-SessionDiagnostic 'hide'
        Start-Sleep -Milliseconds 150
        $taskForeground = [MiniTermUiProbe]::GetForegroundWindow()
        Start-Sleep -Milliseconds 1500
        $taskReply = & $taskExecutable sessions | ConvertFrom-Json
        if ($taskReply.details.states.$taskId -ne 'complete' -or $taskReply.details.unread -notcontains $taskId -or $taskReply.details.noticeCount -ne 1) { throw "WSL helper did not deliver the expected event: $($taskReply | ConvertTo-Json -Depth 5 -Compress)" }
        if ([MiniTermUiProbe]::IsWindowVisible($taskWindow) -or [MiniTermUiProbe]::GetForegroundWindow() -ne $taskForeground) { throw 'WSL completion changed visibility or focus.' }
        Invoke-SessionDiagnostic 'collapse'
        Start-Sleep -Milliseconds 200
        $taskBadgeBounds = [MiniTermUiProbe]::Bounds($taskWindow)
        $taskBadgeImage = New-Object System.Drawing.Bitmap ($taskBadgeBounds.Right-$taskBadgeBounds.Left),($taskBadgeBounds.Bottom-$taskBadgeBounds.Top)
        $taskBadgeGraphics = [System.Drawing.Graphics]::FromImage($taskBadgeImage)
        try { $taskBadgeGraphics.CopyFromScreen($taskBadgeBounds.Left,$taskBadgeBounds.Top,0,0,$taskBadgeImage.Size); $taskBadgeImage.Save((Join-Path $taskFixture 'wsl-unread-badge.png')) }
        finally { $taskBadgeGraphics.Dispose(); $taskBadgeImage.Dispose() }
        & $taskExecutable activate $taskId | Out-Null
        Start-Sleep -Milliseconds 250
        $taskReply = & $taskExecutable sessions | ConvertFrom-Json
        if ($taskReply.sessions[0] -ne $taskId -or $taskReply.details.unread.Count -ne 0) { throw 'WSL session activation lost identity or unread status.' }
        $taskLinuxPid = [int](Get-Content -LiteralPath (Join-Path $taskFixture 'wsl/linux-pid.txt') -Raw)
        Close-SessionFixture
        foreach ($taskChild in $taskWslChildren) { if (Get-Process -Id $taskChild -ErrorAction SilentlyContinue) { throw 'Owned WSL child survived app exit.' } }
        Start-Sleep -Milliseconds 250
        & (Join-Path $env:SystemRoot 'System32/wsl.exe') --distribution $WslDistribution --exec /bin/bash --noprofile --norc -c 'kill -0 "$1" 2>/dev/null' bash $taskLinuxPid | Out-Null
        if ($LASTEXITCODE -eq 0) { throw "WSL Bash PID $taskLinuxPid survived the app exit." }
        $taskWslResult = [pscustomobject]@{ distribution=$WslDistribution; sessionId=$taskId; nativeHelperEventDelivered=$true; hiddenFocusPreserved=$true; activationPreservedSession=$true; windowsChildExited=$true; linuxShellExited=$true; agentConfigurationModified=$false }
    }
    $taskResult = [pscustomobject]@{ executable=$taskExecutable; executableSha256=(Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash; fixture=$taskFixture; singleClickAndInput=$true; fourBashChildren=$true; shellChildrenExited=$true; wsl=$taskWslResult; note='Fixtures only. Literal Unicode text injection does not verify Korean IME composition. WSL event delivery is direct helper invocation, not an installed Linux agent hook.' }
    $taskJson = $taskResult | ConvertTo-Json -Depth 7
    [System.IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/sessions.json'),$taskJson,[System.Text.UTF8Encoding]::new($false))
    $taskJson
} finally {
    if ($taskProcess -and -not $taskProcess.HasExited) {
        $taskTracked = Get-Process -Id $taskProcess.Id -ErrorAction SilentlyContinue
        if ($taskTracked -and $taskTracked.Path -eq $taskExecutable) { Stop-Process -Id $taskProcess.Id }
    }
}
