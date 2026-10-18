param([string]$Executable='.tools/target/debug/miniterm.exe', [string]$WslDistribution, [string]$BashPath='C:\Program Files\Git\bin\bash.exe')
$ErrorActionPreference='Stop'
$taskRoot=Split-Path -Parent $PSScriptRoot
$taskExe=[IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
$taskFixture=Join-Path $taskRoot ('artifacts/footer-lifecycle-'+[Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $taskFixture | Out-Null
Copy-Item -LiteralPath $taskExe -Destination (Join-Path $taskFixture 'miniterm.exe')
$taskExe=Join-Path $taskFixture 'miniterm.exe'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FooterProbe {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 public delegate bool EnumProc(IntPtr hwnd,IntPtr value);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback,IntPtr value);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd,StringBuilder value,int count);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd,IntPtr after,int x,int y,int width,int height,uint flags);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hwnd,uint msg,IntPtr wp,IntPtr lp);
 [DllImport("user32.dll")] public static extern IntPtr LoadCursor(IntPtr instance,IntPtr name);
 [DllImport("user32.dll")] public static extern IntPtr GetCursor();
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 public static IntPtr Find(int process) {
  IntPtr found=IntPtr.Zero; EnumWindows((w,p)=> {uint id;GetWindowThreadProcessId(w,out id);var title=new StringBuilder(256);GetWindowText(w,title,256);if(id==process&&title.ToString()=="MiniTerm"){found=w;return false;}return true;},IntPtr.Zero);return found;
 }
}
'@
[FooterProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))|Out-Null
$taskSaved=@{}
foreach($taskName in @('MINITERM_INSTANCE_ID','MINITERM_DATA_DIR','MINITERM_DIAGNOSTICS','HOME')){$taskSaved[$taskName]=[Environment]::GetEnvironmentVariable($taskName,'Process')}
$taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExe).Hash.ToLowerInvariant();profile=$(if($WslDistribution){$WslDistribution}else{'Git Bash'});passed=$false}
$taskProcess=$null
function Invoke-Footer([string[]]$Arguments) { $taskOutput=& $taskExe @Arguments;if($LASTEXITCODE -ne 0){throw "Fixture helper failed: $($Arguments -join ' ')"};$taskOutput }
function Read-State { (Invoke-Footer @('sessions')) | ConvertFrom-Json }
function Await([scriptblock]$Predicate,[string]$Label) { $taskUntil=[DateTime]::UtcNow.AddSeconds(25);do{if(& $Predicate){return};Start-Sleep -Milliseconds 60}while([DateTime]::UtcNow -lt $taskUntil);throw "Timed out: $Label" }
function Save-Shot([string]$Name) { $taskBounds=New-Object FooterProbe+Rect;[FooterProbe]::GetWindowRect($taskWindow,[ref]$taskBounds)|Out-Null;$taskBitmap=New-Object Drawing.Bitmap ($taskBounds.Right-$taskBounds.Left),($taskBounds.Bottom-$taskBounds.Top);$taskGraphics=[Drawing.Graphics]::FromImage($taskBitmap);try{$taskGraphics.CopyFromScreen($taskBounds.Left,$taskBounds.Top,0,0,$taskBitmap.Size);$taskBitmap.Save((Join-Path $taskFixture $Name))}finally{$taskGraphics.Dispose();$taskBitmap.Dispose()} }
try {
 $env:MINITERM_INSTANCE_ID=[Guid]::NewGuid().ToString();$env:MINITERM_DATA_DIR=$taskFixture;$env:MINITERM_DIAGNOSTICS='1';$env:HOME=Join-Path $taskFixture 'home'
 New-Item -ItemType Directory -Path $env:HOME|Out-Null
 $taskProfile=if($WslDistribution){@{kind='wsl';distribution=$WslDistribution;directory='~'}}else{@{kind='git_bash';directory=$taskFixture;bash_path=$BashPath}}
 $taskSettings=@{schemaVersion=1;shortcut='';notifications=$false;profile=$taskProfile;window=@{miniWidth=560;miniHeight=180;autoCollapse=$false;reducedMotion=$true}}|ConvertTo-Json -Depth 6
 [IO.File]::WriteAllText((Join-Path $taskFixture 'settings.json'),$taskSettings,[Text.UTF8Encoding]::new($false))
 $taskProcess=Start-Process -FilePath $taskExe -PassThru -WindowStyle Hidden
 Await {try{$s=Read-State;$s.sessions.Count -eq 1 -and $null -ne $s.details.ui.'tab-0'}catch{$false}} 'automatic first rendered tab'
 $taskWindow=[FooterProbe]::Find($taskProcess.Id)
 if($taskWindow -eq [IntPtr]::Zero){throw 'Own GUI window not found'}
 $taskResult.firstTabAutomatic=$true
 # No desktop keyboard or pointer input: only this fixture's IPC and window messages.
 foreach($taskPlacement in @('above','bottom')){
  Invoke-Footer @('diagnostic',"fixture-battery-$taskPlacement")|Out-Null
  Await { $s=Read-State; $null -ne $s.details.ui.'terminal-battery' } "battery $taskPlacement"
  $s=Read-State;$taskOriginal=$s.sessions[0]
  $s|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $taskFixture "$taskPlacement-mini.json") -Encoding UTF8
  Save-Shot "$taskPlacement-mini.png"
  Invoke-Footer @('diagnostic','fixture-expand')|Out-Null
  Await { $s=Read-State; $null -ne $s.details.ui.'terminal-battery' -and $s.details.ui.terminal[3]-$s.details.ui.terminal[1] -gt 200 } 'expanded footer'
  $taskBounds=New-Object FooterProbe+Rect;[FooterProbe]::GetWindowRect($taskWindow,[ref]$taskBounds)|Out-Null
  [FooterProbe]::SetWindowPos($taskWindow,[IntPtr]::Zero,0,0,650,500,0x16)|Out-Null
  Start-Sleep -Milliseconds 600
  $s=Read-State;if($null -eq $s.details.ui.'terminal-battery'){throw 'Resize lost footer without input'}
  if($s.sessions[0] -ne $taskOriginal){throw 'Resize changed the session'}
  $taskFooter=$s.details.ui.'terminal-battery';$taskBody=$s.details.ui.terminal
  if([Math]::Abs($taskFooter[3]-$taskBody[3]) -gt 1.5){throw 'Footer is not at the bottom'}
  Save-Shot "$taskPlacement-resized.png"
  Invoke-Footer @('diagnostic','fixture-collapse')|Out-Null
  Await {$null -ne (Read-State).details.ui.'terminal-battery'} 'compact footer after resize'
  Invoke-Footer @('diagnostic','fixture-view')|Out-Null
  Await {$null -ne (Read-State).details.ui.'summary-battery'} 'summary footer'
  Save-Shot "$taskPlacement-summary.png"
  Invoke-Footer @('diagnostic','fixture-view')|Out-Null
  Invoke-Footer @('diagnostic','fixture-finish')|Out-Null
  Await {$null -eq (Read-State).details.ui.'terminal-battery'} 'footer cleared at real Bash prompt'
 }
 $taskResult.bothPlacements=$true;$taskResult.resizeWithoutInput=$true;$taskResult.compactAndSummary=$true;$taskResult.promptClearsFooter=$true
 $taskBounds=New-Object FooterProbe+Rect;[FooterProbe]::GetWindowRect($taskWindow,[ref]$taskBounds)|Out-Null
 $taskWidth=$taskBounds.Right-$taskBounds.Left;$taskHeight=$taskBounds.Bottom-$taskBounds.Top
 foreach($taskHit in @(@(1,1,13,32642),@(($taskWidth-2),1,14,32643),@(1,($taskHeight-2),16,32643),@(($taskWidth-2),($taskHeight-2),17,32642),@(1,($taskHeight/2),10,32644),@(($taskWidth-2),($taskHeight/2),11,32644),@(($taskWidth/2),1,12,32645),@(($taskWidth/2),($taskHeight-2),15,32645))){
  $taskX=$taskBounds.Left+[int]$taskHit[0];$taskY=$taskBounds.Top+[int]$taskHit[1]
  $taskPacked=($taskX -band 0xffff) -bor (($taskY -band 0xffff) -shl 16)
  for($taskRepeat=0;$taskRepeat -lt 5;$taskRepeat++){
   $taskActual=[FooterProbe]::SendMessage($taskWindow,0x84,[IntPtr]::Zero,[IntPtr]$taskPacked).ToInt32()
   if($taskActual -ne $taskHit[2]){throw "Wrong edge hit: $taskActual expected $($taskHit[2])"}
   if([FooterProbe]::SendMessage($taskWindow,0x20,$taskWindow,[IntPtr]($taskActual -bor (0x200 -shl 16))).ToInt32() -ne 1){throw 'Resize cursor message was not handled'}
  }
 }
 $taskResult.allEdgesAndCorners=$true
 Invoke-Footer @('diagnostic','new-session')|Out-Null;Await {(Read-State).sessions.Count -eq 2} 'second tab'
 Invoke-Footer @('diagnostic','fixture-exit')|Out-Null;Await {(Read-State).sessions.Count -eq 1} 'first exit removes second tab'
 Invoke-Footer @('diagnostic','fixture-exit')|Out-Null;Await {(Read-State).sessions.Count -eq 0 -and ![FooterProbe]::IsWindowVisible($taskWindow)} 'one exit hides last tab and window'
 $taskResult.lastExitNeedsNoExtraInput=$true
 Invoke-Footer @()|Out-Null;Await {(Read-State).sessions.Count -eq 1 -and [FooterProbe]::IsWindowVisible($taskWindow)} 'relaunch opens a first tab'
 $taskResult.relaunchOpensTab=$true
 $taskResult.passed=$true
 $taskResult.scope='Real GUI, production PTY and shell; fixed VT battery rows, no account/model or desktop input. Native edge messages; no physical pointer/keyboard claim.'
}catch{$taskResult.error=$_.Exception.Message;throw}finally{
 $taskResult|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $taskFixture 'verification.json') -Encoding UTF8
 if($taskProcess -and !$taskProcess.HasExited){try{Invoke-Footer @('diagnostic','quit')|Out-Null}catch{};if(!$taskProcess.WaitForExit(4000)){$taskProcess.Kill()}}
 foreach($taskName in $taskSaved.Keys){[Environment]::SetEnvironmentVariable($taskName,$taskSaved[$taskName],'Process')}
}
$taskResult|ConvertTo-Json -Depth 8
