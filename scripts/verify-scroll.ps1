param([Parameter(Mandatory=$true)][string]$Fixture,[string]$BatteryLayout='', [string]$Executable='artifacts/miniterm.exe',[switch]$Shared,[switch]$Retry,[switch]$CodexNotify)
$ErrorActionPreference='Stop'
$Streaming=$env:MINITERM_SCROLL_STREAMING -eq '1'
$taskRoot=Split-Path -Parent $PSScriptRoot
$taskExecutable=[IO.Path]::GetFullPath((Join-Path $taskRoot $Executable))
$taskFixture=[IO.Path]::GetFullPath($Fixture)
$taskMarker=Get-Content -LiteralPath (Join-Path $taskFixture 'fixture-owned.json') -Encoding UTF8 -Raw | ConvertFrom-Json
if ((Split-Path -Parent $taskFixture) -ne (Join-Path $taskRoot 'artifacts') -or (Split-Path -Leaf $taskFixture) -notmatch '^scroll-[0-9a-f-]{36}$' -or $taskMarker.kind -ne 'miniterm-local-scroll' -or $taskMarker.root -ne $taskFixture -or (-not $Shared -and $taskMarker.home -ne (Join-Path $taskFixture 'codex-home')) -or ($Shared -and $taskMarker.home -notmatch ('^'+[regex]::Escape($env:LOCALAPPDATA)+'\\mt-s-[0-9a-f]{8}\\h$')) -or (Test-Path -LiteralPath (Join-Path $taskMarker.home 'auth.json'))) { throw 'Expected an owned account-free GUI fixture.' }
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MiniTermScrollProbe {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
 [StructLayout(LayoutKind.Sequential)] public struct Mouse { public int X,Y; public uint Data,Flags,Time; public UIntPtr Extra; }
 [StructLayout(LayoutKind.Sequential)] public struct Key { public ushort Vk,Scan; public uint Flags,Time; public UIntPtr Extra; }
 [StructLayout(LayoutKind.Explicit)] public struct Union { [FieldOffset(0)] public Mouse Mouse; [FieldOffset(0)] public Key Key; }
 [StructLayout(LayoutKind.Sequential)] public struct Input { public uint Type; public Union Data; }
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,System.Text.StringBuilder name,int size);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point p);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern uint SendInput(uint n,Input[] i,int size);
 static Input KeyEvent(ushort vk,ushort scan,uint flags) { return new Input{Type=1,Data=new Union{Key=new Key{Vk=vk,Scan=scan,Flags=flags}}}; }
 static void Send(IntPtr h,Input[] input) {
   if(GetForegroundWindow()!=h){uint pid;var foreground=GetForegroundWindow();GetWindowThreadProcessId(foreground,out pid);var name=new System.Text.StringBuilder(128);GetClassName(foreground,name,name.Capacity);throw new Exception("Fixture lost focus; foreground pid="+pid+", class="+name+"; input cancelled");}
   if(SendInput((uint)input.Length,input,Marshal.SizeOf(typeof(Input)))!=input.Length)throw new Exception("SendInput failed");
 }
 public static void Text(IntPtr h,string text) {
   foreach(var c in text){Send(h,new[]{KeyEvent(0,c,4),KeyEvent(0,c,6)});System.Threading.Thread.Sleep(30);}
 }
 public static void Press(IntPtr h,ushort vk) { Send(h,new[]{KeyEvent(vk,0,0),KeyEvent(vk,0,2)}); }
 public static void FocusByClick(IntPtr h) {
   var r=Bounds(h);SetCursorPos(r.Left+30,r.Top+55);
   mouse_event(2,0,0,0,UIntPtr.Zero);System.Threading.Thread.Sleep(30);mouse_event(4,0,0,0,UIntPtr.Zero);
 }
 public static Rect Bounds(IntPtr h) { Rect r;if(!GetWindowRect(h,out r))throw new Exception("Window unavailable");return r; }
}
'@
$taskOriginalFocus=[MiniTermScrollProbe]::GetForegroundWindow()
$taskOriginalPointer=New-Object MiniTermScrollProbe+Point
[MiniTermScrollProbe]::GetCursorPos([ref]$taskOriginalPointer) | Out-Null
$taskDpiContext=[MiniTermScrollProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$taskUserApps=@(Get-CimInstance Win32_Process -Filter "Name='miniterm.exe'" | Where-Object {
 # IPC helpers share the executable name and can finish during this check.
 # Only no-argument GUI instances belong in the existing-app preservation check.
 $_.CommandLine -match '^"[^"]+"\s*$|^[^"\s]+\s*$'
} | ForEach-Object { $taskExisting=Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; if($taskExisting){[pscustomobject]@{id=$taskExisting.Id;started=$taskExisting.StartTime}} })
$taskId=[Guid]::NewGuid().ToString()
$taskSettings=@{schemaVersion=1;theme='dark';shortcut='';notifications=$false;profile=@{kind='git_bash';directory=$taskMarker.work};window=@{anchor=@{right=7200;bottom=57};miniWidth=680;miniHeight=112;expandedHeight=900;autoCollapse=$false};prompt=@{showUser=$false;showHost=$false;showPath=$false;symbol='QA>'}}
[IO.File]::WriteAllText((Join-Path $taskFixture 'settings.json'),($taskSettings | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
function New-ScrollStart([string]$Arguments) {
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
 foreach ($taskName in @('OPENAI_API_KEY','CODEX_API_KEY','OPENAI_BASE_URL','MINITERM_CODEX_INLINE','MINITERM_SESSION_ID','MINITERM_NOTIFY_EXE','MINITERM_METRICS_PATH','MINITERM_AGENT_BASH','MINITERM_CODEX_SESSION_NOTIFY','CODEX_DAEMON_SHUTDOWN_SOCKET','CODEX_SESSION_ID','CODEX_THREAD_ID','AI_BATTERY_WRAPPED_CODEX','AI_BATTERY_DISABLE_WINDOWS_CODEX_RUNNER','AI_BATTERY_WIN_LAYOUT','AI_BATTERY_ACTIVE_LAYOUT','AI_BATTERY_ORIGINAL_CODEX')) { $taskStart.EnvironmentVariables.Remove($taskName) }
 if($BatteryLayout -eq 'direct'){
  $taskDirect=Join-Path $env:APPDATA 'npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe'
  if(-not (Test-Path -LiteralPath $taskDirect)){throw 'Installed native Codex is required.'}
  $taskStart.EnvironmentVariables['PATH']=(Split-Path -Parent $taskDirect)+';'+$taskStart.EnvironmentVariables['PATH']
 }elseif($BatteryLayout){$taskStart.EnvironmentVariables['AI_BATTERY_WIN_LAYOUT']=$BatteryLayout}
 return $taskStart
}
function Invoke-Scroll([string]$Arguments) {
 $taskStart=New-ScrollStart $Arguments;$taskStart.RedirectStandardOutput=$true;$taskStart.RedirectStandardError=$true
 $taskHelper=[Diagnostics.Process]::Start($taskStart)
 try {
   $taskOut=$taskHelper.StandardOutput.ReadToEndAsync();$taskErr=$taskHelper.StandardError.ReadToEndAsync()
   if(-not $taskHelper.WaitForExit(10000)){ $taskHelper.Kill();throw 'Fixture helper timed out.' }
   if($taskHelper.ExitCode -ne 0){throw $taskErr.Result}
   return $taskOut.Result
 } finally {$taskHelper.Dispose()}
}
function Save-ConsoleMode([string]$Stage) {
 if($env:MINITERM_SCROLL_CONSOLE_MODE_PROBE -ne '1'){return}
 $taskAll=@(Get-CimInstance Win32_Process)
 $taskOwned=@{};$taskOwned[$taskApp.Id]=$true
 do{
  $taskAdded=$false
  foreach($taskInfo in $taskAll){if($taskOwned.ContainsKey([int]$taskInfo.ParentProcessId) -and -not $taskOwned.ContainsKey([int]$taskInfo.ProcessId)){$taskOwned[[int]$taskInfo.ProcessId]=$true;$taskAdded=$true}}
 }while($taskAdded)
 $taskFront=@($taskAll | Where-Object {$taskOwned.ContainsKey([int]$_.ProcessId) -and $_.Name -eq 'codex.exe' -and $_.CommandLine -notmatch 'app-server|exec-server'})
 if($taskFront.Count -ne 1){throw 'Console mode probe requires one owned native Codex frontend.'}
 $taskProbeStart=New-Object Diagnostics.ProcessStartInfo
 $taskProbeStart.FileName=Join-Path $taskRoot 'artifacts/experiments/console-mode-probe.exe'
 $taskProbeStart.Arguments=([int]$taskFront[0].ProcessId).ToString()+' "'+(Join-Path $taskFixture ('console-mode-'+$Stage+'.json'))+'"'
 $taskProbeStart.UseShellExecute=$false;$taskProbeStart.CreateNoWindow=$true
 $taskProbeProcess=[Diagnostics.Process]::Start($taskProbeStart)
 try{if(-not $taskProbeProcess.WaitForExit(2000)){$taskProbeProcess.Kill();throw 'Owned console-mode helper exceeded its time limit.'}}finally{$taskProbeProcess.Dispose()}
}
function Get-ScrollWidget([string]$Name) {
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(12)
 do {
   $taskReply=(Invoke-Scroll 'sessions') | ConvertFrom-Json
   $taskProperty=$taskReply.details.ui.PSObject.Properties[$Name]
   if($null -ne $taskProperty){return $taskProperty.Value}
   Start-Sleep -Milliseconds 100
 } while([DateTime]::UtcNow -lt $taskDeadline)
 throw "Widget not visible: $Name"
}
function Save-ScrollImage([string]$Name) {
 $taskBounds=[MiniTermScrollProbe]::Bounds($taskWindow)
 $taskImage=New-Object Drawing.Bitmap ($taskBounds.Right-$taskBounds.Left),($taskBounds.Bottom-$taskBounds.Top)
 $taskGraphics=[Drawing.Graphics]::FromImage($taskImage)
 try {$taskGraphics.CopyFromScreen($taskBounds.Left,$taskBounds.Top,0,0,$taskImage.Size);$taskImage.Save((Join-Path $taskFixture $Name),[Drawing.Imaging.ImageFormat]::Png)}finally{$taskGraphics.Dispose();$taskImage.Dispose()}
}

function Get-ScrollState { (Invoke-Scroll 'sessions') | ConvertFrom-Json }
function Wait-ScrollSettled {
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(8)
 $taskPrevious=(Get-ScrollState).details.terminalScroll
 $taskStableSince=[DateTime]::UtcNow
 do {
  Start-Sleep -Milliseconds 100
  $taskCurrent=(Get-ScrollState).details.terminalScroll
  if($taskCurrent.wheelEvents -ne $taskPrevious.wheelEvents -or $taskCurrent.displayOffset -ne $taskPrevious.displayOffset){$taskStableSince=[DateTime]::UtcNow}
  if(([DateTime]::UtcNow-$taskStableSince).TotalMilliseconds -ge 400){return}
  $taskPrevious=$taskCurrent
 }while([DateTime]::UtcNow -lt $taskDeadline)
 throw 'Owned wheel animation did not settle.'
}
function Wheel([int]$Delta,[switch]$Live) {
 $taskWheelBefore=(Get-ScrollState).details.terminalScroll.wheelEvents
 $taskBody=Get-ScrollWidget 'terminal'
 $taskBounds=[MiniTermScrollProbe]::Bounds($taskWindow)
 $taskScale=[MiniTermScrollProbe]::GetDpiForWindow($taskWindow)/96.0
 [MiniTermScrollProbe]::SetCursorPos($taskBounds.Left+[int](($taskBody[0]+$taskBody[2])*0.5*$taskScale),$taskBounds.Top+[int](($taskBody[1]+$taskBody[3])*0.5*$taskScale)) | Out-Null
 $taskPoint=New-Object MiniTermScrollProbe+Point
 [MiniTermScrollProbe]::GetCursorPos([ref]$taskPoint) | Out-Null
 if([MiniTermScrollProbe]::WindowFromPoint($taskPoint) -ne $taskWindow){throw 'Own terminal is covered; wheel cancelled.'}
 # Let the native pointer move reach winit before sending a wheel at its new location.
 Start-Sleep -Milliseconds 150
 # Exercise the real Win32 -> winit -> egui route on our own window. A global
 # mouse_event injection was observed with the opposite sign on this desktop;
 # its cause is unconfirmed. Keep that separate from the app's scroll policy.
 $taskWheelParam=[IntPtr](([int64]$Delta -band 0xffff)*65536)
 $taskWheelPoint=[IntPtr](([int64]$taskPoint.Y -shl 16) -bor ($taskPoint.X -band 0xffff))
 if(-not [MiniTermScrollProbe]::PostMessage($taskWindow,0x20a,$taskWheelParam,$taskWheelPoint)){throw 'Own wheel message rejected.'}
 if($Live){
  # A live stream keeps changing displayOffset; waiting for an idle viewport
  # would accidentally test the completed response instead of concurrency.
  $taskWheelDeadline=[DateTime]::UtcNow.AddSeconds(3)
  do{Start-Sleep -Milliseconds 80;$taskWheelAfter=(Get-ScrollState).details.terminalScroll.wheelEvents}while($taskWheelAfter -le $taskWheelBefore -and [DateTime]::UtcNow -lt $taskWheelDeadline)
  if($taskWheelAfter -le $taskWheelBefore){throw 'Native wheel did not reach the GUI while streaming.'}
 }else{Wait-ScrollSettled}
}
function Start-ScrollGui {
 $script:taskApp=[Diagnostics.Process]::Start((New-ScrollStart ''))
 $script:taskGuiRecords+=@{pid=$taskApp.Id;started=$taskApp.StartTime.ToFileTimeUtc().ToString();instance=$taskId}
 [IO.File]::WriteAllText((Join-Path $taskFixture 'owned-gui-records.json'),(ConvertTo-Json -InputObject @($taskGuiRecords)),[Text.UTF8Encoding]::new($false))
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(20)
 do {
  Start-Sleep -Milliseconds 100;$taskApp.Refresh();$taskReady=$false
  if($taskApp.MainWindowHandle -ne [IntPtr]::Zero){$taskBounds=[MiniTermScrollProbe]::Bounds($taskApp.MainWindowHandle);$taskReady=[MiniTermScrollProbe]::IsWindowVisible($taskApp.MainWindowHandle) -and $taskBounds.Right-$taskBounds.Left -gt 100}
 }while(-not $taskReady -and -not $taskApp.HasExited -and [DateTime]::UtcNow -lt $taskDeadline)
 if($taskApp.HasExited -or -not $taskReady){throw 'Fixture GUI did not start.'}
 $script:taskWindow=$taskApp.MainWindowHandle
 Get-ScrollWidget 'terminal' | Out-Null
 Invoke-Scroll 'diagnostic expand' | Out-Null
 Start-Sleep -Milliseconds 1200
 [MiniTermScrollProbe]::SetForegroundWindow($taskWindow) | Out-Null
 Start-Sleep -Milliseconds 100
 if([MiniTermScrollProbe]::GetForegroundWindow() -ne $taskWindow){
  $taskBounds=[MiniTermScrollProbe]::Bounds($taskWindow)
  $taskScale=[MiniTermScrollProbe]::GetDpiForWindow($taskWindow)/96.0
  [MiniTermScrollProbe]::SetCursorPos($taskBounds.Left+[int](12*$taskScale),$taskBounds.Top+[int](14*$taskScale)) | Out-Null
  $taskPointer=New-Object MiniTermScrollProbe+Point
  [MiniTermScrollProbe]::GetCursorPos([ref]$taskPointer) | Out-Null
  if([MiniTermScrollProbe]::WindowFromPoint($taskPointer) -ne $taskWindow){Save-ScrollImage 'focus-covered.png';throw 'Owned window is covered at focus point.'}
  [MiniTermScrollProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
  [MiniTermScrollProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
 }
 Start-Sleep -Milliseconds 200
 if([MiniTermScrollProbe]::GetForegroundWindow() -ne $taskWindow){throw 'Own GUI is not focused.'}
}
function Close-ScrollGui {
 Invoke-Scroll 'diagnostic quit' | Out-Null
 if(-not $taskApp.WaitForExit(5000)){throw 'Owned GUI did not stop.'}
}
function Wait-ScrollRequests([string]$Prompt) {
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(20)
 do {
  Start-Sleep -Milliseconds 100
  $taskRequests=Get-Content -LiteralPath (Join-Path $taskFixture 'requests.json') -Encoding UTF8 -Raw | ConvertFrom-Json
  foreach($taskRequest in @($taskRequests)){
   $taskLastUser=@($taskRequest.input | Where-Object role -eq 'user') | Select-Object -Last 1
   if(@($taskLastUser.content | Where-Object {$_.type -eq 'input_text' -and $_.text -eq $Prompt}).Count -gt 0){Start-Sleep -Milliseconds 1500;return}
  }
 }while([DateTime]::UtcNow -lt $taskDeadline)
 throw "Owned provider did not receive prompt $Prompt."
}
function Wait-ScrollWriter([bool]$Blocked) {
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(20)
 do {
  $taskState=(Get-ScrollState).details.terminalScroll
  if($taskState.codexWriterBlocked -eq $Blocked){return}
  Start-Sleep -Milliseconds 100
 }while([DateTime]::UtcNow -lt $taskDeadline)
 Save-ScrollImage 'writer-wait-failed.png'
 throw "Owned Codex writer state did not become $Blocked."
}
function Get-ScrollFrontends {
 $taskAll=@(Get-CimInstance Win32_Process)
 $taskOwned=@{};$taskOwned[[int]$taskApp.Id]=$true
 do {
  $taskAdded=$false
  foreach($taskInfo in $taskAll){if($taskOwned.ContainsKey([int]$taskInfo.ParentProcessId) -and -not $taskOwned.ContainsKey([int]$taskInfo.ProcessId)){$taskOwned[[int]$taskInfo.ProcessId]=$true;$taskAdded=$true}}
 }while($taskAdded)
 @($taskAll | Where-Object {$taskOwned.ContainsKey([int]$_.ProcessId) -and $_.Name -eq 'codex.exe' -and $_.CommandLine -notmatch 'app-server|exec-server'})
}
$taskApp=$null
$taskGuiRecords=@()
try {
 if($Retry -and -not $Shared){throw 'Retry requires the owned shared backend.'}
 Start-ScrollGui
 $taskSessions=(Get-ScrollState).sessions
 if($CodexNotify){
  $taskNotificationSession=$taskSessions[0]
  Invoke-Scroll 'diagnostic new-session'|Out-Null
  $taskDeadline=[DateTime]::UtcNow.AddSeconds(10)
  do{Start-Sleep -Milliseconds 100;$taskSessions=(Get-ScrollState).sessions}while($taskSessions.Count -ne 2 -and [DateTime]::UtcNow -lt $taskDeadline)
  if($taskSessions.Count -ne 2){throw 'Second notification fixture tab did not start.'}
  Invoke-Scroll 'diagnostic key:ctrl+alt+1'|Out-Null
  Start-Sleep -Milliseconds 300
 }
 $taskProbeLog=Join-Path $taskFixture 'console-input.log'
 $taskLaunch=if($BatteryLayout -eq 'probe'){'"'+(Join-Path $taskRoot 'artifacts/experiments/console-input-probe.exe').Replace('\','/')+'" "'+$taskProbeLog.Replace('\','/')+'" 520'}elseif($Shared){'MINITERM_SCROLL_CALLER=OWNED-ENV codex'}else{'codex --no-daemon --strict-config'}
 if($env:MINITERM_SCROLL_LAUNCH_TRACE -eq '1'){$taskLaunch+='; printf ''__CODEX_EXIT_%s__\n'' "$?"'}
 [MiniTermScrollProbe]::Text($taskWindow,$taskLaunch)
 [MiniTermScrollProbe]::Press($taskWindow,13)
 if($BatteryLayout -eq 'probe'){
  Start-Sleep -Milliseconds 900
  [MiniTermScrollProbe]::Text($taskWindow,'SCROLL-FIRST-TURN')
  Start-Sleep -Milliseconds 600
  [MiniTermScrollProbe]::Press($taskWindow,13)
  $taskDeadline=[DateTime]::UtcNow.AddSeconds(5)
  do{Start-Sleep -Milliseconds 100;$taskProbe=if(Test-Path -LiteralPath $taskProbeLog){Get-Content -LiteralPath $taskProbeLog -Encoding UTF8 -Raw}else{''}}while($taskProbe -notmatch 'LINE:0:SCROLL-FIRST-TURN' -and [DateTime]::UtcNow -lt $taskDeadline)
  if($taskProbe -notmatch 'LINE:0:SCROLL-FIRST-TURN'){throw 'Native console did not receive text and Enter.'}
  [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),(@{passed=$true;nativeConsoleReceived=$true;fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExecutable).Hash}|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
  Invoke-Scroll 'diagnostic quit' | Out-Null
  return
 }
 $taskStartupDeadline=[DateTime]::UtcNow.AddSeconds(30)
 do{$taskStartup=Get-ScrollState;if($taskStartup.details.terminalScroll.codexReady){break};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $taskStartupDeadline)
 if(-not $taskStartup.details.terminalScroll.codexReady){Save-ScrollImage 'startup-not-ready.png';throw 'Owned Codex did not render its startup banner.'}
 Start-Sleep -Milliseconds 3500
 Save-ScrollImage 'codex-startup.png'
 Save-ConsoleMode 'startup'
 [MiniTermScrollProbe]::Text($taskWindow,'SCROLL-FIRST-TURN')
 # Separate typing from Enter: Codex's paste-burst detection otherwise consumes
 # an immediately following Enter as part of synthetic pasted input.
 Start-Sleep -Milliseconds 600
 Save-ScrollImage 'first-typed-before-enter.png'
 Save-ConsoleMode 'before-enter'
 [MiniTermScrollProbe]::Press($taskWindow,13)
 if($CodexNotify){Invoke-Scroll 'diagnostic key:ctrl+alt+2'|Out-Null}
 if($Streaming){
  if($CodexNotify){throw 'Streaming input check requires its own active tab.'}
  $taskStreamDeadline=[DateTime]::UtcNow.AddSeconds(15)
  do{
   Start-Sleep -Milliseconds 100
   $taskProgressPath=Join-Path $taskFixture 'stream-progress.json'
   $taskStreamProgress=if(Test-Path -LiteralPath $taskProgressPath){Get-Content -LiteralPath $taskProgressPath -Encoding UTF8 -Raw|ConvertFrom-Json}else{$null}
  }while((!$taskStreamProgress -or $taskStreamProgress.chunk -lt 30) -and [DateTime]::UtcNow -lt $taskStreamDeadline)
  if(!$taskStreamProgress -or $taskStreamProgress.chunk -lt 30){throw 'Native Codex did not begin its streaming response.'}
  $taskStreamingBefore=Get-ScrollState
  [MiniTermScrollProbe]::Text($taskWindow,'SCROLL-DRAFT-KEEP')
  Wheel 1200 -Live
  $taskStreamingUp=Get-ScrollState
  if($taskStreamingUp.details.terminalScroll.displayOffset -le 0){throw 'Cannot scroll during streaming output.'}
  Save-ScrollImage 'streaming-wheel-up.png'
  Invoke-Scroll 'diagnostic collapse'|Out-Null
  Start-Sleep -Milliseconds 400
  Invoke-Scroll 'diagnostic expand'|Out-Null
  Start-Sleep -Milliseconds 400
  $taskStreamingAfter=Get-ScrollState
  $taskStreamProgress=Get-Content -LiteralPath $taskProgressPath -Encoding UTF8 -Raw|ConvertFrom-Json
  if($taskStreamProgress.chunk -ge $taskStreamProgress.total){throw 'Stream completed before the concurrent input/scroll/resize check.'}
  if($taskStreamingAfter.details.terminalScroll.historyLines -le $taskStreamingBefore.details.terminalScroll.historyLines){throw 'Visible output stopped advancing during concurrent input/scroll/resize.'}
  [IO.File]::WriteAllText((Join-Path $taskFixture 'streaming-snapshots.json'),(@{before=$taskStreamingBefore;up=$taskStreamingUp;after=$taskStreamingAfter;provider=$taskStreamProgress}|ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
  Wheel -12000 -Live
 }
 $taskDeadline=[DateTime]::UtcNow.AddSeconds(25)
 do {Start-Sleep -Milliseconds 100}while(-not (Test-Path -LiteralPath (Join-Path $taskFixture 'first-turn-completed.json')) -and [DateTime]::UtcNow -lt $taskDeadline)
 if(-not (Test-Path -LiteralPath (Join-Path $taskFixture 'first-turn-completed.json'))){
  Save-ScrollImage 'no-request.png'
  Save-ConsoleMode 'after-enter'
  # Diagnose the GUI event path with one egui Enter after a failed native Enter.
  # Keep the whole check failed; this is evidence, never a successful retry or
  # a product delay/replay workaround. The fixture contains only fixed input.
  $taskProbeBefore=Get-ScrollState
  Invoke-Scroll 'diagnostic key:Enter' | Out-Null
  $taskProbeDeadline=[DateTime]::UtcNow.AddSeconds(3)
  do{Start-Sleep -Milliseconds 100}while(-not (Test-Path -LiteralPath (Join-Path $taskFixture 'requests.json')) -and [DateTime]::UtcNow -lt $taskProbeDeadline)
  $taskEnterProbe=@{nativeEnterSubmitted=$false;eguiEnterSubmitted=(Test-Path -LiteralPath (Join-Path $taskFixture 'requests.json'));before=$taskProbeBefore;after=(Get-ScrollState)}
  [IO.File]::WriteAllText((Join-Path $taskFixture 'first-enter-probe.json'),($taskEnterProbe | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
  Save-ScrollImage 'after-egui-enter.png'
  throw 'Actual CLI did not call the local fixture after native Enter.'
 }
 Start-Sleep -Milliseconds 1500
 if($CodexNotify){
  $taskNotificationState=Get-ScrollState
  [IO.File]::WriteAllText((Join-Path $taskFixture 'shared-notification.json'),($taskNotificationState|ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
  if($taskNotificationState.details.states.$taskNotificationSession -ne 'complete' -or $taskNotificationState.details.unread -notcontains $taskNotificationSession -or $taskNotificationState.details.noticeCount -ne 1){throw 'Shared turn did not alert exactly its originating background tab.'}
  if(@($taskNotificationState.details.states.PSObject.Properties|Where-Object {$_.Name -ne $taskNotificationSession -and $_.Value -ne 'unknown'}).Count -ne 0){throw 'Shared notification changed an unrelated tab.'}
  Invoke-Scroll 'diagnostic key:ctrl+alt+1'|Out-Null
  Start-Sleep -Milliseconds 300
 }
 Wait-ScrollSettled
 if(-not $Streaming){[MiniTermScrollProbe]::Text($taskWindow,'SCROLL-DRAFT-KEEP')}
 Start-Sleep -Milliseconds 150
 Save-ScrollImage 'before-wheel.png'
 $taskBefore=Get-ScrollState
 Wheel 1200
 $taskUp=Get-ScrollState
 Save-ScrollImage 'after-wheel-up.png'
 Wheel -1200
 $taskDown=Get-ScrollState
 Save-ScrollImage 'after-wheel-down.png'
 [IO.File]::WriteAllText((Join-Path $taskFixture 'scroll-snapshots.json'),(@{before=$taskBefore;up=$taskUp;down=$taskDown}|ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
 if($null -eq $taskUp.details.ui.PSObject.Properties['terminal-scrolled']){throw 'Wheel did not move the terminal into output history.'}
 if($null -ne $taskDown.details.ui.PSObject.Properties['terminal-scrolled']){throw 'Wheel down did not return to live output.'}
 # Compact input must also render history, rather than staying anchored to the cursor.
 Invoke-Scroll 'diagnostic collapse' | Out-Null
 Start-Sleep -Milliseconds 650
 Wheel 1200
 $taskMiniUp=Get-ScrollState
 Save-ScrollImage 'compact-wheel-up.png'
 if($taskMiniUp.details.terminalScroll.displayOffset -le 0){throw 'Compact wheel did not move into history.'}
 Wheel -1200
 if((Get-ScrollState).details.terminalScroll.displayOffset -ne 0){throw 'Compact wheel did not return to live input.'}
 Invoke-Scroll 'diagnostic expand' | Out-Null
 Start-Sleep -Milliseconds 650
 [MiniTermScrollProbe]::Text($taskWindow,'-TAIL')
 Start-Sleep -Milliseconds 600
 [MiniTermScrollProbe]::Press($taskWindow,13)
 Wait-ScrollRequests 'SCROLL-DRAFT-KEEP-TAIL'
 if(((Get-ScrollState).sessions -join ',') -ne ($taskSessions -join ',')){throw 'Scrolling changed the session.'}
 Start-Sleep -Milliseconds 400
 Save-ScrollImage 'draft-submitted.png'
 if($Streaming){
  [MiniTermScrollProbe]::Text($taskWindow,'SCROLL-CANCEL-TURN')
  Start-Sleep -Milliseconds 600
  [MiniTermScrollProbe]::Press($taskWindow,13)
  $taskCancelDeadline=[DateTime]::UtcNow.AddSeconds(10)
  do{Start-Sleep -Milliseconds 100}while(-not (Test-Path -LiteralPath (Join-Path $taskFixture 'cancel-waiting.json')) -and [DateTime]::UtcNow -lt $taskCancelDeadline)
  if(-not (Test-Path -LiteralPath (Join-Path $taskFixture 'cancel-waiting.json'))){throw 'Owned response did not enter its deliberate wait.'}
  Start-Sleep -Milliseconds 600
  [MiniTermScrollProbe]::Press($taskWindow,27)
  $taskCancelDeadline=[DateTime]::UtcNow.AddSeconds(10)
  do{Start-Sleep -Milliseconds 100}while(-not (Test-Path -LiteralPath (Join-Path $taskFixture 'cancel-closed.json')) -and [DateTime]::UtcNow -lt $taskCancelDeadline)
  if(-not (Test-Path -LiteralPath (Join-Path $taskFixture 'cancel-closed.json'))){throw 'Escape did not cancel the stalled response.'}
  [MiniTermScrollProbe]::Text($taskWindow,'SCROLL-AFTER-CANCEL')
  Start-Sleep -Milliseconds 600
  [MiniTermScrollProbe]::Press($taskWindow,13)
  Wait-ScrollRequests 'SCROLL-AFTER-CANCEL'
  Save-ScrollImage 'after-cancel-response.png'
 }
 if($env:MINITERM_SCROLL_IDLE_SECONDS -eq '30'){
  & (Join-Path $PSScriptRoot 'measure-idle.ps1') -ProcessId $taskApp.Id -Seconds 30 -OutputPath (Join-Path $taskFixture 'codex-idle.json')|Out-Null
 }
 Close-ScrollGui
 if($Retry){
  # Closing the frontend preserves the owned shared server and its conversation.
  Start-ScrollGui
  [MiniTermScrollProbe]::Text($taskWindow,'codex resume --last --all SCROLL-RESUME-HISTORY')
  [MiniTermScrollProbe]::Press($taskWindow,13)
  Wait-ScrollRequests 'SCROLL-RESUME-HISTORY'
  if((Get-ScrollState).details.terminalScroll.codexWriterBlocked){throw 'Default resume opened a private competing writer.'}
  Save-ScrollImage 'shared-resumed.png'
  Close-ScrollGui

  # An explicit private backend is allowed to report a real writer conflict.
  # Exercise lowercase r while held, then after releasing ONLY this fixture.
  Start-ScrollGui
  [MiniTermScrollProbe]::Text($taskWindow,'codex --no-daemon resume --last --all')
  [MiniTermScrollProbe]::Press($taskWindow,13)
  Wait-ScrollWriter $true
  Save-ScrollImage 'private-writer-blocked.png'
  [MiniTermScrollProbe]::Text($taskWindow,'r')
  Start-Sleep -Milliseconds 1000
  if(-not (Get-ScrollState).details.terminalScroll.codexWriterBlocked){throw 'Private backend unexpectedly took the active writer.'}
  $taskNative=Join-Path $env:APPDATA 'npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe'
  $taskStop=New-ScrollStart ''
  $taskStop.FileName=$taskNative;$taskStop.Arguments='app-server daemon stop'
  $taskStop.WorkingDirectory=$taskMarker.work
  $taskStop.RedirectStandardOutput=$true;$taskStop.RedirectStandardError=$true
  $taskStopping=[Diagnostics.Process]::Start($taskStop)
  try {
   $taskStopOut=$taskStopping.StandardOutput.ReadToEndAsync();$taskStopErr=$taskStopping.StandardError.ReadToEndAsync()
   if(-not $taskStopping.WaitForExit(15000)){$taskStopping.Kill();throw 'Owned daemon stop timed out.'}
   if($taskStopping.ExitCode -ne 0){throw $taskStopErr.Result}
  }finally{$taskStopping.Dispose()}
  [MiniTermScrollProbe]::Text($taskWindow,'r')
  Wait-ScrollWriter $false
  Start-Sleep -Milliseconds 3500
  [MiniTermScrollProbe]::Text($taskWindow,'SCROLL-RETRY-RECOVERED')
  Start-Sleep -Milliseconds 600
  [MiniTermScrollProbe]::Press($taskWindow,13)
  Wait-ScrollRequests 'SCROLL-RETRY-RECOVERED'
  Save-ScrollImage 'private-retry-recovered.png'

  [MiniTermScrollProbe]::Text($taskWindow,'/quit')
  Start-Sleep -Milliseconds 600
  [MiniTermScrollProbe]::Press($taskWindow,13)
  $taskDeadline=[DateTime]::UtcNow.AddSeconds(15)
  # DEC 9001 belongs to ConPTY and may stay enabled in Bash after Codex exits.
  # Verify the owned native frontend's lifetime instead of assuming a mode reset.
  do{Start-Sleep -Milliseconds 250;$taskFrontends=@(Get-ScrollFrontends)}while($taskFrontends.Count -gt 0 -and [DateTime]::UtcNow -lt $taskDeadline)
  Save-ScrollImage 'private-quit.png'
  if($taskFrontends.Count -gt 0){throw 'Private frontend did not return to Bash.'}
  Start-Sleep -Milliseconds 400
  [MiniTermScrollProbe]::Text($taskWindow,'codex resume --last --all SCROLL-FINAL-SHARED')
  [MiniTermScrollProbe]::Press($taskWindow,13)
  Wait-ScrollRequests 'SCROLL-FINAL-SHARED'
  Save-ScrollImage 'final-shared-resumed.png'
  Close-ScrollGui
 }
 foreach($taskUserApp in $taskUserApps){$taskExisting=Get-Process -Id $taskUserApp.id -ErrorAction Stop;if($taskExisting.StartTime -ne $taskUserApp.started){throw 'Existing app identity changed.'}}
 $taskResult=@{fixture=$taskFixture;executableSha256=(Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash;batteryLayout=$BatteryLayout;sharedBackendRequested=[bool]$Shared;actualCodex=$true;wheelInput='native_WM_MOUSEWHEEL';wheelUp=$true;wheelDown=$true;compactWheel=$true;sessionPreserved=$true;sharedResumeChecked=[bool]$Retry;retryWhileOwnedWriterActive=[bool]$Retry;retryAfterOwnedWriterReleased=[bool]$Retry;scope='Real GUI and installed CLI; targeted Win32 wheel messages through winit/egui. Own localhost fixed SSE output and fixed AI Battery. Draft checked separately by provider; no account/model or physical wheel device claim.'}
 $taskResult.sharedNotificationChecked=[bool]$CodexNotify
 $taskResult.streamingInputScrollResize=[bool]$Streaming
 [IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
 $taskResult|ConvertTo-Json
} catch {
 try{[IO.File]::WriteAllText((Join-Path $taskFixture 'failure-snapshot.json'),(Invoke-Scroll 'sessions'),[Text.UTF8Encoding]::new($false))}catch{}
 try{Save-ScrollImage 'failure.png'}catch{}
 throw
} finally {
 if($taskApp -and -not $taskApp.HasExited){try{Invoke-Scroll 'diagnostic quit'|Out-Null}catch{};if(-not $taskApp.WaitForExit(3000)){$taskApp.Kill()}}
 [MiniTermScrollProbe]::SetForegroundWindow($taskOriginalFocus)|Out-Null
 [MiniTermScrollProbe]::SetCursorPos($taskOriginalPointer.X,$taskOriginalPointer.Y)|Out-Null
 [MiniTermScrollProbe]::SetThreadDpiAwarenessContext($taskDpiContext)|Out-Null
}
