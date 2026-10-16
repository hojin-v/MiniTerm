param([string]$Executable = 'artifacts/miniterm.exe')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExe = [IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
$taskId = [Guid]::NewGuid().ToString()
$taskFixture = Join-Path $taskRoot ('artifacts/features-' + $taskId)
New-Item -ItemType Directory -Path $taskFixture | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FeatureProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
    public delegate bool EnumProc(IntPtr window,IntPtr param);
    [StructLayout(LayoutKind.Explicit,Size=40)] public struct Input {
        [FieldOffset(0)] public uint Type; [FieldOffset(8)] public ushort Key;
        [FieldOffset(10)] public ushort Scan; [FieldOffset(12)] public uint Flags;
    }
    [DllImport("user32.dll")] public static extern uint SendInput(uint count,Input[] inputs,int size);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window,out Rect rect);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint process);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback,IntPtr param);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr window,StringBuilder text,int max);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window,uint msg,IntPtr wp,IntPtr lp);
    [DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
    [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint key,uint type);
    [DllImport("user32.dll")] public static extern short GetAsyncKeyState(int key);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool GetLayeredWindowAttributes(IntPtr window,out uint key,out byte alpha,out uint flags);
    public static Rect Bounds(IntPtr window) { Rect r; if (!GetWindowRect(window,out r)) throw new Exception("Window missing"); return r; }
    public static IntPtr Find(int pid,string title) {
        IntPtr found=IntPtr.Zero;
        EnumWindows((w,p) => { uint id; GetWindowThreadProcessId(w,out id); var s=new StringBuilder(256); GetWindowText(w,s,256);
            if (id==pid && s.ToString()==title && IsWindowVisible(w)) { found=w; return false; } return true; },IntPtr.Zero);
        return found;
    }
    public static void Key(byte key,bool up) {
        var input=new Input { Type=1,Scan=(ushort)MapVirtualKey(key,0),Flags=8u|(up?2u:0u)|(key==163?1u:0u) };
        if (SendInput(1,new[] { input },40)!=1) throw new Exception("Native key input rejected");
    }
    public static void Click(IntPtr window,int x,int y,bool right=false) {
        var p=(IntPtr)((y<<16)|(x&65535));
        PostMessage(window,512,IntPtr.Zero,p); PostMessage(window,right?516u:513u,(IntPtr)(right?2:1),p);
        System.Threading.Thread.Sleep(60); PostMessage(window,right?517u:514u,IntPtr.Zero,p);
    }
    public static void Text(IntPtr window,string text) {
        if (GetForegroundWindow()!=window) throw new Exception("Own terminal is not focused for text input");
        foreach (var c in text) {
            var down=new Input { Type=1,Scan=c,Flags=4 };
            var up=new Input { Type=1,Scan=c,Flags=6 };
            if (SendInput(2,new[] { down,up },40)!=2) throw new Exception("Native text input rejected");
            System.Threading.Thread.Sleep(30);
        }
    }
}
'@
$taskPreviousFocus = [FeatureProbe]::GetForegroundWindow()
$taskPreviousPointer = New-Object FeatureProbe+Point
[FeatureProbe]::GetCursorPos([ref]$taskPreviousPointer) | Out-Null
$taskDpi = [FeatureProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskUserProcesses = @(Get-Process miniterm -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$taskSettingsPath = Join-Path $taskFixture 'settings.json'
$taskSettings = @{schemaVersion=1;shortcut='';notifications=$false;profile=@{kind='git_bash';directory=$taskRoot};window=@{anchor=@{right=7200;bottom=500};miniWidth=560;miniHeight=112;autoCollapse=$false;transparency=0}}
[IO.File]::WriteAllText($taskSettingsPath,($taskSettings | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
function New-FeatureStart([string]$Arguments) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$taskExe; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.EnvironmentVariables['MINITERM_INSTANCE_ID']=$taskId
    $start.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
    $start.EnvironmentVariables['MINITERM_DATA_DIR']=$taskFixture
    return $start
}
function Invoke-Feature([string]$Arguments) {
    $start=New-FeatureStart $Arguments; $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($start)
    try {
        $out=$p.StandardOutput.ReadToEndAsync(); $err=$p.StandardError.ReadToEndAsync()
        if (-not $p.WaitForExit(10000)) { $p.Kill(); throw 'Feature helper timed out' }
        if ($p.ExitCode -ne 0) { throw $err.Result }
        return $out.Result
    } finally { $p.Dispose() }
}
function Wait-Sessions([int]$Count) {
    $deadline=[DateTime]::UtcNow.AddSeconds(12)
    do {
        $snapshot=(Invoke-Feature 'sessions') | ConvertFrom-Json
        if (@($snapshot.sessions).Count -eq $Count) { return $snapshot }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Expected $Count sessions; got $(@($snapshot.sessions).Count)"
}
function Get-FeatureWidget([string]$Name) {
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {
        $snapshot=(Invoke-Feature 'sessions') | ConvertFrom-Json
        $entry=$snapshot.details.ui.PSObject.Properties[$Name]
        if ($null -ne $entry) { return $entry.Value }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Missing feature widget: $Name"
}
function Click-Feature([string]$Name,[switch]$Right) {
    $r=Get-FeatureWidget $Name
    $bounds=[FeatureProbe]::Bounds($taskWindow)
    [FeatureProbe]::SetCursorPos($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale),$bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) | Out-Null
    Start-Sleep -Milliseconds 80
    $pointer=New-Object FeatureProbe+Point
    [FeatureProbe]::GetCursorPos([ref]$pointer) | Out-Null
    [IO.File]::WriteAllText((Join-Path $taskFixture ('pointer-' + $Name + '.txt')),"expected=$($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale)),$($bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) actual=$($pointer.X),$($pointer.Y) hit=$([FeatureProbe]::WindowFromPoint($pointer)) window=$taskWindow",[Text.Encoding]::UTF8)
    [FeatureProbe]::Click($taskWindow,[int](($r[0]+$r[2])*0.5*$taskScale),[int](($r[1]+$r[3])*0.5*$taskScale),[bool]$Right)
    Start-Sleep -Milliseconds 200
}
function Save-FeatureImage([IntPtr]$Window,[string]$Name) {
    $r=[FeatureProbe]::Bounds($Window)
    $bitmap=New-Object Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top)
    $g=[Drawing.Graphics]::FromImage($bitmap)
    try { $g.CopyFromScreen($r.Left,$r.Top,0,0,$bitmap.Size); $bitmap.Save((Join-Path $taskFixture $Name)) }
    finally { $g.Dispose(); $bitmap.Dispose() }
}
function New-TabShortcut {
    $r=Get-FeatureWidget 'terminal'; $bounds=[FeatureProbe]::Bounds($taskWindow)
    [FeatureProbe]::SetCursorPos($bounds.Left+[int](($r[0]+25)*$taskScale),$bounds.Top+[int](($r[1]+20)*$taskScale)) | Out-Null
    [FeatureProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    [FeatureProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    [FeatureProbe]::SetForegroundWindow($taskWindow) | Out-Null
    Start-Sleep -Milliseconds 100
    if ([FeatureProbe]::GetForegroundWindow() -ne $taskWindow) { throw 'Own test window could not obtain keyboard focus' }
    try {
        [FeatureProbe]::Key(163,$false)
        Start-Sleep -Milliseconds 200
        [IO.File]::WriteAllText((Join-Path $taskFixture 'native-control-state.txt'),"control=$([FeatureProbe]::GetAsyncKeyState(17)) left=$([FeatureProbe]::GetAsyncKeyState(162)) scan=$([FeatureProbe]::MapVirtualKey(162,0))",[Text.Encoding]::UTF8)
        if (([FeatureProbe]::GetAsyncKeyState(17) -band 0x8000) -eq 0) { return $false }
        [FeatureProbe]::Key(160,$false)
        Start-Sleep -Milliseconds 200
        [FeatureProbe]::Key(84,$false); Start-Sleep -Milliseconds 150; [FeatureProbe]::Key(84,$true)
        Start-Sleep -Milliseconds 150
    } finally { [FeatureProbe]::Key(160,$true); [FeatureProbe]::Key(163,$true) }
    return $true
}
$taskProcess=$null
$taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash;passed=$false}
try {
    $taskProcess=[Diagnostics.Process]::Start((New-FeatureStart ''))
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100; $taskProcess.Refresh(); $taskWindow=$taskProcess.MainWindowHandle
        $ready=$false
        if ($taskWindow -ne [IntPtr]::Zero) { $bounds=[FeatureProbe]::Bounds($taskWindow); $ready=($bounds.Right-$bounds.Left) -gt 100 }
    } while (-not $ready -and -not $taskProcess.HasExited -and [DateTime]::UtcNow -lt $deadline)
    if (-not $ready) { throw 'Feature app failed to start' }
    $taskScale=[FeatureProbe]::GetDpiForWindow($taskWindow)/96.0
    $taskFirst=(Wait-Sessions 1).sessions[0]
    Start-Sleep -Milliseconds 700
    Get-FeatureWidget 'header-brand' | Out-Null
    Save-FeatureImage $taskWindow 'header-input.png'
    Click-Feature 'header-layout'
    if ((Get-Content -Raw -Encoding UTF8 $taskSettingsPath | ConvertFrom-Json).window.miniView -ne 'summary') { throw 'Summary switch did not persist' }
    Save-FeatureImage $taskWindow 'header-summary.png'
    Click-Feature 'header-layout'
    $taskResult.viewSwitch=$true
    $taskKeyboard=New-TabShortcut
    if (-not $taskKeyboard) { Click-Feature 'header-add' }
    $taskTwo=Wait-Sessions 2
    if ($taskTwo.sessions -notcontains $taskFirst) { throw 'New tab replaced the original shell' }
    $taskSecond=@($taskTwo.sessions | Where-Object { $_ -ne $taskFirst })[0]
    $taskResult.newTabShortcut=$taskKeyboard
    if (-not $taskKeyboard) { $taskResult.keyboardLimitation='Windows did not retain the injected Ctrl key; physical keyboard chords remain unverified. New-tab button is checked instead.' }
    $taskHide=Get-FeatureWidget 'header-hide'; $taskBounds=[FeatureProbe]::Bounds($taskWindow)
    $taskHoverX=$taskBounds.Left+[int](($taskHide[0]+$taskHide[2])*0.5*$taskScale)
    $taskHoverY=$taskBounds.Top+[int](($taskHide[1]+$taskHide[3])*0.5*$taskScale)
    [FeatureProbe]::SetCursorPos($taskHoverX,$taskHoverY) | Out-Null
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do { Start-Sleep -Milliseconds 100; $taskPopup=[FeatureProbe]::Find($taskProcess.Id,'투명도') }
    while ($taskPopup -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
    if ($taskPopup -eq [IntPtr]::Zero) { throw 'Hide hover did not open the transparency popup' }
    Get-FeatureWidget 'opacity-slider' | Out-Null
    Start-Sleep -Milliseconds 250
    $popupBounds=[FeatureProbe]::Bounds($taskPopup)
    if ($popupBounds.Bottom -gt $taskBounds.Top+27*$taskScale) { throw 'Transparency popup did not float above the header' }
    Save-FeatureImage $taskPopup 'transparency-popup.png'
    $r=Get-FeatureWidget 'opacity-slider'; $popupScale=[FeatureProbe]::GetDpiForWindow($taskPopup)/96.0
    $x=[int](($r[0]+$r[2])*0.5*$popupScale); $y=[int](($r[1]+8)*$popupScale)
    [FeatureProbe]::SetCursorPos($popupBounds.Left+$x,$popupBounds.Top+$y) | Out-Null
    Start-Sleep -Milliseconds 100
    [FeatureProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 70
    [FeatureProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 500
    $taskOpacity=(Get-Content -Raw -Encoding UTF8 $taskSettingsPath | ConvertFrom-Json).window.transparency
    if ($taskOpacity -le 0 -or $taskOpacity -gt 80) { throw 'Floating slider did not persist transparency' }
    $alpha=New-Object byte; $key=New-Object uint32; $flags=New-Object uint32
    [FeatureProbe]::GetLayeredWindowAttributes($taskWindow,[ref]$key,[ref]$alpha,[ref]$flags) | Out-Null
    if ([Math]::Abs($alpha - 255*(100-$taskOpacity)/100) -gt 2) { throw 'Transparency was not applied to the native window' }
    $taskResult.hoverSlider=$true
    $body=Get-FeatureWidget 'terminal'; $taskBounds=[FeatureProbe]::Bounds($taskWindow)
    [FeatureProbe]::SetCursorPos($taskBounds.Left+[int](($body[0]+30)*$taskScale),$taskBounds.Top+[int](($body[1]+20)*$taskScale)) | Out-Null
    Start-Sleep -Milliseconds 500
    [FeatureProbe]::SetForegroundWindow($taskWindow) | Out-Null
    Start-Sleep -Milliseconds 100
    if ($taskKeyboard) { try {
        [FeatureProbe]::Key(163,$false); [FeatureProbe]::Key(164,$false)
        Start-Sleep -Milliseconds 70
        [FeatureProbe]::mouse_event(0x0800,0,0,[uint32]4294967176,[UIntPtr]::Zero)
    } finally { [FeatureProbe]::Key(164,$true); [FeatureProbe]::Key(163,$true) }
    Start-Sleep -Milliseconds 500
    $taskScrolledOpacity=(Get-Content -Raw -Encoding UTF8 $taskSettingsPath | ConvertFrom-Json).window.transparency
    if ($taskScrolledOpacity -ne [Math]::Max(0,$taskOpacity-5)) { throw "Ctrl+Alt+wheel did not adjust transparency: $taskOpacity -> $taskScrolledOpacity" }
    $taskResult.modifierWheel=$true
    } else { $taskResult.modifierWheel=$false }
    Click-Feature 'header-hide'
    if ([FeatureProbe]::IsWindowVisible($taskWindow)) { throw 'Hide button did not hide' }
    if ((Wait-Sessions 2).sessions -notcontains $taskFirst) { throw 'Hiding changed shell identity' }
    Invoke-Feature ('activate ' + $taskSecond) | Out-Null
    Start-Sleep -Milliseconds 300
    [FeatureProbe]::Text($taskWindow,'exit')
    Start-Sleep -Milliseconds 100
    [FeatureProbe]::Key(13,$false); [FeatureProbe]::Key(13,$true)
    $taskOne=Wait-Sessions 1
    if ($taskOne.sessions[0] -ne $taskFirst) { throw 'exit removed the wrong tab' }
    $taskResult.exitRemovesTab=$true
    Invoke-Feature ('activate ' + $taskFirst) | Out-Null
    Start-Sleep -Milliseconds 250
    Click-Feature 'tab-0' -Right
    Click-Feature 'close-confirm'
    Wait-Sessions 0 | Out-Null
    if ([FeatureProbe]::IsWindowVisible($taskWindow) -or $taskProcess.HasExited) { throw 'Last tab must close the HUD and retain the tray process' }
    $taskResult.lastTabHidesHud=$true
    foreach ($id in $taskUserProcesses) { if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) { throw 'User MiniTerm was stopped' } }
    $taskResult.userSessionPreserved=$true
    $taskIcon=[Drawing.Icon]::ExtractAssociatedIcon($taskExe)
    if ($null -eq $taskIcon) { throw 'Executable has no application icon' }
    $taskIcon.ToBitmap().Save((Join-Path $taskFixture 'exe-icon.png')); $taskIcon.Dispose()
    $taskResult.exeIcon=$true
    $taskResult.passed=$true
    $taskResult.scope='Own isolated app; native pointer, client mouse messages, floating slider, typed exit and tab lifecycle. Keyboard chord/wheel pass only when their individual flags are true. Excludes actual Codex/model, human IME and mixed monitor DPI.'
} catch {
    $taskResult.error=$_.Exception.Message
    $taskResult.window=$taskWindow.ToInt64(); $taskResult.scale=$taskScale
    try { $taskResult.failureSnapshot=(Invoke-Feature 'sessions') | ConvertFrom-Json } catch {}
    try { Save-FeatureImage $taskWindow 'failure.png' } catch {}
    throw
} finally {
    [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
    if ($taskProcess -and -not $taskProcess.HasExited) { try { Invoke-Feature 'diagnostic quit' | Out-Null } catch {}; if (-not $taskProcess.WaitForExit(3000)) { $taskProcess.Kill(); $taskProcess.WaitForExit() } }
    [FeatureProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    [FeatureProbe]::SetCursorPos($taskPreviousPointer.X,$taskPreviousPointer.Y) | Out-Null
    [FeatureProbe]::SetThreadDpiAwarenessContext($taskDpi) | Out-Null
}
$taskResult | ConvertTo-Json -Depth 5
