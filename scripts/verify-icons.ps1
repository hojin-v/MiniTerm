param([string]$Executable = 'artifacts/miniterm.exe')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExecutable = [System.IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
if (Get-Process miniterm -ErrorAction SilentlyContinue) { throw 'Close the running MiniTerm before this isolated icon probe.' }
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MiniTermIconProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
    public static Rect Bounds(IntPtr hwnd) { Rect rect; if (!GetWindowRect(hwnd,out rect)) throw new Exception("Window unavailable"); return rect; }
    public static void Click(int x,int y) { SetCursorPos(x,y); System.Threading.Thread.Sleep(35); mouse_event(2,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(30); mouse_event(4,0,0,0,UIntPtr.Zero); }
}
'@
$taskFixture = Join-Path $taskRoot ('artifacts/icons-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskFixture | Out-Null
$taskSettingsPath = Join-Path $taskFixture 'settings.json'
[System.IO.File]::WriteAllText($taskSettingsPath,'{"theme":"light","notifications":false,"shortcut":"","window":{"reducedMotion":true}}',[System.Text.UTF8Encoding]::new($false))
$taskPreviousDpi = [MiniTermIconProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskPreviousFocus = [MiniTermIconProbe]::GetForegroundWindow()
$taskPreviousPointer = New-Object MiniTermIconProbe+Point
[MiniTermIconProbe]::GetCursorPos([ref]$taskPreviousPointer) | Out-Null
$taskStart = New-Object System.Diagnostics.ProcessStartInfo
$taskStart.FileName = $taskExecutable
$taskStart.UseShellExecute = $false
$taskStart.CreateNoWindow = $true
$taskStart.EnvironmentVariables['MINITERM_DATA_DIR'] = $taskFixture
$taskStart.EnvironmentVariables['MINITERM_DIAGNOSTICS'] = '1'
$taskProcess = [System.Diagnostics.Process]::Start($taskStart)
function Click-Toolbar([int]$Offset) {
    $taskBounds = [MiniTermIconProbe]::Bounds($taskWindow)
    [MiniTermIconProbe]::Click($taskBounds.Right-[int]($Offset*$taskScale),$taskBounds.Top+[int](12*$taskScale))
    Start-Sleep -Milliseconds 200
}
function Click-SettingsFooter([int]$Offset) {
    $taskBounds = [MiniTermIconProbe]::Bounds($taskWindow)
    [MiniTermIconProbe]::Click([int](($taskBounds.Left+$taskBounds.Right)/2+$Offset*$taskScale),[int](($taskBounds.Top+$taskBounds.Bottom)/2+265*$taskScale))
    Start-Sleep -Milliseconds 200
}
function Blur-OwnedWindow {
    [MiniTermIconProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    Start-Sleep -Milliseconds 250
    if ([MiniTermIconProbe]::GetForegroundWindow() -eq $taskWindow) { throw 'Cannot verify blur because the previous foreground window did not activate.' }
}
function Save-IconImage([string]$Name) {
    $taskBounds = [MiniTermIconProbe]::Bounds($taskWindow)
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
            $taskInitialBounds = [MiniTermIconProbe]::Bounds($taskProcess.MainWindowHandle)
            $taskReady = ($taskInitialBounds.Right-$taskInitialBounds.Left) -gt 100
        }
    } while (-not $taskReady -and -not $taskProcess.HasExited -and [DateTime]::UtcNow -lt $taskDeadline)
    if (-not $taskReady) { throw 'MiniTerm did not start.' }
    $taskWindow = $taskProcess.MainWindowHandle
    $taskScale = [MiniTermIconProbe]::GetDpiForWindow($taskWindow)/96.0
    Start-Sleep -Milliseconds 600
    $taskMini = [MiniTermIconProbe]::Bounds($taskWindow)
    Click-Toolbar 103
    $taskExpanded = [MiniTermIconProbe]::Bounds($taskWindow)
    if (($taskExpanded.Bottom-$taskExpanded.Top) -le ($taskMini.Bottom-$taskMini.Top)) { throw 'Expand icon failed.' }
    Click-Toolbar 131
    $taskView = Get-Content -LiteralPath $taskSettingsPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($taskView.window.miniView -ne 'summary') { throw 'Mini summary icon did not switch the view.' }
    Click-Toolbar 131
    $taskView = Get-Content -LiteralPath $taskSettingsPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($taskView.window.miniView -ne 'input') { throw 'Mini input icon did not switch the view.' }
    Blur-OwnedWindow
    if ([MiniTermIconProbe]::Bounds($taskWindow).Top -ne $taskMini.Top) { throw 'Automatic collapse on blur failed.' }
    $taskBeforeCancel = Get-Content -LiteralPath $taskSettingsPath -Raw -Encoding UTF8
    Click-Toolbar 47
    Click-SettingsFooter -200
    if ((Get-Content -LiteralPath $taskSettingsPath -Raw -Encoding UTF8) -ne $taskBeforeCancel) { throw 'Cancel icon modified the saved file.' }
    Click-Toolbar 47
    Save-IconImage 'settings-before-save.png'
    Click-SettingsFooter -232
    Save-IconImage 'settings-after-save.png'
    $taskSaved = Get-Content -LiteralPath $taskSettingsPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($taskSaved.schemaVersion -ne 1 -or $taskSaved.window.miniWidth -ne 560) { throw 'Save icon failed to write the validated preferences.' }
    & $taskExecutable diagnostic hide | Out-Null
    & $taskExecutable diagnostic collapse | Out-Null
    Start-Sleep -Milliseconds 250
    Click-Toolbar 75
    & $taskExecutable diagnostic hide | Out-Null
    & $taskExecutable diagnostic collapse | Out-Null
    Start-Sleep -Milliseconds 250
    Click-Toolbar 159
    $taskReply = & $taskExecutable sessions | ConvertFrom-Json
    if ($taskReply.sessions.Count -ne 2) { throw 'New-session icon failed.' }
    & $taskExecutable diagnostic quit | Out-Null
    if (-not $taskProcess.WaitForExit(10000)) { throw 'Graceful exit failed.' }
    $taskJson = [pscustomobject]@{ executableSha256=(Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash; fixture=$taskFixture; expandIcon=$true; miniViewIcon=$true; autoCollapseOnBlur=$true; cancelPreservesSettings=$true; saveWritesSettings=$true; newSessionIcon=$true; note='Real mouse clicks. Tooltip and both themes are also inspected in the window probe screenshots.' } | ConvertTo-Json
    [System.IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/icons.json'),$taskJson,[System.Text.UTF8Encoding]::new($false))
    $taskJson
} finally {
    if (-not $taskProcess.HasExited) {
        $taskTracked = Get-Process -Id $taskProcess.Id -ErrorAction SilentlyContinue
        if ($taskTracked -and $taskTracked.Path -eq $taskExecutable) { Stop-Process -Id $taskProcess.Id }
    }
    [MiniTermIconProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    [MiniTermIconProbe]::SetCursorPos($taskPreviousPointer.X,$taskPreviousPointer.Y) | Out-Null
    [MiniTermIconProbe]::SetThreadDpiAwarenessContext($taskPreviousDpi) | Out-Null
}
