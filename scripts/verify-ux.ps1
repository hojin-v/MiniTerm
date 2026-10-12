param([string]$Executable = 'artifacts/miniterm.exe', [ValidateRange(0,80)][int]$Transparency = 0, [string]$FontFile = '', [string]$BashPath = '', [switch]$GlyphSample, [switch]$SummarySample, [switch]$ProgressSample, [ValidateSet(0,10,30,60)][int]$IdleSeconds = 0, [ValidateRange(1,4)][int]$SessionCount = 1)
$ErrorActionPreference = 'Stop'
if ($ProgressSample) { $SummarySample = $true }
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExecutable = [IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
$taskExplicitBash = $null
$taskRuntimeHash = $null
if ($BashPath) {
    $taskExplicitBash = (Get-Item -LiteralPath $BashPath).FullName
    if ([IO.Path]::GetFileName($taskExplicitBash) -ine 'bash.exe') { throw 'Expected an explicit bash.exe.' }
    $taskRuntimeDll = Join-Path (Split-Path -Parent $taskExplicitBash) 'msys-2.0.dll'
    $taskRuntimeHash = (Get-FileHash -LiteralPath $taskRuntimeDll -Algorithm SHA256).Hash.ToLowerInvariant()
}
$taskId = [Guid]::NewGuid().ToString()
$taskFixture = Join-Path $taskRoot ('artifacts/ux-' + $taskId)
New-Item -ItemType Directory -Path $taskFixture | Out-Null
Add-Type -AssemblyName System.Drawing
if (-not ('MiniTermUxProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MiniTermUxProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct CursorPoint { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out CursorPoint point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window,out Rect rect);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool GetLayeredWindowAttributes(IntPtr window,out uint key,out byte alpha,out uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window,IntPtr after,int x,int y,int w,int h,uint flags);
    [DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr window,IntPtr region);
    [DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int l,int t,int r,int b);
    [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr obj);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr window,uint attribute,out int value,int size);
    public static Rect Bounds(IntPtr window) { Rect r; if (!GetWindowRect(window,out r)) throw new Exception("Missing window"); return r; }
    public static IntPtr Point(int x,int y) { return (IntPtr)((y << 16) | (x & 65535)); }
    public static void Click(IntPtr window,int x,int y) {
        var p=Point(x,y); SendMessage(window,512,IntPtr.Zero,p);
        SendMessage(window,513,(IntPtr)1,p); System.Threading.Thread.Sleep(55);
        SendMessage(window,514,IntPtr.Zero,p); System.Threading.Thread.Sleep(55);
    }
    public static void ClickAsync(IntPtr window,int x,int y) {
        var p=Point(x,y); PostMessage(window,512,IntPtr.Zero,p); PostMessage(window,513,(IntPtr)1,p);
        System.Threading.ThreadPool.QueueUserWorkItem(_ => { System.Threading.Thread.Sleep(55); PostMessage(window,514,IntPtr.Zero,p); });
    }
}
'@
}
$taskFocus = [MiniTermUxProbe]::GetForegroundWindow()
$taskDpiContext = [MiniTermUxProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskUserProcesses = @(Get-Process miniterm -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$taskSettings = @{schemaVersion=1; theme='dark'; fontFamily=$FontFile; shortcut=''; notifications=$false; profile=@{kind='git_bash';directory=$taskRoot}; prompt=@{userLabel='dev-한글';showHost=$false}; window=@{anchor=@{right=7200;bottom=57};miniWidth=560;miniHeight=112;transparency=$Transparency;bodyDoubleClickExpands=$false}}
if ($taskExplicitBash) { $taskSettings.profile.bash_path = $taskExplicitBash }
[IO.File]::WriteAllText((Join-Path $taskFixture 'settings.json'),($taskSettings | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
function New-ProbeStart([string]$Arguments) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$taskExecutable; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
    $start.EnvironmentVariables['MINITERM_INSTANCE_ID']=$taskId
    $start.EnvironmentVariables['MINITERM_DATA_DIR']=$taskFixture
    $start.EnvironmentVariables['MINITERM_METRICS_PATH']=(Join-Path $taskFixture 'metrics.json')
    return $start
}
function Invoke-Probe([string]$Arguments) {
    $start=New-ProbeStart $Arguments; $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($start)
    try { if (-not $p.WaitForExit(10000)) { $p.Kill(); throw 'Diagnostic helper timed out.' }; $text=$p.StandardOutput.ReadToEnd(); if ($p.ExitCode -ne 0) { throw $p.StandardError.ReadToEnd() }; return $text }
    finally { $p.Dispose() }
}
function Get-Widget([string]$Name) {
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {
        $snapshot=(Invoke-Probe 'sessions') | ConvertFrom-Json
        $entry=$snapshot.details.ui.PSObject.Properties[$Name]
        if ($null -ne $entry) { return $entry.Value }
        Start-Sleep -Milliseconds 70
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Widget not visible: $Name"
}
function Invoke-Progress([string]$Action, $Report = $null, [string]$WorkId = '', [long]$Revision = 0, [switch]$Reject) {
    $taskArgs = 'progress ' + $Action
    if ($WorkId) { $taskArgs += ' ' + $WorkId + ' ' + $Revision }
    $taskStart = New-ProbeStart $taskArgs
    $taskStart.EnvironmentVariables['MINITERM_SESSION_ID'] = $taskProgressSession
    $taskStart.RedirectStandardInput = $true
    $taskStart.RedirectStandardOutput = $true
    $taskStart.RedirectStandardError = $true
    $taskHelper = [Diagnostics.Process]::Start($taskStart)
    try {
        $taskOutput = $taskHelper.StandardOutput.ReadToEndAsync()
        $taskError = $taskHelper.StandardError.ReadToEndAsync()
        if ($null -ne $Report) {
            $taskBytes = [Text.UTF8Encoding]::new($false).GetBytes(($Report | ConvertTo-Json -Depth 8 -Compress))
            $taskHelper.StandardInput.BaseStream.Write($taskBytes, 0, $taskBytes.Length)
        }
        $taskHelper.StandardInput.Close()
        if (-not $taskHelper.WaitForExit(10000)) { $taskHelper.Kill(); throw 'Progress helper timed out.' }
        if ($Reject) {
            if ($taskHelper.ExitCode -eq 0) { throw 'Invalid/stale progress was accepted.' }
            return
        }
        if ($taskHelper.ExitCode -ne 0) { throw $taskError.Result }
        return ($taskOutput.Result | ConvertFrom-Json)
    } finally { $taskHelper.Dispose() }
}
function Click-Widget([string]$Name) {
    $r=Get-Widget $Name
    [MiniTermUxProbe]::Click($taskWindow,[int](($r[0]+$r[2])*0.5*$taskScale),[int](($r[1]+$r[3])*0.5*$taskScale))
    Start-Sleep -Milliseconds 160
}
function Save-ProbeImage([string]$Name) {
    $r=[MiniTermUxProbe]::Bounds($taskWindow)
    $bitmap=New-Object Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top)
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    try { $graphics.CopyFromScreen($r.Left,$r.Top,0,0,$bitmap.Size); $bitmap.Save((Join-Path $taskFixture $Name)) }
    finally { $graphics.Dispose(); $bitmap.Dispose() }
}
function Assert-Background([int[]]$Color) {
    if ($Transparency -gt 0) { return }
    $r=[MiniTermUxProbe]::Bounds($taskWindow)
    $image=New-Object Drawing.Bitmap 1,1; $g=[Drawing.Graphics]::FromImage($image)
    try { $g.CopyFromScreen($r.Right-15,$r.Top+[int](($r.Bottom-$r.Top)*0.4),0,0,$image.Size); $pixel=$image.GetPixel(0,0)
        if ([Math]::Abs($pixel.R-$Color[0]) -gt 24 -or [Math]::Abs($pixel.G-$Color[1]) -gt 24 -or [Math]::Abs($pixel.B-$Color[2]) -gt 24) { throw "Theme pixel did not update: $pixel" }
    } finally { $g.Dispose(); $image.Dispose() }
}
$taskProcess=$null
try {
    $taskProcess=[Diagnostics.Process]::Start((New-ProbeStart ''))
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do { Start-Sleep -Milliseconds 100; $taskProcess.Refresh(); $ready=$false
        if ($taskProcess.MainWindowHandle -ne [IntPtr]::Zero) { $bounds=[MiniTermUxProbe]::Bounds($taskProcess.MainWindowHandle); $ready=($bounds.Right-$bounds.Left) -gt 100 }
    } while (-not $ready -and -not $taskProcess.HasExited -and [DateTime]::UtcNow -lt $deadline)
    if ($taskProcess.HasExited -or -not $ready) { throw 'Isolated preview failed to start.' }
    $taskWindow=$taskProcess.MainWindowHandle
    $taskScale=[MiniTermUxProbe]::GetDpiForWindow($taskWindow)/96.0
    Get-Widget 'terminal' | Out-Null
    Start-Sleep -Milliseconds 400
    Invoke-Probe 'diagnostic sample' | Out-Null; Start-Sleep -Milliseconds 250
    $taskSampleDeadline=[DateTime]::UtcNow.AddSeconds(5)
    do { $taskSample=(Invoke-Probe 'sessions') | ConvertFrom-Json
        if ($taskSample.details.fixtureSampleReady) { break }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $taskSampleDeadline)
    if (-not $taskSample.details.fixtureSampleReady) { throw 'Fixture ANSI sample did not reach the real VT grid.' }
    $mini=[MiniTermUxProbe]::Bounds($taskWindow)
    Save-ProbeImage 'mini-input-dark.png'
    $corner=0
    if ([MiniTermUxProbe]::DwmGetWindowAttribute($taskWindow,33,[ref]$corner,4) -ne 0 -or $corner -ne 3) { throw 'Small compositor corners were not configured.' }
    if ($Transparency -gt 0) {
        [uint32]$key=0; [byte]$alpha=0; [uint32]$flags=0
        if (-not [MiniTermUxProbe]::GetLayeredWindowAttributes($taskWindow,[ref]$key,[ref]$alpha,[ref]$flags) -or $alpha -ne [Math]::Floor((100-$Transparency)*255/100)) { throw 'Native transparency was not applied.' }
    }
    $region=[MiniTermUxProbe]::CreateRectRgn(0,0,0,0)
    try { if ([MiniTermUxProbe]::GetWindowRgn($taskWindow,$region) -ne 0) { throw 'Aliased window region remains.' } } finally { [MiniTermUxProbe]::DeleteObject($region) | Out-Null }
    $body=Get-Widget 'terminal'; $x=[int](($body[0]+20)*$taskScale);$y=[int](($body[1]+20)*$taskScale)
    [MiniTermUxProbe]::Click($taskWindow,$x,$y)
    $timer=[Diagnostics.Stopwatch]::StartNew(); [MiniTermUxProbe]::ClickAsync($taskWindow,$x,$y)
    $timeline=New-Object Collections.Generic.List[object]
    $previous=0; $stable=0
    do { Start-Sleep -Milliseconds 8; $expanded=[MiniTermUxProbe]::Bounds($taskWindow); $h=$expanded.Bottom-$expanded.Top
        $timeline.Add([pscustomobject]@{ms=$timer.Elapsed.TotalMilliseconds;height=$h})
        if ($h -eq $previous) { $stable++ } else { $stable=0 }; $previous=$h
    } while ($stable -lt 8 -and $timer.ElapsedMilliseconds -lt 3000)
    if (($expanded.Bottom-$expanded.Top) -le ($mini.Bottom-$mini.Top)) { throw 'Body double click did not expand.' }
    Start-Sleep -Milliseconds 250; $expanded=[MiniTermUxProbe]::Bounds($taskWindow)
    if (($expanded.Right-$expanded.Left) -ne ($mini.Right-$mini.Left)) { throw 'Expansion changed the width.' }
    if ($expanded.Right -ne $mini.Right -or $expanded.Bottom -ne $mini.Bottom) { throw 'Expansion lost the bottom/right anchor.' }
    $taskExpandedSample=(Invoke-Probe 'sessions') | ConvertFrom-Json
    if (-not $taskExpandedSample.details.fixtureSampleReady) { throw 'Expansion lost the fixture output or its ANSI attributes.' }
    if ($timeline.Count -lt 2 -or @($timeline | Select-Object -ExpandProperty height -Unique).Count -lt 2) { throw ('Expansion did not produce intermediate sizes: ' + ($timeline | ConvertTo-Json -Compress)) }
    Save-ProbeImage 'expanded-dark.png'
    # The obsolete false preference must not disable body collapse. Exercise
    # separated top/middle/bottom positions, without interacting with user windows.
    foreach ($taskBodyFraction in @(0.12, 0.5, 0.88)) {
        Start-Sleep -Milliseconds 600
        $taskToggleBody=Get-Widget 'terminal'
        $taskToggleX=[int](($taskToggleBody[0]+($taskToggleBody[2]-$taskToggleBody[0])*0.6)*$taskScale)
        $taskToggleY=[int](($taskToggleBody[1]+($taskToggleBody[3]-$taskToggleBody[1])*$taskBodyFraction)*$taskScale)
        [MiniTermUxProbe]::Click($taskWindow,$taskToggleX,$taskToggleY)
        [MiniTermUxProbe]::Click($taskWindow,$taskToggleX,$taskToggleY)
        Start-Sleep -Milliseconds 300
        $taskCollapsed=[MiniTermUxProbe]::Bounds($taskWindow)
        if (($taskCollapsed.Bottom-$taskCollapsed.Top) -ne ($mini.Bottom-$mini.Top)) { throw "Body double click did not collapse at fraction $taskBodyFraction." }
        Invoke-Probe 'diagnostic expand' | Out-Null
        Start-Sleep -Milliseconds 300
    }
    $hit=[MiniTermUxProbe]::SendMessage($taskWindow,132,[IntPtr]::Zero,[MiniTermUxProbe]::Point($expanded.Right-2,$expanded.Bottom-2)).ToInt32()
    if ($hit -ne 17) { throw "Bottom-right resize hit test failed: $hit" }
    Invoke-Probe 'diagnostic settings' | Out-Null
    Get-Widget 'theme-light' | Out-Null
    Invoke-Probe 'diagnostic theme-light' | Out-Null; Start-Sleep -Milliseconds 180; Assert-Background @(245,247,250); Save-ProbeImage 'settings-light-live.png'
    Invoke-Probe 'diagnostic theme-dark' | Out-Null; Start-Sleep -Milliseconds 180; Assert-Background @(17,21,29); Save-ProbeImage 'settings-dark-live.png'
    Invoke-Probe 'diagnostic settings-cancel' | Out-Null
    $before=[MiniTermUxProbe]::Bounds($taskWindow)
    [MiniTermUxProbe]::SendMessage($taskWindow,561,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
    [MiniTermUxProbe]::SetWindowPos($taskWindow,[IntPtr]::Zero,$before.Left+110,$before.Top+70,[int](680*$taskScale),[int](580*$taskScale),20) | Out-Null
    [MiniTermUxProbe]::SendMessage($taskWindow,562,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 250
    $saved=Get-Content -LiteralPath (Join-Path $taskFixture 'settings.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($null -eq $saved.window.anchor -or $saved.window.miniWidth -ne 680 -or $saved.window.expandedHeight -ne 580) { throw 'Mouse placement/size was not saved.' }
    Invoke-Probe 'diagnostic collapse' | Out-Null; Start-Sleep -Milliseconds 250
    $movedMini=[MiniTermUxProbe]::Bounds($taskWindow)
    Invoke-Probe 'diagnostic expand' | Out-Null; Start-Sleep -Milliseconds 250
    $movedExpanded=[MiniTermUxProbe]::Bounds($taskWindow)
    if ($movedMini.Right -ne $movedExpanded.Right -or $movedMini.Bottom -ne $movedExpanded.Bottom) { throw 'Moved anchor was lost on expansion.' }
    Save-ProbeImage 'moved-expanded.png'
    if ($GlyphSample) {
        Invoke-Probe 'diagnostic sample-ui' | Out-Null
        Start-Sleep -Milliseconds 350
        Save-ProbeImage 'tui-glyphs-dark.png'
        Invoke-Probe 'diagnostic theme-light' | Out-Null
        Start-Sleep -Milliseconds 180
        Save-ProbeImage 'tui-glyphs-light.png'
        Invoke-Probe 'diagnostic theme-dark' | Out-Null
    }
    if ($SummarySample) {
        Invoke-Probe 'diagnostic collapse' | Out-Null
        Start-Sleep -Milliseconds 250
        Invoke-Probe 'diagnostic sample-summary' | Out-Null
        foreach ($taskWidget in @('session-title','summary-status','summary-progress','summary-task','summary-completed','summary-battery')) { Get-Widget $taskWidget | Out-Null }
        foreach ($taskWidget in @('header-add','header-layout','header-hide')) { Get-Widget $taskWidget | Out-Null }
        $taskTitleBounds = Get-Widget 'session-title'
        $taskAddBounds = Get-Widget 'header-add'
        if ($taskTitleBounds[2] -gt $taskAddBounds[0] -or ($taskTitleBounds[2]-$taskTitleBounds[0]) -lt 240) { throw 'Session title space overlaps actions or remains unnecessarily narrow.' }
        $taskSummaryBody=Get-Widget 'terminal'
        $taskSummaryBattery=Get-Widget 'summary-battery'
        foreach ($taskWidget in @('summary-status','summary-task','summary-completed')) {
            $taskSummaryRow=Get-Widget $taskWidget
            if ($taskSummaryRow[1] -lt $taskSummaryBody[1] -or $taskSummaryRow[3] -gt $taskSummaryBattery[1]) { throw 'Summary rows overlap the battery footer.' }
        }
        if ([Math]::Abs($taskSummaryBody[3]-$taskSummaryBattery[3]) -gt 0.5) { throw 'AI Battery is not at the bottom of the summary.' }
        Start-Sleep -Milliseconds 150
        Save-ProbeImage 'summary-dark.png'
        $taskHoverRow=Get-Widget 'summary-task'
        $taskPointerBefore=New-Object MiniTermUxProbe+CursorPoint
        if (-not [MiniTermUxProbe]::GetCursorPos([ref]$taskPointerBefore)) { throw 'Cannot preserve the pointer position.' }
        $taskHoverBounds=[MiniTermUxProbe]::Bounds($taskWindow)
        $taskHoverX=$taskHoverBounds.Left+[int](($taskHoverRow[0]+100)*$taskScale)
        $taskHoverY=$taskHoverBounds.Top+[int](($taskHoverRow[1]+$taskHoverRow[3])*0.5*$taskScale)
        try {
            if (-not [MiniTermUxProbe]::SetCursorPos($taskHoverX,$taskHoverY)) { throw 'Cannot hover the isolated summary.' }
            Start-Sleep -Milliseconds 900
            Get-Widget 'fixture-tooltip-0' | Out-Null
            $taskHoverSnapshot=(Invoke-Probe 'sessions') | ConvertFrom-Json
            $taskHoverTooltipCount=@($taskHoverSnapshot.details.ui.PSObject.Properties | Where-Object { $_.Name -like 'fixture-tooltip-*' }).Count
            if ($taskHoverTooltipCount -ne 1) { throw "Summary hover opened $taskHoverTooltipCount tooltips." }
            Save-ProbeImage 'summary-long-task-hover.png'
        } finally {
            $taskPointerAfter=New-Object MiniTermUxProbe+CursorPoint
            if ([MiniTermUxProbe]::GetCursorPos([ref]$taskPointerAfter) -and $taskPointerAfter.X -eq $taskHoverX -and $taskPointerAfter.Y -eq $taskHoverY) {
                [MiniTermUxProbe]::SetCursorPos($taskPointerBefore.X,$taskPointerBefore.Y) | Out-Null
            }
        }
        Invoke-Probe 'diagnostic theme-light' | Out-Null
        Start-Sleep -Milliseconds 180
        Get-Widget 'summary-completed' | Out-Null
        Save-ProbeImage 'summary-light.png'
        Invoke-Probe 'diagnostic theme-dark' | Out-Null
        $taskSummaryBounds=[MiniTermUxProbe]::Bounds($taskWindow)
        [MiniTermUxProbe]::SendMessage($taskWindow,561,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
        [MiniTermUxProbe]::SetWindowPos($taskWindow,[IntPtr]::Zero,$taskSummaryBounds.Left,$taskSummaryBounds.Top,[int](320*$taskScale),[int](96*$taskScale),20) | Out-Null
        [MiniTermUxProbe]::SendMessage($taskWindow,562,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
        Start-Sleep -Milliseconds 300
        foreach ($taskWidget in @('summary-status','summary-task','summary-battery')) { Get-Widget $taskWidget | Out-Null }
        $taskSmallSummary=(Invoke-Probe 'sessions') | ConvertFrom-Json
        if ($null -ne $taskSmallSummary.details.ui.PSObject.Properties['summary-completed']) { throw 'Small summary retained more rows than fit.' }
        Save-ProbeImage 'summary-small.png'
        Invoke-Probe 'diagnostic sample-summary-command' | Out-Null
        Start-Sleep -Milliseconds 350
        Get-Widget 'summary-status' | Out-Null
        Get-Widget 'summary-output' | Out-Null
        $taskActivitySummary=(Invoke-Probe 'sessions') | ConvertFrom-Json
        if ($null -ne $taskActivitySummary.details.ui.PSObject.Properties['summary-progress'] -or $null -ne $taskActivitySummary.details.ui.PSObject.Properties['summary-task']) { throw 'Command activity invented a task or percentage.' }
        Save-ProbeImage 'summary-command-activity.png'
        Invoke-Probe 'diagnostic sample-ui' | Out-Null
        Start-Sleep -Milliseconds 350
        Get-Widget 'summary-battery' | Out-Null
        $taskNoPlan=(Invoke-Probe 'sessions') | ConvertFrom-Json
        if ($null -ne $taskNoPlan.details.ui.PSObject.Properties['summary-progress'] -or $null -ne $taskNoPlan.details.ui.PSObject.Properties['summary-task']) { throw 'Summary invented progress without an explicit plan.' }
        Save-ProbeImage 'summary-no-plan.png'
        $taskClickBody=Get-Widget 'terminal'
        $taskBeforeSummaryClick=[MiniTermUxProbe]::Bounds($taskWindow)
        [MiniTermUxProbe]::Click($taskWindow,[int](($taskClickBody[0]+20)*$taskScale),[int](($taskClickBody[1]+10)*$taskScale))
        Start-Sleep -Milliseconds 300
        $taskAfterSummaryClick=[MiniTermUxProbe]::Bounds($taskWindow)
        if (($taskAfterSummaryClick.Bottom-$taskAfterSummaryClick.Top) -ne ($taskBeforeSummaryClick.Bottom-$taskBeforeSummaryClick.Top)) { throw 'Summary single click unexpectedly expanded the terminal.' }
        Start-Sleep -Milliseconds 600
        [MiniTermUxProbe]::Click($taskWindow,[int](($taskClickBody[0]+20)*$taskScale),[int](($taskClickBody[1]+10)*$taskScale))
        [MiniTermUxProbe]::Click($taskWindow,[int](($taskClickBody[0]+20)*$taskScale),[int](($taskClickBody[1]+10)*$taskScale))
        Start-Sleep -Milliseconds 300
        $taskAfterSummaryDoubleClick=[MiniTermUxProbe]::Bounds($taskWindow)
        if (($taskAfterSummaryDoubleClick.Bottom-$taskAfterSummaryDoubleClick.Top) -le ($taskBeforeSummaryClick.Bottom-$taskBeforeSummaryClick.Top)) { throw 'Summary double click did not expand the terminal.' }
        Invoke-Probe 'diagnostic collapse' | Out-Null
        Start-Sleep -Milliseconds 250
    }
    if ($ProgressSample) {
        $taskProgressSession = ((Invoke-Probe 'sessions') | ConvertFrom-Json).sessions[0]
        $taskBounds = [MiniTermUxProbe]::Bounds($taskWindow)
        [MiniTermUxProbe]::SendMessage($taskWindow,561,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
        [MiniTermUxProbe]::SetWindowPos($taskWindow,[IntPtr]::Zero,$taskBounds.Left,$taskBounds.Top,[int](560*$taskScale),[int](112*$taskScale),20) | Out-Null
        [MiniTermUxProbe]::SendMessage($taskWindow,562,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
        Start-Sleep -Milliseconds 250
        $taskReport = @{title='한글 태스크 전달 검증';state='running';tasks=@(
            @{text='상단 버튼 정리';state='completed'},
            @{text='한글 태스크 보고';state='running'},
            @{text='갱신 순서 확인';state='pending'}
        )}
        # Exercise the shipped skill adapter with the inherited, isolated helper.
        $taskSkillStart = New-ProbeStart ''
        $taskSkillStart.FileName = (Get-Command python -ErrorAction Stop).Source
        $taskSkillPath = Join-Path $taskRoot '.agents/skills/miniterm-progress/scripts/report.py'
        $taskSkillStart.Arguments = '"' + $taskSkillPath + '" begin'
        $taskSkillStart.EnvironmentVariables['MINITERM_SESSION_ID'] = $taskProgressSession
        $taskSkillStart.EnvironmentVariables['MINITERM_NOTIFY_EXE'] = $taskExecutable
        $taskSkillStart.RedirectStandardInput=$true; $taskSkillStart.RedirectStandardOutput=$true; $taskSkillStart.RedirectStandardError=$true
        $taskSkillProcess = [Diagnostics.Process]::Start($taskSkillStart)
        try {
            $taskSkillOut = $taskSkillProcess.StandardOutput.ReadToEndAsync()
            $taskSkillError = $taskSkillProcess.StandardError.ReadToEndAsync()
            $taskBytes = [Text.UTF8Encoding]::new($false).GetBytes(($taskReport | ConvertTo-Json -Depth 8 -Compress))
            $taskSkillProcess.StandardInput.BaseStream.Write($taskBytes,0,$taskBytes.Length)
            $taskSkillProcess.StandardInput.Close()
            if (-not $taskSkillProcess.WaitForExit(10000)) { $taskSkillProcess.Kill(); throw 'Skill adapter timed out.' }
            if ($taskSkillProcess.ExitCode -ne 0) { throw $taskSkillError.Result }
            $taskReceipt = $taskSkillOut.Result | ConvertFrom-Json
        } finally { $taskSkillProcess.Dispose() }
        $taskRead = Invoke-Progress 'read'
        if ($taskRead.report.tasks[1].text -ne '한글 태스크 보고' -or $taskRead.revision -ne 1) { throw 'UTF-8 report was not delivered through the skill adapter.' }
        foreach ($taskWidget in @('summary-progress','summary-task','summary-completed','summary-battery')) { Get-Widget $taskWidget | Out-Null }
        Start-Sleep -Milliseconds 150; Save-ProbeImage 'progress-skill-dark.png'
        Invoke-Probe 'diagnostic theme-light' | Out-Null
        Start-Sleep -Milliseconds 180; Save-ProbeImage 'progress-skill-light.png'
        Invoke-Probe 'diagnostic theme-dark' | Out-Null
        $taskReport.tasks[1].state='completed'; $taskReport.tasks[2].state='running'
        $taskUpdate = Invoke-Progress 'update' $taskReport $taskReceipt.workId 2
        if ($taskUpdate.revision -ne 2) { throw 'Task transition did not advance the revision.' }
        Invoke-Progress 'update' $taskReport $taskReceipt.workId 2 -Reject
        Start-Sleep -Milliseconds 160; Save-ProbeImage 'progress-updated.png'
        $taskReport.state='completed'
        Invoke-Progress 'update' $taskReport $taskReceipt.workId 3 -Reject
        if ((Invoke-Progress 'read').revision -ne 2) { throw 'Invalid completion mutated the plan.' }
        $taskReport.state='paused'
        Invoke-Progress 'update' $taskReport $taskReceipt.workId 3 | Out-Null
        Start-Sleep -Milliseconds 160; Save-ProbeImage 'progress-paused.png'
        $taskReport.state='running'
        $taskNext = Invoke-Progress 'begin' $taskReport
        Invoke-Progress 'update' $taskReport $taskReceipt.workId 4 -Reject
        if ((Invoke-Progress 'read').workId -ne $taskNext.workId) { throw 'An older work report replaced the current task.' }
        $taskReport.tasks[2].state='completed'; $taskReport.state='completed'
        Invoke-Progress 'update' $taskReport $taskNext.workId 2 | Out-Null
        Start-Sleep -Milliseconds 160; Save-ProbeImage 'progress-completed.png'
        Invoke-Progress 'clear' $null $taskNext.workId 3 | Out-Null
        if ($null -ne (Invoke-Progress 'read')) { throw 'Cleared progress remained in the broker.' }
        Start-Sleep -Milliseconds 160
        $taskCleared = (Invoke-Probe 'sessions') | ConvertFrom-Json
        if ($null -ne $taskCleared.details.ui.PSObject.Properties['summary-progress']) { throw 'Cleared progress remained visible.' }
        Get-Widget 'summary-battery' | Out-Null
        Save-ProbeImage 'progress-cleared.png'
        $taskProgressChecks = @{utf8SkillAdapter=$true;taskTransitions=$true;staleReportRejected=$true;falseCompletionRejected=$true;pausedPlanRetained=$true;newWorkReplacesOld=$true;clearRemovesProgress=$true;batteryPreserved=$true;scope='Own IPC/CLI/skill adapter and fixed GUI data; excludes model skill selection, actual user input and global skill installation.'}
    }
    for ($taskSessionIndex=1; $taskSessionIndex -lt $SessionCount; $taskSessionIndex++) {
        Invoke-Probe 'diagnostic new-session' | Out-Null
        $taskSessionDeadline=[DateTime]::UtcNow.AddSeconds(20)
        do {
            Start-Sleep -Milliseconds 100
            $taskSessionSnapshot=(Invoke-Probe 'sessions') | ConvertFrom-Json
            $taskShells=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($taskProcess.Id) AND Name='bash.exe'" | Select-Object -ExpandProperty ProcessId)
        } while (($taskSessionSnapshot.sessions.Count -ne ($taskSessionIndex+1) -or $taskShells.Count -ne ($taskSessionIndex+1)) -and [DateTime]::UtcNow -lt $taskSessionDeadline)
        if ($taskSessionSnapshot.sessions.Count -ne ($taskSessionIndex+1) -or $taskShells.Count -ne ($taskSessionIndex+1)) { throw 'Isolated Bash session did not start.' }
    }
    Start-Sleep -Milliseconds 600
    $taskSessionSnapshot=(Invoke-Probe 'sessions') | ConvertFrom-Json
    if ($taskSessionSnapshot.sessions.Count -ne $SessionCount) { throw 'Unexpected isolated session count.' }
    $taskShells=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($taskProcess.Id) AND Name='bash.exe'" | Select-Object -ExpandProperty ProcessId)
    if ($taskExplicitBash) {
        foreach ($taskShell in $taskShells) {
            $taskShellPath = (Get-CimInstance Win32_Process -Filter "ProcessId=$taskShell").ExecutablePath
            if ($taskShellPath -ine $taskExplicitBash) { throw 'The GUI fixture did not use the explicit Bash runtime.' }
        }
        if ((Get-FileHash -LiteralPath $taskRuntimeDll -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskRuntimeHash) { throw 'Runtime changed during the GUI test.' }
    }
    $taskFinalForeground = [MiniTermUxProbe]::GetForegroundWindow()
    if ($taskFinalForeground -eq $taskWindow) { throw 'Preview stole foreground focus.' }
    $taskForegroundPreserved = $taskFinalForeground -eq $taskFocus
    foreach ($id in $taskUserProcesses) { if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) { throw 'User MiniTerm stopped during isolated test.' } }
    if ($IdleSeconds -gt 0) {
        & (Join-Path $PSScriptRoot 'measure-idle.ps1') -ProcessId $taskProcess.Id -Seconds $IdleSeconds -OutputPath (Join-Path $taskFixture 'idle.json') | Out-Null
    }
    Invoke-Probe 'diagnostic quit' | Out-Null
    if (-not $taskProcess.WaitForExit(10000)) { throw 'Preview did not close.' }
    $taskMetrics=Get-Content -LiteralPath (Join-Path $taskFixture 'metrics.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($taskMetrics.sessions -ne $SessionCount -or $taskMetrics.pendingStarts -ne 0) { throw 'Session startup was not complete during the resource test.' }
    foreach ($taskShell in $taskShells) { if (Get-Process -Id $taskShell -ErrorAction SilentlyContinue) { throw 'A test Bash process survived app shutdown.' } }
    $result=@{fixture=$taskFixture;executableSha256=(Get-FileHash $taskExecutable -Algorithm SHA256).Hash;bodyDoubleClick=$true;verticalExpansion=$true;fixtureAnsiPreserved=$true;expansionGeometryMs=$timeline[-1].ms;expansionSizes=$timeline;resizeHitTest=$true;dragAndResizePersistence=$true;liveDarkLight=($Transparency -eq 0);compositorSmallCorners=$true;noBinaryRegion=$true;nativeTransparency=$Transparency;glyphCaptures=[bool]$GlyphSample;userSessionPreserved=$true;foregroundPreserved=$true;note='Client mouse messages, shared theme handler and native move/resize completion; geometry includes stable tail and excludes pixel latency. Optional glyph captures require image review; they are not a Codex popup test.'}
    $result.foregroundPreserved=$taskForegroundPreserved
    $result.foregroundChangedExternally=-not $taskForegroundPreserved
    $result.focusObservationScope='Startup metric and final foreground observation only. An unrelated foreground change does not prove the preview stole focus; uninterrupted focus is unverified in that case.'
    $result.sessionCount=$SessionCount
    $result.summaryCaptures=[bool]$SummarySample
    $result.bodyCollapsePositions=@('top','middle','bottom')
    $result.legacyFalsePreferenceIgnored=$true
    if ($SummarySample) { $result.summaryChecks=@{headerTitle=$true;threeHeaderActions=$true;titleSpace=$true;planRowsFit=$true;batteryAtBottom=$true;smallWindowRowBudget=$true;noInventedProgress=$true;commandActivityHasNoInventedPlan=$true;longTaskHoverCaptured=$true;longTaskTooltipCount=$taskHoverTooltipCount;singleClickPreservesSize=$true;doubleClickExpands=$true;scope='Fixed isolated VT samples, widget bounds and dark/light captures. Native pointer moved briefly onto own long task and restored unless user moved it; exactly one egui tooltip observed. Screenshot text requires review. No real model or human mouse/scroll test.'} }
    if ($ProgressSample) { $result.progressChecks=$taskProgressChecks }
    $result.testBashProcessIds=$taskShells
    $result.explicitBashPath=$taskExplicitBash
    $result.runtimeSha256=$taskRuntimeHash
    $result.testBashShutdownConfirmed=$true
    [IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/ux.json'),($result | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($result | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    $result | ConvertTo-Json
} finally {
    if ($taskProcess -and -not $taskProcess.HasExited) { try { Invoke-Probe 'diagnostic quit' | Out-Null } catch {}; if (-not $taskProcess.WaitForExit(3000)) { $taskProcess.Kill(); $taskProcess.WaitForExit() } }
    [MiniTermUxProbe]::SetThreadDpiAwarenessContext($taskDpiContext) | Out-Null
}
