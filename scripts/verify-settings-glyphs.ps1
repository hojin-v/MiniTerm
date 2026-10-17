param([string]$Executable = 'artifacts/miniterm.exe', [ValidateSet(0,10,30,60)][int]$IdleSeconds = 0)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskExe = if([IO.Path]::IsPathRooted($Executable)){(Get-Item -LiteralPath $Executable).FullName}else{(Get-Item -LiteralPath (Join-Path $taskRoot $Executable)).FullName}
$taskId = [Guid]::NewGuid().ToString()
$taskFixture = Join-Path $taskRoot ('artifacts/settings-glyphs-' + $taskId)
New-Item -ItemType Directory -Path $taskFixture | Out-Null
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class GlyphProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    public delegate bool EnumProc(IntPtr window,IntPtr param);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback,IntPtr param);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr window,StringBuilder title,int count);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window,out Rect rect);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window,IntPtr after,int x,int y,int w,int h,uint flags);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    public static IntPtr Find(int pid,string title) {
        IntPtr found=IntPtr.Zero;
        EnumWindows((w,p)=>{uint id; GetWindowThreadProcessId(w,out id); var s=new StringBuilder(256); GetWindowText(w,s,256);
            if(id==pid && s.ToString()==title){found=w;return false;}return true;},IntPtr.Zero);
        return found;
    }
    public static Rect Bounds(IntPtr w){Rect r;if(!GetWindowRect(w,out r))throw new Exception("Own window missing");return r;}
}
'@
$taskDpi = [GlyphProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskSettings = @{schemaVersion=1;shortcut='';notifications=$false;profile=@{kind='git_bash';directory=$taskRoot};window=@{autoCollapse=$false;transparency=0}}
[IO.File]::WriteAllText((Join-Path $taskFixture 'settings.json'),($taskSettings | ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
function New-GlyphStart([string]$Arguments) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName=$taskExe; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.EnvironmentVariables['MINITERM_INSTANCE_ID']=$taskId
    $start.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
    $start.EnvironmentVariables['MINITERM_DATA_DIR']=$taskFixture
    $start.EnvironmentVariables['CODEX_HOME']=(Join-Path $taskFixture 'codex')
    $start.EnvironmentVariables['CLAUDE_CONFIG_DIR']=(Join-Path $taskFixture 'claude')
    return $start
}
function Invoke-Glyph([string]$Arguments) {
    $start=New-GlyphStart $Arguments; $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $helper=[Diagnostics.Process]::Start($start)
    try {
        $out=$helper.StandardOutput.ReadToEndAsync(); $err=$helper.StandardError.ReadToEndAsync()
        if(-not $helper.WaitForExit(10000)){ $helper.Kill(); throw 'Own helper timed out' }
        if($helper.ExitCode -ne 0){throw $err.Result}
        return $out.Result
    } finally {$helper.Dispose()}
}
function Capture-Glyph([string]$Name) {
    $snapshot=(Invoke-Glyph 'sessions') | ConvertFrom-Json
    $rect=$snapshot.details.ui.terminal
    $bounds=[GlyphProbe]::Bounds($taskWindow); $scale=[GlyphProbe]::GetDpiForWindow($taskWindow)/96.0
    $full=New-Object Drawing.Bitmap ($bounds.Right-$bounds.Left),($bounds.Bottom-$bounds.Top)
    $fullGraphics=[Drawing.Graphics]::FromImage($full)
    try {$fullGraphics.CopyFromScreen($bounds.Left,$bounds.Top,0,0,$full.Size);$full.Save((Join-Path $taskFixture ('window-'+$Name)))} finally {$fullGraphics.Dispose();$full.Dispose()}
    # Exclude the title, hover affordances and blinking cursor at the bottom.
    $bitmap=New-Object Drawing.Bitmap ([int]([Math]::Min(480,$rect[2]-$rect[0]-8)*$scale)),([int](108*$scale))
    $g=[Drawing.Graphics]::FromImage($bitmap)
    try {$g.CopyFromScreen($bounds.Left+[int](($rect[0]+4)*$scale),$bounds.Top+[int]($rect[1]*$scale),0,0,$bitmap.Size); $bitmap.Save((Join-Path $taskFixture $Name))}
    finally {$g.Dispose();$bitmap.Dispose()}
    return (Get-FileHash -LiteralPath (Join-Path $taskFixture $Name)).Hash
}
$taskProcess=$null
$taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExe).Hash;passed=$false;scope='Own app and settings; native pixel comparison of fixed Latin/Hangul/style samples while the independent settings viewport is open. No desktop keyboard/mouse input or user config changes.'}
try {
    $taskProcess=[Diagnostics.Process]::Start((New-GlyphStart ''))
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100
        $taskWindow=[GlyphProbe]::Find($taskProcess.Id,'MiniTerm')
        try {$ready=@(((Invoke-Glyph 'sessions') | ConvertFrom-Json).sessions).Count -eq 1} catch {$ready=$false}
    } while((-not $ready -or $taskWindow -eq [IntPtr]::Zero) -and [DateTime]::UtcNow -lt $deadline)
    if(-not $ready -or $taskWindow -eq [IntPtr]::Zero){throw 'Own app did not start'}
    Invoke-Glyph 'diagnostic sample' | Out-Null
    $deadline=[DateTime]::UtcNow.AddSeconds(10)
    do {Start-Sleep -Milliseconds 80;$ready=((Invoke-Glyph 'sessions') | ConvertFrom-Json).details.fixtureSampleReady} while(-not $ready -and [DateTime]::UtcNow -lt $deadline)
    if(-not $ready){throw 'Own Bash output was not ready'}
    Invoke-Glyph 'diagnostic expand' | Out-Null
    Start-Sleep -Milliseconds 500
    $area=[Windows.Forms.Screen]::FromHandle($taskWindow).WorkingArea
    $bounds=[GlyphProbe]::Bounds($taskWindow)
    $settingsWidth=560*[GlyphProbe]::GetDpiForWindow($taskWindow)/96.0
    if($area.Width -lt ($bounds.Right-$bounds.Left+$settingsWidth+36)){throw 'This pixel test needs room for both windows side by side'}
    [GlyphProbe]::SetWindowPos($taskWindow,[IntPtr]::Zero,($area.Right-($bounds.Right-$bounds.Left)-12),($area.Top+12),0,0,0x51) | Out-Null
    $checks=@()
    foreach($theme in @('dark','light')) {
        Invoke-Glyph ('diagnostic theme-'+$theme) | Out-Null
        Invoke-Glyph 'diagnostic sample-ui' | Out-Null
        Start-Sleep -Milliseconds 1000
        $expected=Capture-Glyph ($theme+'-before.png')
        Invoke-Glyph 'diagnostic settings' | Out-Null
        $deadline=[DateTime]::UtcNow.AddSeconds(5)
        do {Start-Sleep -Milliseconds 50;$settings=[GlyphProbe]::Find($taskProcess.Id,'MiniTerm 설정')} while($settings -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
        if($settings -eq [IntPtr]::Zero){throw 'Own settings viewport did not open'}
        [GlyphProbe]::SetWindowPos($settings,[IntPtr]::Zero,($area.Left+12),($area.Top+12),0,0,0x51) | Out-Null
        foreach($pass in 1..3) {
            Invoke-Glyph ('diagnostic theme-'+$theme) | Out-Null
            Start-Sleep -Milliseconds 180
            $actual=Capture-Glyph ($theme+'-settings-'+$pass+'.png')
            $checks+=@{theme=$theme;pass=$pass;matches=($actual -eq $expected)}
        }
        if($theme -eq 'light' -and $IdleSeconds -gt 0) {
            $taskResult.settingsOpenIdle=(& (Join-Path $PSScriptRoot 'measure-idle.ps1') -ProcessId $taskProcess.Id -Seconds $IdleSeconds -OutputPath (Join-Path $taskFixture 'settings-open-idle.json')) | ConvertFrom-Json
        }
        Invoke-Glyph 'diagnostic settings-close' | Out-Null
        Start-Sleep -Milliseconds 250
    }
    $taskResult.checks=$checks
    if(@($checks | Where-Object {-not $_.matches}).Count){throw 'Terminal glyph pixels changed while settings was open'}
    $taskResult.passed=$true
} catch {$taskResult.error=$_.Exception.Message;throw}
finally {
    if($taskProcess -and -not $taskProcess.HasExited){try {Invoke-Glyph 'diagnostic quit' | Out-Null} catch {};if(-not $taskProcess.WaitForExit(3000)){$taskProcess.Kill()}}
    [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    [GlyphProbe]::SetThreadDpiAwarenessContext($taskDpi) | Out-Null
}
$taskResult | ConvertTo-Json -Depth 6
