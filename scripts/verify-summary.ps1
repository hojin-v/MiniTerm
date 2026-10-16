param([string]$Executable = 'artifacts/miniterm.exe')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExe = [IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
$taskId = [Guid]::NewGuid().ToString()
$taskFixture = Join-Path $taskRoot ('artifacts/summary-' + $taskId)
New-Item -ItemType Directory -Path $taskFixture | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class SummaryProbe {
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
$taskPreviousFocus = [SummaryProbe]::GetForegroundWindow()
$taskPreviousPointer = New-Object SummaryProbe+Point
[SummaryProbe]::GetCursorPos([ref]$taskPreviousPointer) | Out-Null
$taskDpi = [SummaryProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskUserProcesses = @(Get-Process miniterm -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$taskSummaryPath = Join-Path $taskFixture 'settings.json'
$taskSummary = @{schemaVersion=1;shortcut='';notifications=$false;profile=@{kind='git_bash';directory=$taskRoot};window=@{anchor=@{right=7200;bottom=500};miniWidth=560;miniHeight=112;miniView='summary';autoCollapse=$false;transparency=0}}
[IO.File]::WriteAllText($taskSummaryPath,($taskSummary | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
function New-SummaryStart([string]$Arguments) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$taskExe; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.EnvironmentVariables['MINITERM_INSTANCE_ID']=$taskId
    $start.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
    $start.EnvironmentVariables['MINITERM_DATA_DIR']=$taskFixture
    $start.EnvironmentVariables['CODEX_HOME']=(Join-Path $taskFixture 'codex')
    $start.EnvironmentVariables['CLAUDE_CONFIG_DIR']=(Join-Path $taskFixture 'claude')
    return $start
}
function Invoke-Summary([string]$Arguments) {
    $start=New-SummaryStart $Arguments; $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($start)
    try {
        $out=$p.StandardOutput.ReadToEndAsync(); $err=$p.StandardError.ReadToEndAsync()
        if (-not $p.WaitForExit(10000)) { $p.Kill(); throw 'Summary helper timed out' }
        if ($p.ExitCode -ne 0) { throw $err.Result }
        return $out.Result
    } finally { $p.Dispose() }
}
function Wait-Sessions([int]$Count) {
    $deadline=[DateTime]::UtcNow.AddSeconds(12)
    do {
        $snapshot=(Invoke-Summary 'sessions') | ConvertFrom-Json
        if (@($snapshot.sessions).Count -eq $Count) { return $snapshot }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Expected $Count sessions; got $(@($snapshot.sessions).Count)"
}
function Get-SummaryWidget([string]$Name) {
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {
        $snapshot=(Invoke-Summary 'sessions') | ConvertFrom-Json
        $entry=$snapshot.details.ui.PSObject.Properties[$Name]
        if ($null -ne $entry) { return $entry.Value }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Missing feature widget: $Name"
}
function Click-Summary([string]$Name,[switch]$Right) {
    $r=Get-SummaryWidget $Name
    $bounds=[SummaryProbe]::Bounds($taskWindow)
    [SummaryProbe]::SetCursorPos($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale),$bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) | Out-Null
    Start-Sleep -Milliseconds 80
    $pointer=New-Object SummaryProbe+Point
    [SummaryProbe]::GetCursorPos([ref]$pointer) | Out-Null
    [IO.File]::WriteAllText((Join-Path $taskFixture ('pointer-' + $Name + '.txt')),"expected=$($bounds.Left+[int](($r[0]+$r[2])*0.5*$taskScale)),$($bounds.Top+[int](($r[1]+$r[3])*0.5*$taskScale)) actual=$($pointer.X),$($pointer.Y) hit=$([SummaryProbe]::WindowFromPoint($pointer)) window=$taskWindow",[Text.Encoding]::UTF8)
    [SummaryProbe]::Click($taskWindow,[int](($r[0]+$r[2])*0.5*$taskScale),[int](($r[1]+$r[3])*0.5*$taskScale),[bool]$Right)
    Start-Sleep -Milliseconds 200
}
function Save-SummaryImage([IntPtr]$Window,[string]$Name) {
    $r=[SummaryProbe]::Bounds($Window)
    $bitmap=New-Object Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top)
    $g=[Drawing.Graphics]::FromImage($bitmap)
    try { $g.CopyFromScreen($r.Left,$r.Top,0,0,$bitmap.Size); $bitmap.Save((Join-Path $taskFixture $Name)) }
    finally { $g.Dispose(); $bitmap.Dispose() }
}


function Invoke-Report([string]$Action,$Report=$null,[string]$WorkId='',[long]$Revision=0,[switch]$Reject,[switch]$Skill) {
    $arguments='progress ' + $Action
    if ($WorkId) { $arguments += ' ' + $WorkId + ' ' + $Revision }
    $start=New-SummaryStart $arguments
    $start.EnvironmentVariables['MINITERM_SESSION_ID']=$taskSession
    $start.EnvironmentVariables['MINITERM_NOTIFY_EXE']=$taskExe
    if ($Skill) {
        $start.FileName=(Get-Command python -ErrorAction Stop).Source
        $start.Arguments='"' + (Join-Path $taskRoot '.agents/skills/miniterm-progress/scripts/report.py') + '" ' + $Action
    }
    $start.RedirectStandardInput=$true; $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($start)
    try {
        $out=$p.StandardOutput.ReadToEndAsync(); $err=$p.StandardError.ReadToEndAsync()
        if ($null -ne $Report) {
            $bytes=[Text.UTF8Encoding]::new($false).GetBytes(($Report | ConvertTo-Json -Depth 8 -Compress))
            $p.StandardInput.BaseStream.Write($bytes,0,$bytes.Length)
        }
        $p.StandardInput.Close()
        if (-not $p.WaitForExit(10000)) { $p.Kill(); throw 'Progress helper timed out' }
        if ($Reject) {
            if ($p.ExitCode -eq 0) { throw 'Invalid progress was accepted' }
            return
        }
        if ($p.ExitCode -ne 0) { throw $err.Result }
        return ($out.Result | ConvertFrom-Json)
    } finally { $p.Dispose() }
}
function Assert-Progress([int]$Percent) {
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {
        $track=Get-SummaryWidget 'summary-progress'
        $fill=Get-SummaryWidget 'summary-progress-fill'
        $actual=100*($fill[2]-$fill[0])/($track[2]-$track[0])
        if ([Math]::Abs($actual-$Percent) -lt 0.1) { return }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Rendered progress was $actual%, expected $Percent%"
}
function Hover-Summary([string]$Name,[int]$TooltipCount) {
    $r=Get-SummaryWidget $Name; $bounds=[SummaryProbe]::Bounds($taskWindow)
    $x=[int](($r[0]+$r[2])*0.5*$taskScale); $y=[int](($r[1]+$r[3])*0.5*$taskScale)
    [SummaryProbe]::SetForegroundWindow($taskWindow) | Out-Null
    [SummaryProbe]::SetCursorPos($bounds.Left+5,$bounds.Top+100) | Out-Null
    Start-Sleep -Milliseconds 150
    [SummaryProbe]::SetCursorPos($bounds.Left+$x,$bounds.Top+$y) | Out-Null
    $pointer=New-Object SummaryProbe+Point
    [SummaryProbe]::GetCursorPos([ref]$pointer) | Out-Null
    $hit=[SummaryProbe]::WindowFromPoint($pointer)
    [IO.File]::WriteAllText((Join-Path $taskFixture ('hover-' + $Name + '-' + $TooltipCount + '.txt')),"actual=$($pointer.X),$($pointer.Y) hit=$hit own=$taskWindow focused=$([SummaryProbe]::GetForegroundWindow())",[Text.Encoding]::UTF8)
    if ($hit -ne $taskWindow) { throw 'The pointer did not reach the owned summary' }
    [SummaryProbe]::PostMessage($taskWindow,512,[IntPtr]::Zero,[IntPtr](($y -shl 16) -bor ($x -band 65535))) | Out-Null
    Start-Sleep -Milliseconds 850
    $ui=((Invoke-Summary 'sessions') | ConvertFrom-Json).details.ui
    $count=@($ui.PSObject.Properties | Where-Object Name -Like 'fixture-tooltip-*').Count
    if ($count -ne $TooltipCount) { throw "Hover $Name created $count tooltips, expected $TooltipCount" }
}
function Assert-DialogButtons {
    $dialog=Get-SummaryWidget 'close-dialog'; $cancel=Get-SummaryWidget 'close-cancel'; $confirm=Get-SummaryWidget 'close-confirm'
    if ($confirm[2]-$confirm[0] -lt 120 -or $cancel[2]-$cancel[0] -lt 120) { throw 'Confirmation buttons remain too narrow' }
    if ($cancel[0]-$dialog[0] -gt 16 -or $dialog[2]-$confirm[2] -gt 16 -or $dialog[3]-$confirm[3] -gt 16) { throw 'Confirmation retains excessive margins' }
    $bounds=[SummaryProbe]::Bounds($taskWindow)
    $image=New-Object Drawing.Bitmap 1,1; $g=[Drawing.Graphics]::FromImage($image)
    try {
        $g.CopyFromScreen($bounds.Left+[int](($confirm[0]+8)*$taskScale),$bounds.Top+[int](($confirm[1]+8)*$taskScale),0,0,$image.Size)
        $color=$image.GetPixel(0,0)
        if ($color.R -gt $color.G+12) { throw 'Confirmation button still uses red styling' }
    } finally { $g.Dispose(); $image.Dispose() }
}
$taskProcess=$null
$taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash;passed=$false}
try {
    $taskProcess=[Diagnostics.Process]::Start((New-SummaryStart ''))
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100; $taskProcess.Refresh(); $taskWindow=$taskProcess.MainWindowHandle
        $ready=$false
        if ($taskWindow -ne [IntPtr]::Zero) { $bounds=[SummaryProbe]::Bounds($taskWindow); $ready=($bounds.Right-$bounds.Left) -gt 100 }
    } while (-not $ready -and -not $taskProcess.HasExited -and [DateTime]::UtcNow -lt $deadline)
    if (-not $ready) { throw 'Summary app failed to start' }
    $taskScale=[SummaryProbe]::GetDpiForWindow($taskWindow)/96.0
    $taskSession=(Wait-Sessions 1).sessions[0]
    Start-Sleep -Milliseconds 700
    Get-SummaryWidget 'summary-unreported' | Out-Null
    Hover-Summary 'summary-unreported' 0
    $report=@{title='작업 요약 검증';state='running';tasks=@(
        @{text='화면 분석';state='completed'},
        @{text='진행 표시 확인';state='running'},
        @{text='회귀 검사';state='pending'}
    )}
    $receipt=Invoke-Report 'begin' $report -Skill
    Assert-Progress 33
    Get-SummaryWidget 'summary-task' | Out-Null
    $read=Invoke-Report 'read'
    if ($read.report.tasks[1].text -ne '진행 표시 확인') { throw 'Skill report lost its Korean current task' }
    Hover-Summary 'summary-task' 0
    Save-SummaryImage $taskWindow 'progress-33.png'
    $long='진행 표시 확인: ' + ('긴 작업 설명을 생략 없이 확인합니다. ' * 7)
    $report.tasks[1].text=$long
    $receipt=Invoke-Report 'update' $report $receipt.workId ($receipt.revision+1)
    Assert-Progress 33
    Hover-Summary 'summary-task' 1
    Save-SummaryImage $taskWindow 'long-task-one-tooltip.png'
    $taskResult.tooltips=@{visibleText=0;clippedText=1;noPlanText=0}
    $report.tasks[1].text='진행 표시 확인'
    $report.tasks[1].state='completed'; $report.tasks[2].state='running'
    $receipt=Invoke-Report 'update' $report $receipt.workId ($receipt.revision+1)
    Assert-Progress 66
    Save-SummaryImage $taskWindow 'progress-66.png'
    $report.state='paused'
    $receipt=Invoke-Report 'update' $report $receipt.workId ($receipt.revision+1)
    Assert-Progress 66
    Save-SummaryImage $taskWindow 'progress-paused.png'
    $report.state='completed'
    Invoke-Report 'update' $report $receipt.workId ($receipt.revision+1) -Reject
    Assert-Progress 66
    $report.tasks[2].state='completed'
    $receipt=Invoke-Report 'update' $report $receipt.workId ($receipt.revision+1)
    Assert-Progress 100
    Get-SummaryWidget 'summary-completed' | Out-Null
    Save-SummaryImage $taskWindow 'progress-100.png'
    $taskResult.renderedProgress=@(33,66,100)
    $taskResult.skillAdapterAndCurrentTask=$true
    $taskResult.pausedAndFalseCompletion=$true
    Invoke-Summary 'diagnostic new-session' | Out-Null
    $sessions=Wait-Sessions 2; $taskOther=@($sessions.sessions | Where-Object { $_ -ne $taskSession })[0]
    Get-SummaryWidget 'summary-unreported' | Out-Null
    $ui=((Invoke-Summary 'sessions') | ConvertFrom-Json).details.ui
    if ($ui.PSObject.Properties['summary-progress']) { throw 'Plan leaked into another tab' }
    Invoke-Summary ('activate ' + $taskSession) | Out-Null
    Invoke-Summary 'diagnostic collapse' | Out-Null
    Start-Sleep -Milliseconds 250
    Assert-Progress 100
    Invoke-Report 'clear' $null $receipt.workId ($receipt.revision+1) | Out-Null
    Get-SummaryWidget 'summary-unreported' | Out-Null
    $ui=((Invoke-Summary 'sessions') | ConvertFrom-Json).details.ui
    if ($ui.PSObject.Properties['summary-progress']) { throw 'Cleared plan retained progress' }
    $taskResult.sessionIsolationAndClear=$true
    Click-Summary 'tab-1' -Right
    Get-SummaryWidget 'close-dialog' | Out-Null
    Start-Sleep -Milliseconds 250
    Assert-DialogButtons
    Save-SummaryImage $taskWindow 'close-tab.png'
    Click-Summary 'close-cancel'
    if (@((Wait-Sessions 2).sessions).Count -ne 2) { throw 'Cancel closed a shell' }
    Click-Summary 'tab-1' -Right
    Click-Summary 'close-confirm'
    if ((Wait-Sessions 1).sessions[0] -ne $taskSession) { throw 'Tab close removed the active shell' }
    $taskResult.closeTabAndCancel=$true
    Invoke-Summary 'diagnostic confirm-quit' | Out-Null
    Get-SummaryWidget 'close-dialog' | Out-Null
    Start-Sleep -Milliseconds 250
    Assert-DialogButtons
    Save-SummaryImage $taskWindow 'close-all.png'
    Click-Summary 'close-cancel'
    if ($taskProcess.HasExited -or (Wait-Sessions 1).sessions[0] -ne $taskSession) { throw 'Cancel quit the app or changed the shell' }
    Invoke-Summary 'diagnostic confirm-quit' | Out-Null
    Click-Summary 'close-confirm'
    if (-not $taskProcess.WaitForExit(5000)) { throw 'Close all failed to quit the owned app' }
    $taskResult.closeAllAndCancel=$true
    foreach ($id in $taskUserProcesses) { if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) { throw 'User MiniTerm was stopped' } }
    $taskResult.userSessionPreserved=$true
    $taskResult.passed=$true
    $taskResult.scope='Own native app/helper/IPC and actual project skill adapter; renders explicit fixture plan, verifies bar width and hover behavior, real tab/app closure and cancellation. No model call or automatic live-agent reporting guarantee.'
} catch {
    $taskResult.error=$_.Exception.Message
    try { $taskResult.failureSnapshot=(Invoke-Summary 'sessions') | ConvertFrom-Json } catch {}
    try { Save-SummaryImage $taskWindow 'failure.png' } catch {}
    throw
} finally {
    [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    if ($taskProcess -and -not $taskProcess.HasExited) { try { Invoke-Summary 'diagnostic quit' | Out-Null } catch {}; if (-not $taskProcess.WaitForExit(3000)) { $taskProcess.Kill(); $taskProcess.WaitForExit() } }
    [SummaryProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    [SummaryProbe]::SetCursorPos($taskPreviousPointer.X,$taskPreviousPointer.Y) | Out-Null
    [SummaryProbe]::SetThreadDpiAwarenessContext($taskDpi) | Out-Null
}
$taskResult | ConvertTo-Json -Depth 6
