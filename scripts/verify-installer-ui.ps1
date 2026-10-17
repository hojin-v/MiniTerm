param([Parameter(Mandatory=$true)][string]$InstallerPath, [Parameter(Mandatory=$true)][string]$InstallDirectory)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') { throw 'UI installation is restricted to the ephemeral Windows runner.' }
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class InstallerUi {
 public class Control { public IntPtr Handle; public int Id; public string Text; public bool Visible; }
 public delegate bool EnumProc(IntPtr handle, IntPtr param);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback,IntPtr param);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent,EnumProc callback,IntPtr param);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr handle,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr handle,StringBuilder text,int count);
 [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr handle);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr handle);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr handle);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr handle,uint message,IntPtr wparam,IntPtr lparam);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr handle,uint message,IntPtr wparam,IntPtr lparam);
 public static IntPtr Window(int process) {
  IntPtr result=IntPtr.Zero;
  EnumWindows((handle,param)=>{uint pid;GetWindowThreadProcessId(handle,out pid);if(pid==process&&IsWindowVisible(handle)){result=handle;return false;}return true;},IntPtr.Zero);
  return result;
 }
 public static Control[] Controls(IntPtr window) {
  var controls=new List<Control>();
  EnumChildWindows(window,(handle,param)=>{var text=new StringBuilder(2048);GetWindowText(handle,text,text.Capacity);controls.Add(new Control{Handle=handle,Id=GetDlgCtrlID(handle),Text=text.ToString(),Visible=IsWindowVisible(handle)});return true;},IntPtr.Zero);
  return controls.ToArray();
 }
}
'@
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskStart = [Diagnostics.ProcessStartInfo]::new()
$taskStart.FileName = (Get-Item -LiteralPath $InstallerPath).FullName
$taskStart.UseShellExecute = $false
$taskStart.WindowStyle = 'Normal'
$taskStart.ArgumentList.Add('/D=' + $InstallDirectory)
$taskProcess = [Diagnostics.Process]::Start($taskStart)
$taskPages = @()
$taskPassed = $false
function Read-InstallerControls {
    $taskProcess.Refresh()
    if ($taskProcess.HasExited) { throw ('Installer exited before Finish: ' + $taskProcess.ExitCode) }
    $taskWindow = [InstallerUi]::Window($taskProcess.Id)
    if ($taskWindow -eq [IntPtr]::Zero) { return @() }
    return @([InstallerUi]::Controls($taskWindow) | Where-Object Visible)
}
function Click-InstallerNext($Controls) {
    $taskNext = @($Controls | Where-Object { $_.Id -eq 1 -and [InstallerUi]::IsWindowEnabled($_.Handle) })
    if ($taskNext.Count -ne 1) { throw 'Expected one enabled installer Next/Install/Finish button.' }
    if (-not [InstallerUi]::PostMessage($taskNext[0].Handle,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)) { throw 'Installer button click failed.' }
}
try {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(20)
    $taskControls = @()
    while ([DateTime]::UtcNow -lt $taskDeadline) {
        $taskControls = @(Read-InstallerControls)
        if ($taskControls.Text -contains '시작 및 바로가기 설정') { break }
        if ($taskControls.Count) {
            $taskPages += ($taskControls.Text -join ' | ')
            Click-InstallerNext $taskControls
        }
        Start-Sleep -Milliseconds 500
    }
    if ($taskControls.Text -notcontains '시작 및 바로가기 설정') { throw 'Installer options page was not reached.' }
    foreach ($taskChoice in @(@{text='시작 메뉴에 MiniTerm 표시 (권장)';state=1},@{text='바탕화면 바로가기 만들기';state=0},@{text='Windows에 로그인할 때 MiniTerm 실행';state=0})) {
        $taskBox = @($taskControls | Where-Object Text -eq $taskChoice.text)
        if ($taskBox.Count -ne 1 -or [InstallerUi]::SendMessage($taskBox[0].Handle,0xF0,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -ne $taskChoice.state) { throw ('Incorrect installer option/default: ' + $taskChoice.text) }
        [InstallerUi]::SendMessage($taskBox[0].Handle,0xF1,[IntPtr]1,[IntPtr]::Zero) | Out-Null
    }
    $taskPages += ($taskControls.Text -join ' | ')
    Click-InstallerNext $taskControls
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(120)
    while ([DateTime]::UtcNow -lt $taskDeadline) {
        Start-Sleep -Milliseconds 250
        $taskControls = @(Read-InstallerControls)
        if ($taskControls.Text -contains 'MiniTerm 설치 완료') { break }
    }
    if ($taskControls.Text -notcontains 'MiniTerm 설치 완료') { throw 'Installer did not reach its success page.' }
    $taskSummary = $taskControls.Text -join ' | '
    if (-not $taskSummary.Contains($InstallDirectory) -or -not $taskSummary.Contains('바탕화면 바로가기를 만들었습니다.') -or -not $taskSummary.Contains('Windows 로그인 시 자동으로 실행합니다.')) { throw 'Finish page does not explain the installation and selected options.' }
    $taskRun = @($taskControls | Where-Object Text -eq 'MiniTerm 지금 실행')
    if ($taskRun.Count -ne 1 -or [InstallerUi]::SendMessage($taskRun[0].Handle,0xF0,[IntPtr]::Zero,[IntPtr]::Zero).ToInt32() -ne 1) { throw 'Finish page must offer launch now, checked by default.' }
    # The runner verifies installer UI without depending on its GPU. Local
    # product GUI tests separately verify startup with the same private Bash.
    [InstallerUi]::SendMessage($taskRun[0].Handle,0xF1,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
    $taskPages += $taskSummary
    Click-InstallerNext $taskControls
    if (-not $taskProcess.WaitForExit(15000) -or $taskProcess.ExitCode -ne 0) { throw 'Installer did not finish successfully.' }
    $taskPassed = $true
} finally {
    if (-not $taskProcess.HasExited) { $taskProcess.Kill($true); $taskProcess.WaitForExit() }
    $taskProcess.Dispose()
    $taskResult = @{passed=$taskPassed;pages=$taskPages;optionsVerified=$taskPassed;finishVerified=$taskPassed;launchOffered=$taskPassed;scope='Targeted Win32 controls of the owned NSIS process on an ephemeral runner. No physical keyboard/mouse or GPU requirement; launch is offered and deselected for this fixture.'}
    [IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/installer-ui-verification.json'),($taskResult | ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
}
