param([string]$Executable = 'artifacts/miniterm.exe')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExe = [IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
$taskId = [Guid]::NewGuid().ToString()
$taskFixture = Join-Path $taskRoot ('artifacts/settings-' + $taskId)
New-Item -ItemType Directory -Path $taskFixture | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class SettingsProbe {
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
$taskPreviousFocus = [SettingsProbe]::GetForegroundWindow()
$taskPreviousPointer = New-Object SettingsProbe+Point
[SettingsProbe]::GetCursorPos([ref]$taskPreviousPointer) | Out-Null
$taskDpi = [SettingsProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskUserProcesses = @(Get-Process miniterm -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$taskSettingsPath = Join-Path $taskFixture 'settings.json'
$taskSettings = @{schemaVersion=1;shortcut='';notifications=$false;profile=@{kind='git_bash';directory=$taskRoot};window=@{anchor=@{right=7200;bottom=500};miniWidth=560;miniHeight=112;autoCollapse=$false;transparency=0}}
[IO.File]::WriteAllText($taskSettingsPath,($taskSettings | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
function New-SettingsStart([string]$Arguments) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$taskExe; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.EnvironmentVariables['MINITERM_INSTANCE_ID']=$taskId
    $start.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
    $start.EnvironmentVariables['MINITERM_DATA_DIR']=$taskFixture
    $start.EnvironmentVariables['CODEX_HOME']=(Join-Path $taskFixture 'codex')
    $start.EnvironmentVariables['CLAUDE_CONFIG_DIR']=(Join-Path $taskFixture 'claude')
    return $start
}
function Invoke-Settings([string]$Arguments) {
    $start=New-SettingsStart $Arguments; $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($start)
    try {
        $out=$p.StandardOutput.ReadToEndAsync(); $err=$p.StandardError.ReadToEndAsync()
        if (-not $p.WaitForExit(10000)) { $p.Kill(); throw 'Settings helper timed out' }
        if ($p.ExitCode -ne 0) { throw $err.Result }
        return $out.Result
    } finally { $p.Dispose() }
}
function Wait-Sessions([int]$Count) {
    $deadline=[DateTime]::UtcNow.AddSeconds(12)
    do {
        $snapshot=(Invoke-Settings 'sessions') | ConvertFrom-Json
        if (@($snapshot.sessions).Count -eq $Count) { return $snapshot }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Expected $Count sessions; got $(@($snapshot.sessions).Count)"
}
function Get-SettingsWidget([string]$Name) {
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {
        $snapshot=(Invoke-Settings 'sessions') | ConvertFrom-Json
        $entry=$snapshot.details.ui.PSObject.Properties[$Name]
        if ($null -ne $entry) { return $entry.Value }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Missing feature widget: $Name"
}
function Click-Settings([string]$Name,[switch]$Right) {
    $r=Get-SettingsWidget $Name
    $bounds=[SettingsProbe]::Bounds($taskWindow)
    [SettingsProbe]::SetCursorPos($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale),$bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) | Out-Null
    Start-Sleep -Milliseconds 80
    $pointer=New-Object SettingsProbe+Point
    [SettingsProbe]::GetCursorPos([ref]$pointer) | Out-Null
    [IO.File]::WriteAllText((Join-Path $taskFixture ('pointer-' + $Name + '.txt')),"expected=$($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale)),$($bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) actual=$($pointer.X),$($pointer.Y) hit=$([SettingsProbe]::WindowFromPoint($pointer)) window=$taskWindow",[Text.Encoding]::UTF8)
    [SettingsProbe]::Click($taskWindow,[int](($r[0]+$r[2])*0.5*$taskScale),[int](($r[1]+$r[3])*0.5*$taskScale),[bool]$Right)
    Start-Sleep -Milliseconds 200
}
function Save-SettingsImage([IntPtr]$Window,[string]$Name) {
    $r=[SettingsProbe]::Bounds($Window)
    $bitmap=New-Object Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top)
    $g=[Drawing.Graphics]::FromImage($bitmap)
    try { $g.CopyFromScreen($r.Left,$r.Top,0,0,$bitmap.Size); $bitmap.Save((Join-Path $taskFixture $Name)) }
    finally { $g.Dispose(); $bitmap.Dispose() }
}

function Scroll-ToWidget([string]$Name) {
    for ($i=0; $i -lt 24; $i++) {
        $r=Get-SettingsWidget $Name
        $clip=Get-SettingsWidget 'settings-scroll'
        $y=($r[1]+$r[3])*0.5
        if ($y -ge $clip[1]+4 -and $y -le $clip[3]-4) { return }
        $bounds=[SettingsProbe]::Bounds($taskWindow)
        $x=$bounds.Left+[int](($clip[0]+$clip[2])*0.5*$taskScale)
        $screenY=$bounds.Top+[int](($clip[1]+$clip[3])*0.5*$taskScale)
        [SettingsProbe]::SetCursorPos($x,$screenY) | Out-Null
        $delta=if ($y -lt $clip[1]) { 120 } else { -120 }
        [SettingsProbe]::PostMessage($taskWindow,522,[IntPtr]($delta -shl 16),[IntPtr](($screenY -shl 16) -bor ($x -band 65535))) | Out-Null
        Start-Sleep -Milliseconds 180
    }
    throw "Could not scroll to settings widget: $Name"
}
function Click-Option([string]$Name) { Scroll-ToWidget $Name; Click-Settings $Name }
function Assert-Palette([int[]]$Expected) {
    $r=Get-SettingsWidget 'settings-window'; $bounds=[SettingsProbe]::Bounds($taskWindow)
    $image=New-Object Drawing.Bitmap 1,1; $g=[Drawing.Graphics]::FromImage($image)
    try {
        $g.CopyFromScreen($bounds.Left+[int](($r[0]+8)*$taskScale),$bounds.Top+[int](($r[1]+42)*$taskScale),0,0,$image.Size)
        $color=$image.GetPixel(0,0)
        if ($color.R -ne $Expected[0] -or $color.G -ne $Expected[1] -or $color.B -ne $Expected[2]) { throw "Settings palette mismatch: $color" }
    } finally { $g.Dispose(); $image.Dispose() }
}
function Assert-Open {
    $deadline=[DateTime]::UtcNow.AddSeconds(8)
    do {
        $script:taskWindow=[SettingsProbe]::Find($taskProcess.Id,'MiniTerm 설정')
        if ($taskWindow -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($taskWindow -eq [IntPtr]::Zero) { throw 'Independent settings window did not open' }
    $script:taskScale=[SettingsProbe]::GetDpiForWindow($taskWindow)/96.0
    Get-SettingsWidget 'settings-window' | Out-Null
    $ui=((Invoke-Settings 'sessions') | ConvertFrom-Json).details.ui
    if ($ui.PSObject.Properties['settings-save'] -or $ui.PSObject.Properties['settings-cancel']) { throw 'Save/cancel controls still present' }
}
function Read-Preferences { Get-Content -Raw -Encoding UTF8 $taskSettingsPath | ConvertFrom-Json }
function Assert-Closed {
    Start-Sleep -Milliseconds 250
    $ui=((Invoke-Settings 'sessions') | ConvertFrom-Json).details.ui
    if ($ui.PSObject.Properties['settings-window'] -or [SettingsProbe]::Find($taskProcess.Id,'MiniTerm 설정') -ne [IntPtr]::Zero) { throw 'X did not close settings' }
    $script:taskWindow=$taskHudWindow
    $script:taskScale=[SettingsProbe]::GetDpiForWindow($taskWindow)/96.0
}
New-Item -ItemType Directory -Path (Join-Path $taskFixture 'codex'),(Join-Path $taskFixture 'claude') | Out-Null
$taskCodexPath=Join-Path $taskFixture 'codex/config.toml'
$taskClaudePath=Join-Path $taskFixture 'claude/settings.json'
[IO.File]::WriteAllText($taskCodexPath,"# isolated existing callback`nnotify = ['fixture-callback', 'literal argument']`n",[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText($taskClaudePath,'{"theme":"dark","hooks":{"Stop":[{"hooks":[{"type":"command","command":"existing-fixture"}]}]}}',[Text.UTF8Encoding]::new($false))
$taskProcess=$null
$taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash;passed=$false}
try {
    $taskProcess=[Diagnostics.Process]::Start((New-SettingsStart ''))
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100; $taskProcess.Refresh(); $taskWindow=$taskProcess.MainWindowHandle
        $ready=$false
        if ($taskWindow -ne [IntPtr]::Zero) { $bounds=[SettingsProbe]::Bounds($taskWindow); $ready=($bounds.Right-$bounds.Left) -gt 100 }
    } while (-not $ready -and -not $taskProcess.HasExited -and [DateTime]::UtcNow -lt $deadline)
    if (-not $ready) { throw 'Settings app failed to start' }
    $taskScale=[SettingsProbe]::GetDpiForWindow($taskWindow)/96.0
    $taskHudWindow=$taskWindow
    $taskInitialHud=[SettingsProbe]::Bounds($taskHudWindow)
    $taskFirst=(Wait-Sessions 1).sessions[0]
    Invoke-Settings 'diagnostic settings' | Out-Null
    Start-Sleep -Milliseconds 300
    [SettingsProbe]::SetForegroundWindow($taskWindow) | Out-Null
    Assert-Open
    $taskSettingsBounds=[SettingsProbe]::Bounds($taskWindow)
    $taskSettingsWidth=($taskSettingsBounds.Right-$taskSettingsBounds.Left)/$taskScale
    $taskSettingsHeight=($taskSettingsBounds.Bottom-$taskSettingsBounds.Top)/$taskScale
    if ([Math]::Abs($taskSettingsWidth-560) -gt 1 -or [Math]::Abs($taskSettingsHeight-720) -gt 1) { throw 'Settings size is not fixed at 560x720 DIP' }
    $taskAfterOpenHud=[SettingsProbe]::Bounds($taskHudWindow)
    if ($taskAfterOpenHud.Right-$taskAfterOpenHud.Left -ne $taskInitialHud.Right-$taskInitialHud.Left -or $taskAfterOpenHud.Bottom-$taskAfterOpenHud.Top -ne $taskInitialHud.Bottom-$taskInitialHud.Top) { throw 'Opening settings resized the HUD' }
    Assert-Palette @(17,21,29)
    Save-SettingsImage $taskWindow 'terminal-dark.png'
    Click-Option 'settings-alternate-scroll'
    if (-not (Read-Preferences).alternateScroll) { throw 'Checkbox was not immediately saved' }
    Assert-Open
    Click-Option 'theme-light'
    if ((Read-Preferences).theme -ne 'light') { throw 'Light theme was not immediately saved' }
    Assert-Palette @(245,247,250)
    Save-SettingsImage $taskWindow 'terminal-light.png'
    Click-Settings 'settings-close'
    Assert-Closed
    if (-not (Read-Preferences).alternateScroll -or (Read-Preferences).theme -ne 'light') { throw 'X reverted preferences' }
    $taskResult.liveSettingsAndClose=$true
    Invoke-Settings 'diagnostic settings' | Out-Null
    Assert-Open
    Click-Settings 'settings-tab-2'
    Click-Option 'settings-notifications'
    if (-not (Read-Preferences).notifications) { throw 'Notification toggle was not immediately saved' }
    Click-Option 'settings-notification-sound'
    if (-not (Read-Preferences).notificationSound) { throw 'Sound toggle was not immediately saved' }
    $notifications=Get-SettingsWidget 'settings-notifications'; $sound=Get-SettingsWidget 'settings-notification-sound'
    if ($sound[1]-$notifications[3] -lt 8) { throw 'Notification controls remain crowded' }
    Save-SettingsImage $taskWindow 'notifications-light.png'
    Click-Option 'settings-integration-codex-true'
    Get-SettingsWidget 'settings-integration-feedback' | Out-Null
    if (-not (Get-Content -Raw -Encoding UTF8 $taskCodexPath).Contains('--chain-file')) { throw 'Codex registration failed' }
    Click-Option 'settings-integration-codex-false'
    $restored=Get-Content -Raw -Encoding UTF8 $taskCodexPath
    if (-not $restored.Contains('fixture-callback') -or $restored.Contains('--chain-file')) { throw 'Codex unregister did not restore existing callback' }
    Click-Option 'settings-integration-claude-true'
    $hooks=Get-Content -Raw -Encoding UTF8 $taskClaudePath | ConvertFrom-Json
    if (@($hooks.hooks.Stop).Count -ne 2) { throw 'Claude registration failed' }
    Click-Option 'settings-integration-claude-false'
    $hooks=Get-Content -Raw -Encoding UTF8 $taskClaudePath | ConvertFrom-Json
    if (@($hooks.hooks.Stop).Count -ne 1 -or $hooks.hooks.Stop[0].hooks[0].command -ne 'existing-fixture') { throw 'Claude unregister removed an existing hook' }
    $taskResult.notificationOptionsAndRegistrations=$true
    Click-Settings 'settings-tab-4'
    Click-Option 'settings-codex-bash-true'
    $taskEnvironments=Join-Path $taskFixture 'codex/environments.toml'
    if(-not (Test-Path -LiteralPath $taskEnvironments) -or -not (Get-Content -LiteralPath $taskEnvironments -Raw -Encoding UTF8).Contains('default = "miniterm-git-bash"')){throw 'Codex Bash environment was not registered by the settings button.'}
    if(-not (Get-Content -LiteralPath $taskCodexPath -Raw -Encoding UTF8).Contains('fixture-callback')){throw 'Bash registration changed the notification callback.'}
    Save-SettingsImage $taskWindow 'codex-bash-settings.png'
    Click-Option 'settings-codex-bash-false'
    $taskEnvironmentAfter=Get-Content -LiteralPath $taskEnvironments -Raw -Encoding UTF8
    if($taskEnvironmentAfter -match 'miniterm-git-bash' -or (Test-Path -LiteralPath (Join-Path $taskFixture 'codex/miniterm-bash.json'))){throw 'Codex Bash environment was not removed by the settings button.'}
    $taskResult.codexBashRegistrationAndRemoval=$true
    Click-Settings 'settings-tab-1'
    Scroll-ToWidget 'settings-transparency'
    $r=Get-SettingsWidget 'settings-transparency'; $bounds=[SettingsProbe]::Bounds($taskWindow)
    $x=[int](($r[0]+70)*$taskScale); $y=[int](($r[1]+$r[3])*0.5*$taskScale)
    [SettingsProbe]::SetCursorPos($bounds.Left+$x,$bounds.Top+$y) | Out-Null
    [SettingsProbe]::Click($taskWindow,$x,$y)
    Start-Sleep -Milliseconds 180
    $opacity=(Read-Preferences).window.transparency
    if ($opacity -le 0) { throw 'Settings transparency slider did not immediately save' }
    $key=[uint32]0; $alpha=[byte]0; $flags=[uint32]0
    if (-not [SettingsProbe]::GetLayeredWindowAttributes($taskHudWindow,[ref]$key,[ref]$alpha,[ref]$flags) -or [Math]::Abs($alpha-[Math]::Round(255*(1-$opacity/100.0))) -gt 1) { throw 'Settings transparency was not applied to the native HUD' }
    $r=Get-SettingsWidget 'settings-transparency'; $bounds=[SettingsProbe]::Bounds($taskWindow)
    $x=[int](($r[0]+1)*$taskScale); $y=[int](($r[1]+$r[3])*0.5*$taskScale)
    [SettingsProbe]::SetCursorPos($bounds.Left+$x,$bounds.Top+$y) | Out-Null
    [SettingsProbe]::Click($taskWindow,$x,$y)
    Start-Sleep -Milliseconds 180
    if ((Read-Preferences).window.transparency -ne 0) { throw 'Transparency could not be restored to zero' }
    $taskResult.liveTransparency=$true
    Click-Option 'settings-shortcut'
    [SettingsProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    [SettingsProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    [SettingsProbe]::Text($taskWindow,'invalid')
    Get-SettingsWidget 'settings-error' | Out-Null
    if ((Read-Preferences).shortcut -ne '') { throw 'Invalid shortcut overwrote last accepted value' }
    Click-Settings 'settings-tab-2'
    Click-Option 'settings-notifications'
    if ((Read-Preferences).notifications) { throw 'Unfinished shortcut prevented unrelated notification change' }
    Get-SettingsWidget 'settings-error' | Out-Null
    $taskResult.invalidInputPreservesOtherControls=$true
    Click-Settings 'settings-tab-3'
    Click-Option 'settings-prompt-enabled'
    if ((Read-Preferences).prompt.enabled) { throw 'Prompt option was not immediately saved' }
    $states=@(Get-ChildItem -LiteralPath $taskFixture -Recurse -File -Filter state)
    if ($states.Count -ne 1 -or (Get-Content -Encoding UTF8 $states[0].FullName | Select-Object -First 1) -ne '0') { throw 'Open session prompt was not updated' }
    if ((Wait-Sessions 1).sessions[0] -ne $taskFirst) { throw 'Settings restarted the shell' }
    Save-SettingsImage $taskWindow 'prompt-light.png'
    $taskResult.openSessionPromptAndIdentity=$true
    Click-Settings 'settings-tab-0'
    Click-Option 'theme-dark'
    Assert-Palette @(17,21,29)
    Click-Settings 'settings-close'
    Assert-Closed
    Invoke-Settings 'diagnostic quit' | Out-Null
    if (-not $taskProcess.WaitForExit(5000)) { throw 'Own fixture failed to quit' }
    $taskProcess.Dispose()
    $taskProcess=[Diagnostics.Process]::Start((New-SettingsStart ''))
    Wait-Sessions 1 | Out-Null
    Start-Sleep -Milliseconds 350
    $taskProcess.Refresh(); $taskWindow=$taskProcess.MainWindowHandle
    $taskHudWindow=$taskWindow
    $taskScale=[SettingsProbe]::GetDpiForWindow($taskWindow)/96.0
    Invoke-Settings 'diagnostic settings' | Out-Null
    Assert-Open
    $saved=Read-Preferences
    if ($saved.theme -ne 'dark' -or -not $saved.alternateScroll -or $saved.notifications -or -not $saved.notificationSound -or $saved.prompt.enabled -or $saved.shortcut -ne '') { throw 'Restart lost live settings' }
    $taskResult.restartPersistence=$true
    $bounds=[SettingsProbe]::Bounds($taskHudWindow)
    $hudScale=[SettingsProbe]::GetDpiForWindow($taskHudWindow)/96.0
    [SettingsProbe]::SetWindowPos($taskHudWindow,[IntPtr]::Zero,$bounds.Left,$bounds.Top,[int](320*$hudScale),[int](112*$hudScale),20) | Out-Null
    Start-Sleep -Milliseconds 250
    $root=[SettingsProbe]::Bounds($taskWindow); $dialog=Get-SettingsWidget 'settings-window'; $close=Get-SettingsWidget 'settings-close'
    $clientWidth=($root.Right-$root.Left)/$taskScale
    if ([Math]::Abs($clientWidth-560) -gt 1 -or [Math]::Abs(($root.Bottom-$root.Top)/$taskScale-720) -gt 1) { throw 'Small HUD resized the settings window' }
    if ($dialog[0] -lt 0 -or $dialog[2] -gt $clientWidth -or $close[2] -gt $clientWidth) { throw 'Small-window settings or close button are clipped' }
    Save-SettingsImage $taskWindow 'small-settings.png'
    foreach ($tab in @(1,2,3)) {
        Click-Settings ('settings-tab-' + $tab)
        if ($tab -eq 3) { Click-Option 'settings-prompt-enabled' }
        $dialog=Get-SettingsWidget 'settings-window'; $close=Get-SettingsWidget 'settings-close'
        if ($dialog[0] -lt 0 -or $dialog[2] -gt $clientWidth -or $close[2] -gt $clientWidth) { throw "Small-window tab $tab is clipped" }
        Save-SettingsImage $taskWindow ('small-tab-' + $tab + '.png')
        if ($tab -eq 3) { Click-Option 'settings-prompt-enabled' }
    }
    Click-Settings 'settings-close'
    Assert-Closed
    $taskResult.smallWindow=$true
    $taskResult.fixedSettingsSize=@{widthDip=560;heightDip=720;independentOfHud=$true;openingKeepsHudSize=$true}
    Invoke-Settings 'diagnostic hide' | Out-Null
    Invoke-Settings 'diagnostic settings' | Out-Null
    Assert-Open
    if ([SettingsProbe]::IsWindowVisible($taskHudWindow)) { throw 'Opening settings unnecessarily showed the hidden HUD' }
    Click-Settings 'settings-close'
    Assert-Closed
    if ($taskProcess.HasExited) { throw 'Closing settings stopped MiniTerm' }
    $taskResult.hiddenHudSettings=$true
    foreach ($id in $taskUserProcesses) { if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) { throw 'User MiniTerm was stopped' } }
    $taskResult.userSessionPreserved=$true
    $taskResult.passed=$true
    $taskResult.scope='Own isolated app/settings/Codex/Claude config; actual native mouse clicks, native text, immediate persistence, X/reopen/restart, prompt state and shell UUID. No model call, user CLI config change, Windows registration action or real toast-click verification.'
} catch {
    $taskResult.error=$_.Exception.Message
    try { $taskResult.failureSnapshot=(Invoke-Settings 'sessions') | ConvertFrom-Json } catch {}
    try { Save-SettingsImage $taskWindow 'failure.png' } catch {}
    throw
} finally {
    [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
    if ($taskProcess -and -not $taskProcess.HasExited) { try { Invoke-Settings 'diagnostic quit' | Out-Null } catch {}; if (-not $taskProcess.WaitForExit(3000)) { $taskProcess.Kill(); $taskProcess.WaitForExit() } }
    [SettingsProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    [SettingsProbe]::SetCursorPos($taskPreviousPointer.X,$taskPreviousPointer.Y) | Out-Null
    [SettingsProbe]::SetThreadDpiAwarenessContext($taskDpi) | Out-Null
}
$taskResult | ConvertTo-Json -Depth 5
