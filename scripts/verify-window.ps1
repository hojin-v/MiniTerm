param([string]$Executable = 'artifacts/miniterm.exe', [switch]$ReducedMotion, [string]$OutputPath = 'artifacts/window.json', [ValidateSet('dark','light')][string]$Theme = 'light', [ValidateRange(280,1000)][int]$MiniWidth = 620)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExecutable = [System.IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
if (-not (Test-Path -LiteralPath $taskExecutable)) { throw 'Build MiniTerm before running the window probe.' }
if (Get-Process miniterm -ErrorAction SilentlyContinue) { throw 'Close the running MiniTerm before the isolated window probe.' }
Add-Type -AssemblyName System.Drawing
if (-not ('MiniTermWindowProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MiniTermWindowProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MonitorInfo { public int Size; public Rect Monitor,Work; public uint Flags; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hwnd,uint flags);
    [DllImport("user32.dll")] public static extern bool GetMonitorInfo(IntPtr monitor,ref MonitorInfo info);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
    [DllImport("user32.dll")] static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
    public static void Click(int x,int y) { SetCursorPos(x,y); System.Threading.Thread.Sleep(35); mouse_event(2,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(30); mouse_event(4,0,0,0,UIntPtr.Zero); }
    public static Rect Bounds(IntPtr hwnd) { Rect rect; if (!GetWindowRect(hwnd,out rect)) throw new Exception("Window is unavailable"); return rect; }
    public static MonitorInfo Monitor(IntPtr hwnd) { var info=new MonitorInfo { Size=Marshal.SizeOf(typeof(MonitorInfo)) }; if (!GetMonitorInfo(MonitorFromWindow(hwnd,2),ref info)) throw new Exception("Monitor unavailable"); return info; }
}
'@
}
$taskFixture = Join-Path $taskRoot ('artifacts/window-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskFixture | Out-Null
$taskSettings = @{ schemaVersion=1; theme=$Theme; fontSize=14; notifications=$false; shortcut=''; window=@{ monitor='\\.\MISSING'; miniWidth=$MiniWidth; miniHeight=136; right=24; bottom=16; expandedWidthPercent=60; expandedHeightPercent=75; reducedMotion=[bool]$ReducedMotion } }
$taskSettingsPath = Join-Path $taskFixture 'settings.json'
[System.IO.File]::WriteAllText($taskSettingsPath,($taskSettings | ConvertTo-Json -Depth 5),[System.Text.UTF8Encoding]::new($false))
$taskSettingsHash = (Get-FileHash -LiteralPath $taskSettingsPath -Algorithm SHA256).Hash
$taskPreviousFocus = [MiniTermWindowProbe]::GetForegroundWindow()
$taskPreviousDpi = [MiniTermWindowProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskPreviousPointer = New-Object MiniTermWindowProbe+Point
[MiniTermWindowProbe]::GetCursorPos([ref]$taskPreviousPointer) | Out-Null
$taskStart = New-Object System.Diagnostics.ProcessStartInfo
$taskStart.FileName = $taskExecutable
$taskStart.UseShellExecute = $false
$taskStart.CreateNoWindow = $true
$taskStart.EnvironmentVariables['MINITERM_DATA_DIR'] = $taskFixture
$taskStart.EnvironmentVariables['MINITERM_DIAGNOSTICS'] = '1'
$taskStart.EnvironmentVariables['MINITERM_METRICS_PATH'] = Join-Path $taskFixture 'metrics.json'
$taskProcess = [System.Diagnostics.Process]::Start($taskStart)
function Invoke-WindowDiagnostic([string]$Command) {
    & $taskExecutable diagnostic $Command | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Diagnostic command failed: $Command" }
}
function Save-WindowImage([IntPtr]$Window,[string]$Name) {
    $taskBounds = [MiniTermWindowProbe]::Bounds($Window)
    $taskImage = New-Object System.Drawing.Bitmap ($taskBounds.Right-$taskBounds.Left),($taskBounds.Bottom-$taskBounds.Top)
    $taskGraphics = [System.Drawing.Graphics]::FromImage($taskImage)
    try { $taskGraphics.CopyFromScreen($taskBounds.Left,$taskBounds.Top,0,0,$taskImage.Size); $taskImage.Save((Join-Path $taskFixture $Name)) }
    finally { $taskGraphics.Dispose(); $taskImage.Dispose() }
}
try {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100
        $taskProcess.Refresh()
        $taskReady = $false
        if ($taskProcess.MainWindowHandle -ne [IntPtr]::Zero) {
            $taskInitialBounds = [MiniTermWindowProbe]::Bounds($taskProcess.MainWindowHandle)
            $taskReady = [MiniTermWindowProbe]::IsWindowVisible($taskProcess.MainWindowHandle) -and ($taskInitialBounds.Right-$taskInitialBounds.Left) -gt 100
        }
    } while (-not $taskReady -and -not $taskProcess.HasExited -and [DateTime]::UtcNow -lt $taskDeadline)
    if ($taskProcess.HasExited -or -not $taskReady) { throw 'MiniTerm did not start.' }
    $taskWindow = $taskProcess.MainWindowHandle
    Start-Sleep -Milliseconds 700
    $taskMini = [MiniTermWindowProbe]::Bounds($taskWindow)
    $taskMonitor = [MiniTermWindowProbe]::Monitor($taskWindow)
    $taskScale = [MiniTermWindowProbe]::GetDpiForWindow($taskWindow) / 96.0
    if (($taskMini.Right-$taskMini.Left) -ne [Math]::Round($MiniWidth*$taskScale) -or ($taskMini.Bottom-$taskMini.Top) -ne [Math]::Round(136*$taskScale)) { throw "Configured mini DIP size was not applied: bounds=$($taskMini | ConvertTo-Json -Compress) scale=$taskScale" }
    if ($taskMini.Right -ne $taskMonitor.Work.Right-[Math]::Round(24*$taskScale) -or $taskMini.Bottom -ne $taskMonitor.Work.Bottom-[Math]::Round(16*$taskScale) -or ($taskMonitor.Flags -band 1) -eq 0) { throw 'Disconnected-monitor fallback or configured offsets failed.' }
    Save-WindowImage $taskWindow 'mini-light.png'
    $taskAnimation = @()
    $taskWatch = [System.Diagnostics.Stopwatch]::StartNew()
    Invoke-WindowDiagnostic 'expand'
    do {
        $taskBounds = [MiniTermWindowProbe]::Bounds($taskWindow)
        if ($taskBounds.Right -ne $taskMini.Right -or $taskBounds.Bottom -ne $taskMini.Bottom) { throw 'Animation changed the right or bottom anchor.' }
        $taskAnimation += [pscustomobject]@{ elapsedMs=$taskWatch.Elapsed.TotalMilliseconds; width=$taskBounds.Right-$taskBounds.Left; height=$taskBounds.Bottom-$taskBounds.Top }
        Start-Sleep -Milliseconds 8
    } while ($taskWatch.Elapsed.TotalMilliseconds -lt 350)
    $taskExpanded = [MiniTermWindowProbe]::Bounds($taskWindow)
    if (($taskExpanded.Bottom-$taskExpanded.Top) -ne [Math]::Round(($taskMonitor.Work.Bottom-$taskMonitor.Work.Top)*0.75)) { throw 'Configured expansion height failed.' }
    Invoke-WindowDiagnostic 'settings'
    Start-Sleep -Milliseconds 350
    Save-WindowImage $taskWindow 'settings-terminal.png'
    [MiniTermWindowProbe]::SetForegroundWindow($taskWindow) | Out-Null
    $taskCenterX = ($taskExpanded.Left+$taskExpanded.Right)/2
    $taskCenterY = ($taskExpanded.Top+$taskExpanded.Bottom)/2
    [MiniTermWindowProbe]::Click([int]($taskCenterX-197*$taskScale),[int]($taskCenterY-230*$taskScale))
    Start-Sleep -Milliseconds 150
    Save-WindowImage $taskWindow 'settings-window.png'
    [MiniTermWindowProbe]::Click([int]($taskCenterX-231*$taskScale),[int]($taskCenterY-230*$taskScale))
    Start-Sleep -Milliseconds 150
    [MiniTermWindowProbe]::Click([int]($taskCenterX-155*$taskScale),[int]($taskCenterY-170*$taskScale))
    Start-Sleep -Milliseconds 150
    Save-WindowImage $taskWindow 'settings-wsl.png'
    Invoke-WindowDiagnostic 'hide'
    Start-Sleep -Milliseconds 150
    if ([MiniTermWindowProbe]::IsWindowVisible($taskWindow)) { throw 'Diagnostic hide failed.' }
    $taskSessionIds = @((& $taskExecutable sessions | ConvertFrom-Json).sessions)
    Invoke-WindowDiagnostic 'expand'
    Start-Sleep -Milliseconds 350
    for ($taskIndex=1; $taskIndex -lt 4; $taskIndex++) { Invoke-WindowDiagnostic 'new-session'; Start-Sleep -Milliseconds 350 }
    $taskReply = & $taskExecutable sessions | ConvertFrom-Json
    if ($taskReply.sessions.Count -ne 4 -or $taskReply.sessions -notcontains $taskSessionIds[0]) { throw 'Session identity was lost or four sessions were not created.' }
    Invoke-WindowDiagnostic 'hide'
    Invoke-WindowDiagnostic 'collapse'
    Start-Sleep -Milliseconds 200
    Save-WindowImage $taskWindow 'four-sessions-mini.png'
    [MiniTermWindowProbe]::SetCursorPos($taskMini.Right-[int](47*$taskScale),$taskMini.Top+[int](12*$taskScale)) | Out-Null
    Start-Sleep -Milliseconds 700
    Save-WindowImage $taskWindow 'toolbar-hover.png'
    Invoke-WindowDiagnostic 'new-session'
    Start-Sleep -Milliseconds 200
    if ((& $taskExecutable sessions | ConvertFrom-Json).sessions.Count -ne 4) { throw 'The four-session limit failed.' }
    Invoke-WindowDiagnostic 'quit'
    if (-not $taskProcess.WaitForExit(10000)) { throw 'Graceful diagnostic exit failed.' }
    if ((Get-FileHash -LiteralPath $taskSettingsPath -Algorithm SHA256).Hash -ne $taskSettingsHash) { throw 'The isolated preferences changed unexpectedly.' }
    $taskMetrics = Get-Content -LiteralPath (Join-Path $taskFixture 'metrics.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $taskResult = [pscustomobject]@{ executable=$taskExecutable; executableSha256=(Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash; fixture=$taskFixture; theme=$Theme; miniWidth=$MiniWidth; configuredSizeAndOffsets=$true; missingMonitorFallsBackToPrimary=$true; anchorsPreserved=$true; reducedMotion=[bool]$ReducedMotion; animationSamples=$taskAnimation; fourSessionLimit=$true; settingsRetained=$true; gracefulExit=$true; metrics=$taskMetrics; note='One physical monitor tested. Mixed DPI and physical unplug/replug still need separate verification. Animation samples include IPC dispatch time.' }
    $taskJson = $taskResult | ConvertTo-Json -Depth 8
    [System.IO.File]::WriteAllText((Join-Path $taskRoot $OutputPath),$taskJson,[System.Text.UTF8Encoding]::new($false))
    $taskJson
} finally {
    if (-not $taskProcess.HasExited) {
        $taskTracked = Get-Process -Id $taskProcess.Id -ErrorAction SilentlyContinue
        if ($taskTracked -and $taskTracked.Path -eq $taskExecutable) { Stop-Process -Id $taskProcess.Id }
    }
    [MiniTermWindowProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    [MiniTermWindowProbe]::SetThreadDpiAwarenessContext($taskPreviousDpi) | Out-Null
    [MiniTermWindowProbe]::SetCursorPos($taskPreviousPointer.X,$taskPreviousPointer.Y) | Out-Null
}
