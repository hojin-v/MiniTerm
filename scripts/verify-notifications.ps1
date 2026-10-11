param([Parameter(Mandatory = $true)][int]$ProcessId, [string]$HelperPath)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
if (-not $HelperPath) { $HelperPath = Join-Path $taskRoot 'artifacts/dev/miniterm.exe' }
$HelperPath = (Resolve-Path -LiteralPath $HelperPath).Path
$taskApp = Get-Process -Id $ProcessId
if ($taskApp.ProcessName -ne 'miniterm' -or $taskApp.Path -ne $HelperPath) { throw 'An owned MiniTerm test process is required' }
if (-not ('MiniTermNotificationProbe' -as [type])) { Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
[ComImport, Guid("53e31837-6600-4a81-9395-75cffe746f94"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface MiniTermToastCallback { void Activate([MarshalAs(UnmanagedType.LPWStr)]string app,[MarshalAs(UnmanagedType.LPWStr)]string args,IntPtr data,uint count); }
public static class MiniTermNotificationProbe {
[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr hwnd);
[DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr hwnd,int state);
[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr hwnd);
[DllImport("user32.dll")]public static extern bool AllowSetForegroundWindow(uint processId);
[DllImport("ole32.dll")]static extern int CoCreateInstance(ref Guid clsid,IntPtr outer,uint context,ref Guid iid,out IntPtr obj);
public static void Activate(string id){var clsid=new Guid("a1b499dc-c27d-4eac-8ff2-4fb9314ed8ae");var iid=new Guid("53e31837-6600-4a81-9395-75cffe746f94");IntPtr raw;int hr=CoCreateInstance(ref clsid,IntPtr.Zero,4,ref iid,out raw);Marshal.ThrowExceptionForHR(hr);object obj=Marshal.GetObjectForIUnknown(raw);Marshal.Release(raw);try{((MiniTermToastCallback)obj).Activate("MiniTerm.Hud","session="+id,IntPtr.Zero,0);}finally{Marshal.ReleaseComObject(obj);}}
}
'@
}
function Quote-Native([string]$Value) { '"' + [regex]::Replace([regex]::Replace($Value,'(\\*)"','$1$1\"'),'(\\+)$','$1$1') + '"' }
function Invoke-Helper([string[]]$Arguments, [string]$InputText) {
    $taskStart = New-Object System.Diagnostics.ProcessStartInfo
    $taskStart.FileName = $HelperPath
    $taskStart.Arguments = ($Arguments | ForEach-Object { Quote-Native $_ }) -join ' '
    $taskStart.UseShellExecute = $false
    $taskStart.CreateNoWindow = $true
    $taskStart.RedirectStandardOutput = $true
    $taskStart.RedirectStandardError = $true
    $taskStart.RedirectStandardInput = $true
    if ($script:taskSessionId) { $taskStart.EnvironmentVariables['MINITERM_SESSION_ID'] = $script:taskSessionId }
    $taskProcess = [System.Diagnostics.Process]::Start($taskStart)
    try {
        if ($InputText) { $taskBytes = [System.Text.Encoding]::UTF8.GetBytes($InputText); $taskProcess.StandardInput.BaseStream.Write($taskBytes,0,$taskBytes.Length) }
        $taskProcess.StandardInput.Close()
        if (-not $taskProcess.WaitForExit(5000)) { throw 'Notification helper did not finish' }
        $taskOut = $taskProcess.StandardOutput.ReadToEnd()
        $taskError = $taskProcess.StandardError.ReadToEnd()
        if ($taskProcess.ExitCode -ne 0 -or $taskError) { throw "Helper failed: $taskError" }
        $taskOut
    } finally { $taskProcess.Dispose() }
}
function Snapshot { Invoke-Helper @('sessions') | ConvertFrom-Json }
function Claude([string]$Name, [hashtable]$Extra = @{}) {
    $taskData = @{ hook_event_name = $Name; session_id = 'fixture-claude-main'; cwd = 'C:/MiniTerm-notification-test' }
    foreach ($taskKey in $Extra.Keys) { $taskData[$taskKey] = $Extra[$taskKey] }
    Invoke-Helper @('notify','claude') ($taskData | ConvertTo-Json -Compress) | Out-Null
}
function Assert-State([string]$Expected) {
    $taskSnapshot = Snapshot
    if ($taskSnapshot.details.states.$script:taskSessionId -ne $Expected) { throw "Expected $Expected state" }
}
$taskPreviousFocus = [MiniTermNotificationProbe]::GetForegroundWindow()
$taskWindow = $taskApp.MainWindowHandle
$taskInitial = Snapshot
if ($taskInitial.sessions.Count -ne 1) { throw 'A fresh single session is required' }
$script:taskSessionId = $taskInitial.sessions[0]
$taskCount = $taskInitial.details.noticeCount
$taskToasts = $taskInitial.details.toastCount
$taskShells = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$ProcessId AND Name='bash.exe'" | Select-Object -ExpandProperty ProcessId)
try {
    [MiniTermNotificationProbe]::ShowWindow($taskWindow,0) | Out-Null
    $taskPayload = @{ type = 'agent-turn-complete'; 'thread-id' = 'fixture-codex'; 'turn-id' = [Guid]::NewGuid().ToString(); cwd = 'C:/MiniTerm-notification-test'; 'last-assistant-message' = 'PRIVATE_CONTENT_MUST_NOT_APPEAR' } | ConvertTo-Json -Compress
    Invoke-Helper @('notify','codex',$taskPayload) | Out-Null
    Invoke-Helper @('notify','codex',$taskPayload) | Out-Null
    Assert-State 'complete'
    $taskAfter = Snapshot
    if ($taskAfter.details.noticeCount -ne $taskCount + 1 -or $taskAfter.details.unread -notcontains $script:taskSessionId) { throw 'Completion deduplication or hidden badge failed' }
    if ([MiniTermNotificationProbe]::GetForegroundWindow() -ne $taskPreviousFocus -or [MiniTermNotificationProbe]::IsWindowVisible($taskWindow)) { throw 'Background completion changed focus or visibility' }
    Claude 'SubagentStop' @{ agent_id = 'child' }
    Claude 'Stop' @{ agent_id = 'child' }
    Claude 'Stop' @{ stop_hook_active = $true }
    if ((Snapshot).details.noticeCount -ne $taskCount + 1) { throw 'Subagent/continuation falsely completed the main response' }
    Claude 'UserPromptSubmit'; Assert-State 'running'
    Claude 'PermissionRequest'; Assert-State 'needs_input'
    Claude 'PostToolUse'; Assert-State 'running'
    Claude 'StopFailure' @{ error = 'rate_limit'; error_details = 'private detail omitted' }; Assert-State 'failed'
    [MiniTermNotificationProbe]::AllowSetForegroundWindow([uint32]$ProcessId) | Out-Null
    [MiniTermNotificationProbe]::Activate($script:taskSessionId)
    Start-Sleep -Milliseconds 300
    if (-not [MiniTermNotificationProbe]::IsWindowVisible($taskWindow) -or [MiniTermNotificationProbe]::GetForegroundWindow() -ne $taskWindow) { throw "Native COM activation: visible=$([MiniTermNotificationProbe]::IsWindowVisible($taskWindow)) foreground=$([MiniTermNotificationProbe]::GetForegroundWindow()) expected=$taskWindow" }
    foreach ($taskShell in $taskShells) { Get-Process -Id $taskShell | Out-Null }
    Claude 'Stop' @{ stop_hook_active = $false }
    Start-Sleep -Milliseconds 100
    $taskViewed = Snapshot
    if ($taskViewed.details.toastCount -ne $taskToasts + 3 -or $taskViewed.details.unread -contains $script:taskSessionId) { throw "Viewed-session suppression failed: $($taskViewed.details | ConvertTo-Json -Compress)" }
    if ($taskViewed.details.toastError) { throw $taskViewed.details.toastError }
    $taskResult = [pscustomobject]@{ helperOfficialSchemas = $true; duplicateTurnSuppressed = $true; hiddenBadgeWithoutFocusTheft = $true; subagentsAndContinuationExcluded = $true; permissionResumeErrorStates = $true; viewedPopupSuppressed = $true; nativeComActivationRestored = $true; bashProcessIds = $taskShells; windowsToastsPublished = $taskViewed.details.toastCount - $taskToasts; actualToastUiClickTested = $false; note = 'Native COM activation was invoked directly after granting foreground rights. Actual Windows popup click and installed/restarted activation require separate validation.' }
    $taskJson = $taskResult | ConvertTo-Json
    [System.IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/notifications.json'),$taskJson,[System.Text.UTF8Encoding]::new($false))
    $taskJson
} finally { [MiniTermNotificationProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null }
