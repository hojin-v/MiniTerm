param([string]$ManifestDirectory = 'winget/manifests/h/hojin-v/MiniTerm/0.1.0-alpha.1')
$ErrorActionPreference = 'Stop'
trap {
    if ($env:GITHUB_ACTIONS -eq 'true') {
        $taskFailure = ($_ | Out-String).Trim().Replace('%','%25').Replace("`r",'%0D').Replace("`n",'%0A')
        Write-Output ('::error title=WinGet installation::' + $taskFailure)
    }
    throw
}
# This test installs/uninstalls and registers notifications. Never run it on the developer's desktop.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or -not $env:RUNNER_TEMP) { throw 'Installer test is restricted to an ephemeral GitHub-hosted Windows runner.' }
$taskRegistry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm'
if ((Test-Path $taskRegistry) -or (Get-Process miniterm -ErrorAction SilentlyContinue)) { throw 'A pre-existing MiniTerm installation/session must not be modified.' }
function Invoke-TestWinGet([string[]]$Arguments) {
    $taskStart = [Diagnostics.ProcessStartInfo]::new()
    $taskStart.FileName = (Get-Command winget).Source
    $taskStart.UseShellExecute = $false
    $taskStart.CreateNoWindow = $true
    $taskStart.RedirectStandardOutput = $true
    $taskStart.RedirectStandardError = $true
    foreach ($taskArgument in $Arguments) { $taskStart.ArgumentList.Add($taskArgument) }
    $taskChild = [Diagnostics.Process]::Start($taskStart)
    try {
        $taskOutput = $taskChild.StandardOutput.ReadToEndAsync()
        $taskError = $taskChild.StandardError.ReadToEndAsync()
        $taskCompleted = $taskChild.WaitForExit(180000)
        if (-not $taskCompleted) { $taskChild.Kill($true); $taskChild.WaitForExit() }
        $taskText = $taskOutput.GetAwaiter().GetResult() + $taskError.GetAwaiter().GetResult()
        Write-Output $taskText
        if (-not $taskCompleted) { throw ('WinGet ' + $Arguments[0] + ' timed out after 180 seconds: ' + $taskText) }
        if ($taskChild.ExitCode -ne 0) { throw ('WinGet ' + $Arguments[0] + ' failed, exit ' + $taskChild.ExitCode + ': ' + $taskText) }
    } finally { $taskChild.Dispose() }
}
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskDirectory = if ([IO.Path]::IsPathRooted($ManifestDirectory)) { $ManifestDirectory } else { Join-Path $taskRoot $ManifestDirectory }
$taskId = [guid]::NewGuid().ToString()
$taskInstall = Join-Path $env:RUNNER_TEMP ('MiniTerm-winget-' + $taskId)
$taskData = Join-Path $env:RUNNER_TEMP ('MiniTerm-data-' + $taskId)
$env:MINITERM_INSTANCE_ID = $taskId
$env:MINITERM_DIAGNOSTICS = '1'
$env:MINITERM_DATA_DIR = $taskData
$env:CODEX_HOME = Join-Path $taskData 'codex'
New-Item -ItemType Directory -Path $taskData | Out-Null
$taskSettings = Join-Path $taskData 'settings.json'
[IO.File]::WriteAllText($taskSettings, '{"schemaVersion":1,"theme":"light"}', [Text.UTF8Encoding]::new($false))
$taskSettingsHash = (Get-FileHash -LiteralPath $taskSettings -Algorithm SHA256).Hash
& winget settings --enable LocalManifestFiles --disable-interactivity
if ($LASTEXITCODE -ne 0) { throw 'Cannot enable local manifest testing in the ephemeral runner.' }
Invoke-TestWinGet @('install','--manifest',$taskDirectory,'--silent','--location',$taskInstall,'--dependency-source','winget','--disable-interactivity','--accept-source-agreements','--accept-package-agreements')
$taskEntry = Get-ItemProperty -LiteralPath $taskRegistry
$taskVersion = [regex]::Match([IO.File]::ReadAllText((Join-Path $taskDirectory 'hojin-v.MiniTerm.yaml')), '(?m)^PackageVersion: (\S+)\s*$').Groups[1].Value
if ($taskEntry.DisplayName -ne 'MiniTerm' -or $taskEntry.Publisher -ne 'MiniTerm contributors' -or $taskEntry.DisplayVersion -ne $taskVersion -or $taskEntry.InstallLocation -ine $taskInstall) { throw 'Installed Apps & Features metadata does not match the manifest.' }
$taskExe = Join-Path $taskInstall 'miniterm.exe'
if (-not (Test-Path -LiteralPath $taskExe)) { throw 'The installed executable is missing.' }
$taskStart = [Diagnostics.ProcessStartInfo]::new()
$taskStart.FileName = $taskExe
$taskStart.Arguments = '--check-update-safe'
$taskStart.UseShellExecute = $false
$taskStart.CreateNoWindow = $true
$taskProcess = [Diagnostics.Process]::Start($taskStart)
try {
    if (-not $taskProcess.WaitForExit(10000) -or $taskProcess.ExitCode -ne 0) { throw 'The installed application cannot load or reports an unexpected live session.' }
} finally { $taskProcess.Dispose() }
Invoke-TestWinGet @('uninstall','--manifest',$taskDirectory,'--silent','--disable-interactivity','--accept-source-agreements')
if ((Test-Path $taskRegistry) -or (Test-Path -LiteralPath $taskExe)) { throw 'WinGet removal left installed app files or the Apps & Features entry.' }
if ((Get-FileHash -LiteralPath $taskSettings -Algorithm SHA256).Hash -ne $taskSettingsHash) { throw 'Install/removal changed saved user settings.' }
$taskManifestHashes = Get-ChildItem -LiteralPath $taskDirectory -File -Filter '*.yaml' | Sort-Object Name | ForEach-Object { $_.Name + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
$taskHasher = [Security.Cryptography.SHA256]::Create()
try { $taskManifestHash = [BitConverter]::ToString($taskHasher.ComputeHash([Text.Encoding]::UTF8.GetBytes(($taskManifestHashes -join "`n") + "`n"))).Replace('-', '').ToLowerInvariant() } finally { $taskHasher.Dispose() }
$taskResult = @{ packageIdentifier='hojin-v.MiniTerm'; packageVersion=$taskEntry.DisplayVersion; manifestSha256=$taskManifestHash; sourceCommit=$env:GITHUB_SHA; installed=$true; uninstalled=$true; settingsPreserved=$true; executableLoaded=$true; scope='WinGet local-manifest install with package dependencies and removal on an ephemeral Windows runner. No physical GUI, IME, account or model validation.' }
New-Item -ItemType Directory -Path (Join-Path $taskRoot 'artifacts') -Force | Out-Null
[IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/winget-install-verification.json'), ($taskResult | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
Write-Output ('::notice title=WinGet installation verified::' + ($taskResult | ConvertTo-Json -Compress))
$taskResult | ConvertTo-Json
