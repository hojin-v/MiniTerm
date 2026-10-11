param()
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskId = [Guid]::NewGuid().ToString()
$taskSession = [Guid]::NewGuid().ToString()
$taskFixture = Join-Path $taskRoot ('artifacts/progress-adapter-' + $taskId)
New-Item -ItemType Directory -Path $taskFixture | Out-Null
$taskPython = (Get-Command python -ErrorAction Stop).Source
$taskScript = Join-Path $taskRoot '.agents/skills/miniterm-progress/scripts/report.py'
$taskResults = @{}
foreach ($taskCase in @('old-helper','old-server','unregistered-session')) {
    $taskReply = @{ok=$true; sessions=@($taskSession)}
    if ($taskCase -ne 'old-helper') { $taskReply.helperCapabilities=@('progress-v1') }
    if ($taskCase -ne 'old-server') { $taskReply.capabilities=@('progress-v1') }
    if ($taskCase -eq 'unregistered-session') { $taskReply.sessions=@() }
    $taskBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($taskReply | ConvertTo-Json -Depth 4 -Compress)))
    $taskHelper = Join-Path $taskFixture ($taskCase + '.exe')
    $taskLog = Join-Path $taskFixture ($taskCase + '.log')
    $taskType = 'ProgressAdapter_' + [Guid]::NewGuid().ToString('N')
    $taskSource = @"
using System;
using System.IO;
using System.Text;
public class $taskType {
    public static int Main(string[] args) {
        File.AppendAllText(Environment.GetEnvironmentVariable("MINITERM_ADAPTER_LOG"), String.Join(" ",args)+"\n");
        if (args.Length==1 && args[0]=="sessions") {
            Console.WriteLine(Encoding.UTF8.GetString(Convert.FromBase64String("$taskBase64")));
            return 0;
        }
        // The real legacy fallback would activate a window. Record any attempt.
        return 91;
    }
}
"@
    Add-Type -TypeDefinition $taskSource -OutputAssembly $taskHelper -OutputType ConsoleApplication
    $taskStart = New-Object Diagnostics.ProcessStartInfo
    $taskStart.FileName=$taskPython; $taskStart.Arguments='"'+$taskScript+'" begin'
    $taskStart.UseShellExecute=$false; $taskStart.CreateNoWindow=$true
    $taskStart.RedirectStandardInput=$true; $taskStart.RedirectStandardOutput=$true; $taskStart.RedirectStandardError=$true
    $taskStart.EnvironmentVariables['MINITERM_SESSION_ID']=$taskSession
    $taskStart.EnvironmentVariables['MINITERM_NOTIFY_EXE']=$taskHelper
    $taskStart.EnvironmentVariables['MINITERM_ADAPTER_LOG']=$taskLog
    $taskProcess = [Diagnostics.Process]::Start($taskStart)
    try {
        $taskOutput=$taskProcess.StandardOutput.ReadToEndAsync(); $taskError=$taskProcess.StandardError.ReadToEndAsync()
        $taskProcess.StandardInput.Write('{"title":"sample","state":"running","tasks":[{"text":"sample task","state":"running"}]}')
        $taskProcess.StandardInput.Close()
        if (-not $taskProcess.WaitForExit(10000)) { $taskProcess.Kill(); throw 'Adapter compatibility test timed out.' }
        if ($taskProcess.ExitCode -ne 2 -or $taskOutput.Result.Length -ne 0) { throw 'Unsupported integration was accepted.' }
        $taskCalls = @(Get-Content -LiteralPath $taskLog -Encoding UTF8)
        if ($taskCalls.Count -ne 1 -or $taskCalls[0] -ne 'sessions') { throw 'Adapter sent a command that the legacy helper cannot support.' }
        $taskResults[$taskCase]=$true
    } finally { $taskProcess.Dispose() }
}
$taskResult = @{fixture=$taskFixture;checks=$taskResults;globalConfigChanged=$false;scope='Owned compiled protocol stubs; checks refusal before unknown progress commands. Native helper/IPC/GUI/Unicode are tested separately with verify-ux -ProgressSample.'}
[IO.File]::WriteAllText((Join-Path $taskFixture 'verification.json'),($taskResult | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
$taskResult | ConvertTo-Json -Depth 5
