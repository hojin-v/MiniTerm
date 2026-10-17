param([Parameter(Mandatory=$true)][string]$ApplicationDirectory)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskArtifacts = [IO.Path]::GetFullPath((Join-Path $taskRoot 'artifacts'))
$taskApplication = [IO.Path]::GetFullPath($ApplicationDirectory)
if (-not $taskApplication.StartsWith($taskArtifacts + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime verification must stay inside this workspace artifacts directory.' }
$taskRuntime = Join-Path $taskApplication 'runtime/git'
$taskManifest = Get-Content -LiteralPath (Join-Path $taskRuntime 'miniterm-runtime.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskRuntimeDll = Join-Path $taskRuntime 'usr/bin/msys-2.0.dll'
$taskRuntimeHash = (Get-FileHash -LiteralPath $taskRuntimeDll -Algorithm SHA256).Hash.ToLowerInvariant()
if ($taskRuntimeHash -ne $taskManifest.msysSha256) { throw 'Runtime manifest mismatch.' }
$taskInstalledDll = Join-Path $env:ProgramFiles 'Git/usr/bin/msys-2.0.dll'
$taskInstalledHash = if (Test-Path -LiteralPath $taskInstalledDll) { (Get-FileHash -LiteralPath $taskInstalledDll -Algorithm SHA256).Hash } else { $null }
$taskBuildLines = & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--no-run','--message-format=json')
$taskBuilds = @($taskBuildLines | ForEach-Object { if ($_ -like '{*') { ConvertFrom-Json -InputObject $_ } })
$taskLib = $taskBuilds | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.kind -contains 'lib' -and $_.executable } | Select-Object -Last 1
$taskPty = $taskBuilds | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'bash_conpty' -and $_.executable } | Select-Object -Last 1
if (-not $taskLib -or -not $taskPty) { throw 'Required test executables were not produced.' }
$taskSelection = Join-Path $taskApplication 'verify-bash-selection.exe'
$taskProduction = Join-Path $taskApplication 'verify-bash-conpty.exe'
Copy-Item -LiteralPath $taskLib.executable -Destination $taskSelection
Copy-Item -LiteralPath $taskPty.executable -Destination $taskProduction
$taskEvidence = Join-Path $taskApplication ('verification-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskEvidence | Out-Null
$taskResult = @{runtimeRoot=$taskRuntime;runtimeSha256=$taskRuntimeHash;manifestSha256=(Get-FileHash -LiteralPath (Join-Path $taskRuntime 'miniterm-runtime.json') -Algorithm SHA256).Hash.ToLowerInvariant();installedGitDll=$taskInstalledDll;installedGitSha256=$taskInstalledHash;installedGitControlObserved=[bool]$taskInstalledHash;startedUtc=[DateTime]::UtcNow.ToString('o');checks=@();passed=$false;scope='Real default ShellProfile with runtime next to the test executable; no explicit Bash override. Existing production ConPTY, prompt, Windows bridge and local-provider notification checks; excludes human input, GUI pixels, actual account/model responses, public installation and complete job-control transitions.'}
foreach ($taskStage in @(
    @{name='Selection';exe=$taskSelection;args='runtime::tests::prepared_bundle_is_selected_by_default_profile --ignored --nocapture --test-threads=1';count=1},
    @{name='Pty';exe=$taskProduction;args='--ignored --nocapture --test-threads=1 --skip wsl_ --skip git_windows_bridge --skip git_codex_notify --skip git_agent_bash';count=9},
    @{name='WindowsBridge';exe=$taskProduction;args='git_windows_bridge --ignored --nocapture --test-threads=1';count=1},
    @{name='CodexNotify';exe=$taskProduction;args='git_codex_notify --ignored --nocapture --test-threads=1';count=1},
    @{name='AgentBash';exe=$taskProduction;args='git_agent_bash --ignored --nocapture --test-threads=1';count=2}
)) {
    $taskStart = New-Object Diagnostics.ProcessStartInfo
    $taskStart.FileName=$taskStage.exe; $taskStart.Arguments=$taskStage.args; $taskStart.WorkingDirectory=$taskRoot
    $taskStart.UseShellExecute=$false; $taskStart.CreateNoWindow=$true
    $taskStart.RedirectStandardOutput=$true; $taskStart.RedirectStandardError=$true
    $taskStart.StandardOutputEncoding=[Text.UTF8Encoding]::new($false); $taskStart.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
    $taskStart.EnvironmentVariables.Remove('MINITERM_GIT_TEST_BASH')
    $taskStart.EnvironmentVariables['MINITERM_INSTANCE_ID']=[guid]::NewGuid().ToString()
    $taskStart.EnvironmentVariables['MINITERM_DATA_DIR']=Join-Path $taskEvidence $taskStage.name
    $taskStart.EnvironmentVariables['MINITERM_DIAGNOSTICS']='1'
    $taskProcess=[Diagnostics.Process]::Start($taskStart)
    Write-Output ('Started ' + $taskStage.name + ' with default runtime selection; PID ' + $taskProcess.Id)
    $taskStdout=$taskProcess.StandardOutput.ReadToEndAsync(); $taskStderr=$taskProcess.StandardError.ReadToEndAsync()
    try {
        if (-not $taskProcess.WaitForExit(600000)) { $taskProcess.Kill(); throw 'Runtime verification timed out.' }
        $taskProcess.Refresh(); $taskExit=$taskProcess.ExitCode
        $taskOutput=$taskStdout.Result + $taskStderr.Result
        [IO.File]::WriteAllText((Join-Path $taskEvidence ($taskStage.name + '.log')),$taskOutput,[Text.UTF8Encoding]::new($false))
        $taskSummary=[regex]::Matches($taskOutput,'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;')
        $taskPassed=$taskExit -eq 0 -and $taskSummary.Count -eq 1 -and $taskSummary[0].Groups[1].Value -eq 'ok' -and [int]$taskSummary[0].Groups[2].Value -eq $taskStage.count -and [int]$taskSummary[0].Groups[3].Value -eq 0
        $taskResult.checks += @{name=$taskStage.name;passed=$taskPassed;exitCode=$taskExit;expectedCount=$taskStage.count;summary=($taskSummary | ForEach-Object Value)}
        Write-Output ($taskResult.checks[-1] | ConvertTo-Json -Compress)
        if (-not $taskPassed) { break }
        if ((Get-FileHash -LiteralPath $taskRuntimeDll -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskRuntimeHash) { throw 'Runtime changed during verification.' }
    } finally { $taskProcess.Dispose() }
}
$taskResult.finishedUtc=[DateTime]::UtcNow.ToString('o')
$taskFinalInstalledHash=if (Test-Path -LiteralPath $taskInstalledDll) { (Get-FileHash -LiteralPath $taskInstalledDll -Algorithm SHA256).Hash } else { $null }
$taskResult.installedGitUnchanged=$taskInstalledHash -eq $taskFinalInstalledHash
$taskResult.passed=$taskResult.checks.Count -eq 5 -and @($taskResult.checks | Where-Object { -not $_.passed }).Count -eq 0 -and $taskResult.installedGitUnchanged
[IO.File]::WriteAllText((Join-Path $taskEvidence 'verification.json'),($taskResult | ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
Write-Output ('Evidence: ' + $taskEvidence)
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'bundled-runtime-verification.json'),($taskResult | ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
$taskCompatibility = @{recordedUtc=[DateTime]::UtcNow.ToString('o');profile='default bundled Git';gitVersion=$taskManifest.gitVersion;runtimeSha256=$taskRuntimeHash;allPassed=$taskResult.passed;acceptedForAlpha=$taskResult.passed;passed=if ($taskResult.passed) { 9 } else { 0 };failed=if ($taskResult.passed) { 0 } else { 1 };failedTests=@();checks=$taskResult.checks;scope=$taskResult.scope}
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'COMPATIBILITY.json'),($taskCompatibility | ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
if (Test-Path -LiteralPath (Join-Path $taskEvidence 'Pty.log')) { Copy-Item -LiteralPath (Join-Path $taskEvidence 'Pty.log') -Destination (Join-Path $taskArtifacts 'git-compatibility.log') }
if (-not $taskResult.passed) { throw 'Default bundled runtime verification failed; inspect the recorded logs.' }
