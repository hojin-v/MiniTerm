param([switch]$AllowKnownFailures)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskArtifacts = Join-Path $taskRoot 'artifacts'
New-Item -ItemType Directory -Path $taskArtifacts -Force | Out-Null
$taskFixture = Join-Path $taskArtifacts ('compatibility-' + [Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $taskFixture | Out-Null
$taskStart = New-Object Diagnostics.ProcessStartInfo
$taskStart.FileName = (Get-Command powershell.exe).Source
$taskStart.Arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + (Join-Path $PSScriptRoot 'test.ps1') + '" -Pty'
$taskStart.UseShellExecute = $false
$taskStart.CreateNoWindow = $true
$taskStart.RedirectStandardOutput = $true
$taskStart.RedirectStandardError = $true
$taskProcess = [Diagnostics.Process]::Start($taskStart)
$taskStdout = $taskProcess.StandardOutput.ReadToEndAsync()
$taskStderr = $taskProcess.StandardError.ReadToEndAsync()
try {
    if (-not $taskProcess.WaitForExit(600000)) { $taskProcess.Kill(); throw 'Git compatibility tests timed out.' }
    $taskProcess.Refresh()
    $taskExit = $taskProcess.ExitCode
    [IO.File]::WriteAllText((Join-Path $taskFixture 'stdout.log'),$taskStdout.Result,[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $taskFixture 'stderr.log'),$taskStderr.Result,[Text.UTF8Encoding]::new($false))
} finally { $taskProcess.Dispose() }
$taskOutput = [IO.File]::ReadAllText((Join-Path $taskFixture 'stdout.log')) + [IO.File]::ReadAllText((Join-Path $taskFixture 'stderr.log'))
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'git-compatibility.log'),$taskOutput,[Text.UTF8Encoding]::new($false))
$taskSummary = [regex]::Match($taskOutput,'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;')
$taskPassed = if ($taskSummary.Success) { [int]$taskSummary.Groups[1].Value } else { 0 }
$taskFailed = if ($taskSummary.Success) { [int]$taskSummary.Groups[2].Value } else { 0 }
$taskNames = @([regex]::Matches($taskOutput,'(?m)^    ([a-z][a-z0-9_]+)\r?$') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
$taskKnown = @('bash_managed_prompt_resize_preserves_typeahead','bash_resize_immediate_typeahead_preserves_commands','git_session_input_after_resize_preserves_commands_and_ansi_styles','git_session_vertical_resize_preserves_input_and_styles')
$taskUnknown = @($taskNames | Where-Object { $_ -notin $taskKnown })
$taskAllPassed = $taskExit -eq 0 -and $taskPassed -eq 8 -and $taskFailed -eq 0
$taskKnownOnly = $taskExit -ne 0 -and $taskSummary.Success -and ($taskPassed + $taskFailed) -eq 8 -and $taskFailed -gt 0 -and $taskNames.Count -eq $taskFailed -and $taskUnknown.Count -eq 0
$taskResult = @{recordedUtc=[DateTime]::UtcNow.ToString('o'); hostGitVersion=(& git --version); profile= $(if ($env:MINITERM_GIT_TEST_BASH) {'explicit test override'} else {'default installed Git auto detection'}); exitCode=$taskExit; passed=$taskPassed; failed=$taskFailed; failedTests=$taskNames; allPassed=$taskAllPassed; knownFailuresOnly=$taskKnownOnly; acceptedForAlpha=($taskAllPassed -or ($AllowKnownFailures -and $taskKnownOnly)); scope='Existing eight production Session/prompt/ConPTY tests. Fixed test commands; excludes GUI latency, real model/account responses and physical input. Known resize failures remain failures.'}
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'COMPATIBILITY.json'),($taskResult | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
$taskResult | ConvertTo-Json -Depth 5
if ($env:GITHUB_ACTIONS -eq 'true') {
    $taskSeverity = if (-not $taskResult.acceptedForAlpha) { 'error' } elseif (-not $taskAllPassed) { 'warning' } else { 'notice' }
    $taskAnnotation = ($taskResult | ConvertTo-Json -Depth 5 -Compress).Replace('%','%25').Replace("`r",'%0D').Replace("`n",'%0A')
    Write-Output ("::$taskSeverity title=Default Git compatibility::$taskAnnotation")
    if (-not $taskResult.acceptedForAlpha) {
        $taskDetails = @([regex]::Matches($taskOutput,"(?s)thread '[^']+' panicked at[^\n]+\n.{0,900}") | Select-Object -First 8 | ForEach-Object { $_.Value }) -join "`n"
        if (-not $taskDetails) { $taskDetails = $taskOutput.Substring([Math]::Max(0,$taskOutput.Length - 1200)) }
        $taskDetails = $taskDetails.Replace('%','%25').Replace("`r",'%0D').Replace("`n",'%0A')
        Write-Output ("::error title=Git fixture diagnostics::$taskDetails")
    }
}
if (-not $taskResult.acceptedForAlpha) { throw 'Git compatibility gate failed. Inspect artifacts/git-compatibility.log and COMPATIBILITY.json.' }
