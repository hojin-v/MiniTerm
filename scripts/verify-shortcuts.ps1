param([string]$Executable = 'artifacts/miniterm.exe',[switch]$FixtureKeys)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExe = [IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
$taskId = [Guid]::NewGuid().ToString()
$taskFixture = Join-Path $taskRoot ('artifacts/shortcuts-' + $taskId)
New-Item -ItemType Directory -Path $taskFixture | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ShortcutProbe {
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
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window,IntPtr after,int x,int y,int width,int height,uint flags);
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
    public static void Chord(IntPtr window,byte key,bool ctrl,bool shift,bool alt) {
        if (GetForegroundWindow()!=window) throw new Exception("Own window is not focused");
        var mods=new System.Collections.Generic.List<Input>();
        if (ctrl) mods.Add(new Input { Type=1,Key=17 });
        if (shift) mods.Add(new Input { Type=1,Key=16 });
        if (alt) mods.Add(new Input { Type=1,Key=18 });
        try {
            if (mods.Count>0 && SendInput((uint)mods.Count,mods.ToArray(),40)!=mods.Count) throw new Exception("Native modifiers rejected");
            System.Threading.Thread.Sleep(220);
            var keys=new[] { new Input { Type=1,Key=key },new Input { Type=1,Key=key,Flags=2 } };
            if (SendInput(2,keys,40)!=2) throw new Exception("Native key rejected");
            System.Threading.Thread.Sleep(120);
        } finally {
            mods.Reverse();
            for (int i=0;i<mods.Count;i++) { var input=mods[i]; input.Flags=2; mods[i]=input; }
            if (mods.Count>0) SendInput((uint)mods.Count,mods.ToArray(),40);
        }
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
$taskPreviousFocus = [ShortcutProbe]::GetForegroundWindow()
$taskPreviousPointer = New-Object ShortcutProbe+Point
[ShortcutProbe]::GetCursorPos([ref]$taskPreviousPointer) | Out-Null
$taskDpi = [ShortcutProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskUserProcesses = @(Get-Process miniterm -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$taskShortcutPath = Join-Path $taskFixture 'settings.json'
$taskShortcut = @{schemaVersion=1;shortcut='';notifications=$false;profile=@{kind='git_bash';directory=$taskRoot};window=@{anchor=@{right=7200;bottom=500};miniWidth=560;miniHeight=112;miniView='input';autoCollapse=$false;transparency=0}}
[IO.File]::WriteAllText($taskShortcutPath,($taskShortcut | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
function New-ShortcutStart([string]$Arguments) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$taskExe; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.EnvironmentVariables['MINITERM_INSTANCE_ID']=$taskId
    $start.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
    $start.EnvironmentVariables['MINITERM_DATA_DIR']=$taskFixture
    $start.EnvironmentVariables['CODEX_HOME']=(Join-Path $taskFixture 'codex')
    $start.EnvironmentVariables['CLAUDE_CONFIG_DIR']=(Join-Path $taskFixture 'claude')
    return $start
}
function Invoke-Shortcut([string]$Arguments) {
    $start=New-ShortcutStart $Arguments; $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($start)
    try {
        $out=$p.StandardOutput.ReadToEndAsync(); $err=$p.StandardError.ReadToEndAsync()
        if (-not $p.WaitForExit(10000)) { $p.Kill(); throw 'Shortcut helper timed out' }
        if ($p.ExitCode -ne 0) { throw $err.Result }
        return $out.Result
    } finally { $p.Dispose() }
}
function Wait-Sessions([int]$Count) {
    $deadline=[DateTime]::UtcNow.AddSeconds(12)
    do {
        $snapshot=(Invoke-Shortcut 'sessions') | ConvertFrom-Json
        if (@($snapshot.sessions).Count -eq $Count) { return $snapshot }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Expected $Count sessions; got $(@($snapshot.sessions).Count)"
}
function Get-ShortcutWidget([string]$Name) {
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {
        $snapshot=(Invoke-Shortcut 'sessions') | ConvertFrom-Json
        $entry=$snapshot.details.ui.PSObject.Properties[$Name]
        if ($null -ne $entry) { return $entry.Value }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Missing feature widget: $Name"
}
function Click-Shortcut([string]$Name,[switch]$Right) {
    $r=Get-ShortcutWidget $Name
    $bounds=[ShortcutProbe]::Bounds($taskWindow)
    [ShortcutProbe]::SetCursorPos($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale),$bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) | Out-Null
    Start-Sleep -Milliseconds 80
    $pointer=New-Object ShortcutProbe+Point
    [ShortcutProbe]::GetCursorPos([ref]$pointer) | Out-Null
    [IO.File]::WriteAllText((Join-Path $taskFixture ('pointer-' + $Name + '.txt')),"expected=$($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale)),$($bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) actual=$($pointer.X),$($pointer.Y) hit=$([ShortcutProbe]::WindowFromPoint($pointer)) window=$taskWindow",[Text.Encoding]::UTF8)
    [ShortcutProbe]::Click($taskWindow,[int](($r[0]+$r[2])*0.5*$taskScale),[int](($r[1]+$r[3])*0.5*$taskScale),[bool]$Right)
    Start-Sleep -Milliseconds 200
}
function Save-ShortcutImage([IntPtr]$Window,[string]$Name) {
    $r=[ShortcutProbe]::Bounds($Window)
    $bitmap=New-Object Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top)
    $g=[Drawing.Graphics]::FromImage($bitmap)
    try { $g.CopyFromScreen($r.Left,$r.Top,0,0,$bitmap.Size); $bitmap.Save((Join-Path $taskFixture $Name)) }
    finally { $g.Dispose(); $bitmap.Dispose() }
}

function Send-Chord([byte]$Key,[bool]$Ctrl=$true,[bool]$Shift=$false,[bool]$Alt=$false,[IntPtr]$Window=$taskWindow) {
    [ShortcutProbe]::SetForegroundWindow($Window) | Out-Null
    Start-Sleep -Milliseconds 100
    if ([ShortcutProbe]::GetForegroundWindow() -ne $Window) {
        $bounds=[ShortcutProbe]::Bounds($Window)
        $scale=[ShortcutProbe]::GetDpiForWindow($Window)/96.0
        [ShortcutProbe]::SetCursorPos($bounds.Left+[int](12*$scale),$bounds.Top+[int](14*$scale)) | Out-Null
        $point=New-Object ShortcutProbe+Point
        [ShortcutProbe]::GetCursorPos([ref]$point) | Out-Null
        if ([ShortcutProbe]::WindowFromPoint($point) -ne $Window) { throw 'Owned window is covered at the focus point' }
        [ShortcutProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
        [ShortcutProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
        Start-Sleep -Milliseconds 150
    }
    if ($FixtureKeys) {
        $name=switch ($Key) {
            9 { 'Tab' } 13 { 'Enter' } 27 { 'Escape' } 33 { 'PageUp' } 34 { 'PageDown' }
            35 { 'End' } 36 { 'Home' } 38 { 'ArrowUp' } 40 { 'ArrowDown' }
            48 { '0' } 50 { '2' } 52 { '4' } 77 { 'M' } 84 { 'T' } 87 { 'W' }
            187 { if ($Shift) { 'Plus' } else { 'Equals' } } 188 { 'Comma' } 189 { 'Minus' }
            default { throw 'Unsupported fixture key' }
        }
        $parts=@(); if ($Ctrl) { $parts+='ctrl' }; if ($Shift) { $parts+='shift' }; if ($Alt) { $parts+='alt' }; $parts+=$name
        Invoke-Shortcut ('diagnostic key:' + ($parts -join '+')) | Out-Null
    } else {
        [ShortcutProbe]::Chord($Window,$Key,$Ctrl,$Shift,$Alt)
    }
    Start-Sleep -Milliseconds 250
}
function Assert-Selected([int]$Index) {
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {
        $ui=((Invoke-Shortcut 'sessions') | ConvertFrom-Json).details.ui
        $a=$ui.PSObject.Properties['selected-tab']; $b=$ui.PSObject.Properties['tab-' + $Index]
        if ($a -and $b -and (($a.Value -join ',') -eq ($b.Value -join ','))) { return }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Native shortcut did not select tab $Index"
}
function Assert-NoDialog {
    $ui=((Invoke-Shortcut 'sessions') | ConvertFrom-Json).details.ui
    if ($ui.PSObject.Properties['close-confirm']) { throw 'Escape did not cancel the confirmation' }
}
function Preferences { Get-Content -Raw -Encoding UTF8 $taskShortcutPath | ConvertFrom-Json }
$taskProcess=$null
$taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash;passed=$false;inputMode=$(if ($FixtureKeys) { 'egui_fixture' } else { 'native_SendInput' })}
try {
    $taskProcess=[Diagnostics.Process]::Start((New-ShortcutStart ''))
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100; $taskProcess.Refresh(); $taskWindow=$taskProcess.MainWindowHandle
        $ready=$false
        if ($taskWindow -ne [IntPtr]::Zero) { $bounds=[ShortcutProbe]::Bounds($taskWindow); $ready=($bounds.Right-$bounds.Left) -gt 100 }
    } while (-not $ready -and -not $taskProcess.HasExited -and [DateTime]::UtcNow -lt $deadline)
    if (-not $ready) { throw 'Shortcut app failed to start' }
    $taskScale=[ShortcutProbe]::GetDpiForWindow($taskWindow)/96.0
    $taskFirst=(Wait-Sessions 1).sessions[0]
    Start-Sleep -Milliseconds 700
    Send-Chord 84 $true $true
    $taskSecond=@((Wait-Sessions 2).sessions | Where-Object { $_ -ne $taskFirst })[0]
    Assert-Selected 1
    Send-Chord 84 $true $true
    $taskAll=(Wait-Sessions 3).sessions
    Assert-Selected 2
    $taskResult.newTab=$true
    Send-Chord 9
    Assert-Selected 0
    Send-Chord 9 $true $true
    Assert-Selected 2
    Send-Chord 50 $true $false $true
    Assert-Selected 1
    Send-Chord 52 $true $false $true
    Assert-Selected 1
    $taskResult.tabNavigationAndMissingIndex=$true
    Send-Chord 87
    Assert-NoDialog
    Wait-Sessions 3 | Out-Null
    $taskResult.plainCtrlWPreservesTabs=$true
    $taskBeforeFont=Get-ShortcutWidget 'terminal'
    Send-Chord 187
    if ((Preferences).fontSize -ne 15) { throw 'Ctrl+= did not enlarge and save the terminal font' }
    $taskAfterFont=Get-ShortcutWidget 'terminal'
    if ([Math]::Abs(($taskBeforeFont[2]-$taskBeforeFont[0])-($taskAfterFont[2]-$taskAfterFont[0])) -gt 1) { throw 'Font shortcut scaled the entire interface' }
    Send-Chord 189
    if ((Preferences).fontSize -ne 14) { throw 'Ctrl+- did not reduce the font' }
    Send-Chord 187 $true $true
    if ((Preferences).fontSize -ne 15) { throw 'Ctrl++ did not enlarge the font' }
    Send-Chord 48
    if ((Preferences).fontSize -ne 14) { throw 'Ctrl+0 did not restore the default font' }
    $taskResult.fontShortcutsAndNoUiZoom=$true
    Send-Chord 77 $true $false $true
    if ((Preferences).window.miniView -ne 'summary') { throw 'Mini view shortcut did not select summary' }
    Save-ShortcutImage $taskWindow 'brand-summary.png'
    Send-Chord 77 $true $false $true
    if ((Preferences).window.miniView -ne 'input') { throw 'Mini view shortcut did not restore input' }
    $taskResult.miniViewShortcut=$true
    Send-Chord 188
    $taskSettingsWindow=[ShortcutProbe]::Find($taskProcess.Id,'MiniTerm 설정')
    if ($taskSettingsWindow -eq [IntPtr]::Zero) { throw 'Ctrl+, did not open settings' }
    Save-ShortcutImage $taskSettingsWindow 'settings-clean.png'
    Send-Chord 27 $false $false $false $taskSettingsWindow
    if ([ShortcutProbe]::Find($taskProcess.Id,'MiniTerm 설정') -ne [IntPtr]::Zero) { throw 'Escape did not close settings' }
    $taskResult.settingsShortcut=$true
    $mini=[ShortcutProbe]::Bounds($taskWindow)
    Send-Chord 13 $true $false $true
    $expanded=[ShortcutProbe]::Bounds($taskWindow)
    if ($expanded.Bottom-$expanded.Top -le $mini.Bottom-$mini.Top -or $expanded.Right-$expanded.Left -ne $mini.Right-$mini.Left) { throw 'Expand shortcut changed the width or did not expand' }
    Start-Sleep -Milliseconds 350
    Invoke-Shortcut 'diagnostic sample-scroll' | Out-Null
    Start-Sleep -Milliseconds 250
    Send-Chord 36 $true $true
    Get-ShortcutWidget 'terminal-scrolled' | Out-Null
    Save-ShortcutImage $taskWindow 'scroll-history.png'
    Send-Chord 35 $true $true
    $ui=((Invoke-Shortcut 'sessions') | ConvertFrom-Json).details.ui
    if ($ui.PSObject.Properties['terminal-scrolled']) { throw 'Scroll to bottom did not return to the live screen' }
    Send-Chord 33 $true $true
    Get-ShortcutWidget 'terminal-scrolled' | Out-Null
    Send-Chord 34 $true $true
    Send-Chord 38 $true $true
    Get-ShortcutWidget 'terminal-scrolled' | Out-Null
    Send-Chord 40 $true $true
    Send-Chord 35 $true $true
    $taskResult.scrollHistoryShortcuts=$true
    Send-Chord 13 $true $false $true
    $collapsed=[ShortcutProbe]::Bounds($taskWindow)
    if ($collapsed.Bottom-$collapsed.Top -ne $mini.Bottom-$mini.Top) { throw 'Expand shortcut did not restore mini height' }
    $taskResult.expansionShortcut=$true
    if (((Wait-Sessions 3).sessions -join ',') -ne ($taskAll -join ',')) { throw 'Shortcuts replaced a running shell' }
    Send-Chord 87 $true $true
    Get-ShortcutWidget 'close-confirm' | Out-Null
    Send-Chord 27 $false
    Assert-NoDialog
    Wait-Sessions 3 | Out-Null
    Send-Chord 87 $true $true
    Get-ShortcutWidget 'close-confirm' | Out-Null
    Start-Sleep -Milliseconds 250
    Save-ShortcutImage $taskWindow 'shortcut-close-confirmation.png'
    Click-Shortcut 'close-confirm'
    $taskRemaining=(Wait-Sessions 2).sessions
    if ($taskRemaining -notcontains $taskFirst -or $taskRemaining -contains $taskSecond) { throw 'Close shortcut removed the wrong tab' }
    $taskResult.closeTabAndEscape=$true
    $taskIcon=[Drawing.Icon]::ExtractAssociatedIcon($taskExe)
    if ($null -eq $taskIcon) { throw 'Executable icon missing' }
    $bitmap=$taskIcon.ToBitmap()
    try { $bitmap.Save((Join-Path $taskFixture 'exe-icon.png')) } finally { $bitmap.Dispose(); $taskIcon.Dispose() }
    $taskResult.brandExeIcon=$true
    foreach ($id in $taskUserProcesses) { if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) { throw 'User MiniTerm was stopped' } }
    $taskResult.userSessionPreserved=$true
    $taskResult.passed=$true
    $taskResult.scope='Own isolated app/settings/IPC; inputMode distinguishes native SendInput from injected egui events. Tab selection and UUID preservation, confirmation/cancel, fonts, fixed VT history, mini view, fixed settings, executable icon. No ConPTY input compatibility, user shell/config change or model call. Clipboard contents are not modified by this test.'
} catch {
    $taskResult.error=$_.Exception.Message
    try { $taskResult.failureSnapshot=(Invoke-Shortcut 'sessions') | ConvertFrom-Json } catch {}
    try { Save-ShortcutImage $taskWindow 'failure.png' } catch {}
    throw
} finally {
    [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    if ($taskProcess -and -not $taskProcess.HasExited) { try { Invoke-Shortcut 'diagnostic quit' | Out-Null } catch {}; if (-not $taskProcess.WaitForExit(3000)) { $taskProcess.Kill(); $taskProcess.WaitForExit() } }
    [ShortcutProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    [ShortcutProbe]::SetCursorPos($taskPreviousPointer.X,$taskPreviousPointer.Y) | Out-Null
    [ShortcutProbe]::SetThreadDpiAwarenessContext($taskDpi) | Out-Null
}
$taskResult | ConvertTo-Json -Depth 6
