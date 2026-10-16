param([Parameter(Mandatory=$true)][string]$Fixture)
$ErrorActionPreference='Stop'
$taskRoot=Split-Path -Parent $PSScriptRoot
$taskExecutable=Join-Path $taskRoot 'artifacts/miniterm.exe'
$taskFixture=[IO.Path]::GetFullPath($Fixture)
$taskMarker=Get-Content -LiteralPath (Join-Path $taskFixture 'fixture-owned.json') -Encoding UTF8 -Raw | ConvertFrom-Json
if ((Split-Path -Parent $taskFixture) -ne (Join-Path $taskRoot 'artifacts') -or (Split-Path -Leaf $taskFixture) -notmatch '^goal-hud-[0-9a-f-]{36}$' -or $taskMarker.kind -ne 'miniterm-local-goal-hud' -or $taskMarker.root -ne $taskFixture -or $taskMarker.home -ne (Join-Path $taskFixture 'codex-home') -or (Test-Path -LiteralPath (Join-Path $taskMarker.home 'auth.json'))) { throw 'Expected an owned account-free GUI fixture.' }
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MiniTermGoalProbe {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
 [StructLayout(LayoutKind.Sequential)] public struct Mouse { public int X,Y; public uint Data,Flags,Time; public UIntPtr Extra; }
 [StructLayout(LayoutKind.Sequential)] public struct Key { public ushort Vk,Scan; public uint Flags,Time; public UIntPtr Extra; }
 [StructLayout(LayoutKind.Explicit)] public struct Union { [FieldOffset(0)] public Mouse Mouse; [FieldOffset(0)] public Key Key; }
 [StructLayout(LayoutKind.Sequential)] public struct Input { public uint Type; public Union Data; }
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point p);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern uint SendInput(uint n,Input[] i,int size);
 static Input KeyEvent(ushort vk,ushort scan,uint flags) { return new Input{Type=1,Data=new Union{Key=new Key{Vk=vk,Scan=scan,Flags=flags}}}; }
 static void Send(IntPtr h,Input[] input) {
   if(GetForegroundWindow()!=h)throw new Exception("Fixture lost focus; input cancelled");
   if(SendInput((uint)input.Length,input,Marshal.SizeOf(typeof(Input)))!=input.Length)throw new Exception("SendInput failed");
 }
 public static void Text(IntPtr h,string text) {
   var input=new Input[text.Length*2];
   for(int n=0;n<text.Length;n++){input[n*2]=KeyEvent(0,text[n],4);input[n*2+1]=KeyEvent(0,text[n],6);}
   Send(h,input);
 }
 public static void Press(IntPtr h,ushort vk) { Send(h,new[]{KeyEvent(vk,0,0),KeyEvent(vk,0,2)}); }
 public static void FocusByClick(IntPtr h) {
   var r=Bounds(h);SetCursorPos(r.Left+30,r.Top+55);
   mouse_event(2,0,0,0,UIntPtr.Zero);System.Threading.Thread.Sleep(30);mouse_event(4,0,0,0,UIntPtr.Zero);
 }
 public static Rect Bounds(IntPtr h) { Rect r;if(!GetWindowRect(h,out r))throw new Exception("Window unavailable");return r; }
}
'@
$taskOriginalFocus=[MiniTermGoalProbe]::GetForegroundWindow()
$taskOriginalPointer=New-Object MiniTermGoalProbe+Point
[MiniTermGoalProbe]::GetCursorPos([ref]$taskOriginalPointer) | Out-Null
$taskDpiContext=[MiniTermGoalProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskUserApps=@(Get-Process miniterm -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{id=$_.Id;started=$_.StartTime} })
$taskId=[Guid]::NewGuid().ToString()
$taskSettings=@{schemaVersion=1;theme='dark';shortcut='';notifications=$false;profile=@{kind='git_bash';directory=$taskMarker.work};window=@{anchor=@{right=7200;bottom=57};miniWidth=680;miniHeight=112;expandedHeight=900;autoCollapse=$false};prompt=@{showUser=$false;showHost=$false;showPath=$false;symbol='QA>'}}
[IO.File]::WriteAllText((Join-Path $taskFixture 'settings.json'),($taskSettings | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
function New-GoalStart([string]$Arguments) {
 $taskStart=New-Object Diagnostics.ProcessStartInfo
 $taskStart.FileName=$taskExecutable;$taskStart.Arguments=$Arguments;$taskStart.UseShellExecute=$false;$taskStart.CreateNoWindow=$true
 $taskStart.EnvironmentVariables['MINITERM_INSTANCE_ID']=$taskId
 $taskStart.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
 $taskStart.EnvironmentVariables['MINITERM_DATA_DIR']=$taskFixture
 $taskStart.EnvironmentVariables['CODEX_HOME']=$taskMarker.home
 $taskStart.EnvironmentVariables['AI_BATTERY_BIN']=(Join-Path $taskFixture 'battery-fixture.cjs')
 $taskStart.EnvironmentVariables['AI_BATTERY_DATA_DIR']=(Join-Path $taskFixture 'battery-data')
 $taskStart.EnvironmentVariables['AI_BATTERY_RUNTIME_DOWNLOAD']='0'
 $taskStart.EnvironmentVariables['AI_BATTERY_DEBUG_LOG']=(Join-Path $taskFixture 'battery-debug.jsonl')
 # Use the already installed VT-capable Node in this child only. An isolated
 # battery data folder deliberately has no runtime and cannot download one.
 $taskVtNode=Join-Path $env:LOCALAPPDATA 'ai-battery/runtimes/node-v24.21.0-win-x64/node.exe'
 if(-not (Test-Path -LiteralPath $taskVtNode)){throw 'Installed VT-capable AI Battery Node is required.'}
 $taskStart.EnvironmentVariables['PATH']=(Split-Path -Parent $taskVtNode)+';'+$env:PATH
 foreach ($taskName in @('OPENAI_API_KEY','CODEX_API_KEY','OPENAI_BASE_URL','MINITERM_CODEX_INLINE','MINITERM_SESSION_ID','MINITERM_NOTIFY_EXE','MINITERM_METRICS_PATH','AI_BATTERY_WRAPPED_CODEX','AI_BATTERY_DISABLE_WINDOWS_CODEX_RUNNER','AI_BATTERY_WIN_LAYOUT','AI_BATTERY_ACTIVE_LAYOUT','AI_BATTERY_ORIGINAL_CODEX')) { $taskStart.EnvironmentVariables.Remove($taskName) }
 return $taskStart
}
function Invoke-Goal([string]$Arguments) {
 $taskStart=New-GoalStart $Arguments;$taskStart.RedirectStandardOutput=$true;$taskStart.RedirectStandardError=$true
 $taskHelper=[Diagnostics.Process]::Start($taskStart)
 try {
   $taskOut=$taskHelper.StandardOutput.ReadToEndAsync();$taskErr=$taskHelper.StandardError.ReadToEndAsync()
   if(-not $taskHelper.WaitForExit(10000)){ $taskHelper.Kill();throw 'Fixture helper timed out.' }
   if($taskHelper.ExitCode -ne 0){throw $taskErr.Result}
   return $taskOut.Result
 } finally {$taskHelper.Dispose()}
}
function Get-GoalWidget([string]$Name) {
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(12)
 do {
   $taskReply=(Invoke-Goal 'sessions') | ConvertFrom-Json
   $taskProperty=$taskReply.details.ui.PSObject.Properties[$Name]
   if($null -ne $taskProperty){return $taskProperty.Value}
   Start-Sleep -Milliseconds 100
 } while([DateTime]::UtcNow -lt $taskDeadline)
 throw "Widget not visible: $Name"
}
function Save-GoalImage([string]$Name) {
 $taskBounds=[MiniTermGoalProbe]::Bounds($taskWindow)
 $taskImage=New-Object Drawing.Bitmap ($taskBounds.Right-$taskBounds.Left),($taskBounds.Bottom-$taskBounds.Top)
 $taskGraphics=[Drawing.Graphics]::FromImage($taskImage)
 try {$taskGraphics.CopyFromScreen($taskBounds.Left,$taskBounds.Top,0,0,$taskImage.Size);$taskImage.Save((Join-Path $taskFixture $Name),[Drawing.Imaging.ImageFormat]::Png)}finally{$taskGraphics.Dispose();$taskImage.Dispose()}
}
$taskApp=$null
try {
 $taskApp=[Diagnostics.Process]::Start((New-GoalStart ''))
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(20)
 do {
   Start-Sleep -Milliseconds 100;$taskApp.Refresh();$taskReady=$false
   if($taskApp.MainWindowHandle -ne [IntPtr]::Zero){$taskInitialBounds=[MiniTermGoalProbe]::Bounds($taskApp.MainWindowHandle);$taskReady=[MiniTermGoalProbe]::IsWindowVisible($taskApp.MainWindowHandle) -and ($taskInitialBounds.Right-$taskInitialBounds.Left) -gt 100}
 }while(-not $taskReady -and -not $taskApp.HasExited -and [DateTime]::UtcNow -lt $taskDeadline)
 if($taskApp.HasExited -or -not $taskReady){throw 'Fixture GUI did not start.'}
 $taskWindow=$taskApp.MainWindowHandle
 Get-GoalWidget 'terminal' | Out-Null
 $taskShells=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($taskApp.Id) AND Name='bash.exe'" | Select-Object -ExpandProperty ProcessId)
 if($taskShells.Count -ne 1){throw 'Expected exactly one owned Bash child.'}
 Invoke-Goal 'diagnostic expand' | Out-Null
 Start-Sleep -Milliseconds 800
 [MiniTermGoalProbe]::FocusByClick($taskWindow)
 Start-Sleep -Milliseconds 150
 if([MiniTermGoalProbe]::GetForegroundWindow() -ne $taskWindow){
   Save-GoalImage 'focus-unavailable.png'
   @{window=$taskWindow.ToInt64();foreground=[MiniTermGoalProbe]::GetForegroundWindow().ToInt64();bounds=[MiniTermGoalProbe]::Bounds($taskWindow)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskFixture 'focus.json') -Encoding UTF8
   throw 'Fixture did not gain focus; no input was sent.'
 }
 [MiniTermGoalProbe]::Text($taskWindow,'codex --no-daemon --strict-config resume --last --all')
 [MiniTermGoalProbe]::Press($taskWindow,13)
 Get-GoalWidget 'fixture-goal-choice' | Out-Null
 $taskDockDeadline=[DateTime]::UtcNow.AddSeconds(5);$taskStable=0;$taskPreviousBounds=''
 do {
   $taskLayout=(Invoke-Goal 'sessions') | ConvertFrom-Json
   $taskChooser=$taskLayout.details.ui.PSObject.Properties['fixture-goal-choice'].Value
   $taskBody=$taskLayout.details.ui.PSObject.Properties['terminal'].Value
   $taskBoundsKey=@($taskChooser+$taskBody)-join ','
   $taskAtInput=$null -ne $taskChooser -and $null -ne $taskBody -and $taskChooser[1] -ge $taskBody[3]-220 -and $taskChooser[3] -ge $taskBody[3]-45
   if($taskAtInput -and $taskBoundsKey -eq $taskPreviousBounds){$taskStable++}else{$taskStable=0}
   $taskPreviousBounds=$taskBoundsKey
   if($taskStable -ge 2){break}
   Start-Sleep -Milliseconds 100
 }while([DateTime]::UtcNow -lt $taskDockDeadline)
 if($taskStable -lt 2){
   Save-GoalImage 'failed-floating.png'
   @{choice=$taskChooser;body=$taskBody} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskFixture 'failed-layout.json') -Encoding UTF8
   throw 'Actual Codex goal chooser is floating above the input region.'
 }
 Save-GoalImage 'goal-docked-dark.png'
 $taskBatteryLog=Get-Content -LiteralPath (Join-Path $taskFixture 'battery-debug.jsonl') -Encoding UTF8 -Raw
 if($taskBatteryLog -notmatch '"layout":"inline"' -or $taskBatteryLog -notmatch '"event":"overlay:start"' -or $taskBatteryLog -notmatch '"--no-alt-screen"'){throw 'AI Battery direct inline overlay and inline Codex execution were not observed.'}
 Invoke-Goal 'diagnostic theme-light' | Out-Null
 Start-Sleep -Milliseconds 200
 Save-GoalImage 'goal-docked-light.png'
 Invoke-Goal 'diagnostic theme-dark' | Out-Null
 [MiniTermGoalProbe]::Press($taskWindow,40)
 [MiniTermGoalProbe]::Press($taskWindow,13)
 Start-Sleep -Milliseconds 500
 $taskAfter=(Invoke-Goal 'sessions') | ConvertFrom-Json
 if($null -ne $taskAfter.details.ui.PSObject.Properties['fixture-goal-choice']){Save-GoalImage 'failed-dismissal.png';throw 'Dismissed goal chooser remains visible.'}
 Save-GoalImage 'goal-dismissed.png'
 Invoke-Goal 'diagnostic quit' | Out-Null
 if(-not $taskApp.WaitForExit(5000)){throw 'Fixture GUI did not close.'}
 foreach($taskShell in $taskShells){if(Get-Process -Id $taskShell -ErrorAction SilentlyContinue){throw 'Owned Bash survived the fixture GUI exit.'}}
 foreach($taskUserApp in $taskUserApps){$taskExisting=Get-Process -Id $taskUserApp.id -ErrorAction Stop;if($taskExisting.StartTime -ne $taskUserApp.started){throw 'User app identity changed.'}}
 $taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash;actualNativeCodex=$true;actualBashBridge=$true;actualBatteryInlineOverlay=$true;inlineArgumentObserved=$true;goalChoiceBounds=$taskChooser;terminalBounds=$taskBody;goalDocked=$true;dismissed=$true;darkLightCaptures=$true;userSessionPreserved=$true;ownedBashProcessIds=$taskShells;ownedBashShutdownConfirmed=$true;scope='Real GUI + installed native CLI and AI Battery inline overlay. Own paused-goal history and localhost HTTP-error provider; fixed battery reading. Excludes account/model responses, Korean IME and ongoing real tool output.'}
 [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
 $taskResult | ConvertTo-Json
} catch {
 try { Save-GoalImage 'failure.png' } catch {}
 try { [IO.File]::WriteAllText((Join-Path $taskFixture 'failure-snapshot.json'),(Invoke-Goal 'sessions'),[Text.UTF8Encoding]::new($false)) } catch {}
 throw
} finally {
 if($taskApp -and -not $taskApp.HasExited){try{Invoke-Goal 'diagnostic quit' | Out-Null}catch{};if(-not $taskApp.WaitForExit(3000)){$taskApp.Kill()}}
 [MiniTermGoalProbe]::SetForegroundWindow($taskOriginalFocus) | Out-Null
 [MiniTermGoalProbe]::SetCursorPos($taskOriginalPointer.X,$taskOriginalPointer.Y) | Out-Null
 [MiniTermGoalProbe]::SetThreadDpiAwarenessContext($taskDpiContext) | Out-Null
}
