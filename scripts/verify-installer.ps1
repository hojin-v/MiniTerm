param([string]$ManifestDirectory = 'winget/manifests/h/Hnode/MiniTerm/0.1.0-alpha.6', [string]$InstallerPath = '')
$ErrorActionPreference = 'Stop'
trap {
    if ($env:GITHUB_ACTIONS -eq 'true') {
        $taskFailure = ($_ | Out-String).Trim().Replace('%','%25').Replace("`r",'%0D').Replace("`n",'%0A')
        Write-Output ('::error title=Native installer verification::' + $taskFailure)
    }
    throw
}
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or -not $env:RUNNER_TEMP) { throw 'Installer verification is restricted to an ephemeral GitHub-hosted Windows runner.' }
$taskRegistry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm'
if ((Test-Path $taskRegistry) -or (Get-Process miniterm -ErrorAction SilentlyContinue)) { throw 'A pre-existing MiniTerm installation/session must not be modified.' }
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskDirectory = if ([IO.Path]::IsPathRooted($ManifestDirectory)) { $ManifestDirectory } else { Join-Path $taskRoot $ManifestDirectory }
if ($InstallerPath) {
    $taskLocalInstaller = (Get-Item -LiteralPath $InstallerPath).FullName
    $taskRelease = Get-Content -LiteralPath (Join-Path $taskRoot 'artifacts/RELEASE_MANIFEST.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $taskEntry = @($taskRelease.files | Where-Object { $_.name -eq [IO.Path]::GetFileName($taskLocalInstaller) })
    if ($taskEntry.Count -ne 1 -or $taskRelease.sourceCommit -ne (& git -C $taskRoot rev-parse HEAD) -or $taskRelease.sourceDirty) { throw 'Local installer must match the clean release manifest.' }
    $taskVersion = $taskRelease.releaseTag.TrimStart('v')
    $taskExpectedHash = $taskEntry[0].sha256
} else {
    $taskManifest = [IO.File]::ReadAllText((Join-Path $taskDirectory 'Hnode.MiniTerm.installer.yaml'))
    $taskVersion = [regex]::Match($taskManifest, '(?m)^PackageVersion: (\S+)\s*$').Groups[1].Value
    $taskExpectedHash = [regex]::Match($taskManifest, '(?m)^\s*InstallerSha256: ([A-Fa-f0-9]{64})\s*$').Groups[1].Value
}
if ($taskVersion -notmatch '^[0-9]+\.[0-9]+\.[0-9]+-alpha\.[1-9][0-9]*$') { throw 'Expected an explicit alpha version.' }
$taskId = [guid]::NewGuid().ToString()
$taskInstaller = Join-Path $env:RUNNER_TEMP ('MiniTerm-verified-' + $taskId + '.exe')
$taskInstall = Join-Path $env:RUNNER_TEMP ('MiniTerm-native-' + $taskId)
$taskData = Join-Path $env:LOCALAPPDATA 'MiniTerm'
if (Test-Path -LiteralPath $taskData) { throw 'Pre-existing user settings must not be modified.' }
$env:MINITERM_INSTANCE_ID = $taskId
$env:MINITERM_DIAGNOSTICS = '1'
$env:MINITERM_DATA_DIR = $taskData
$env:CODEX_HOME = Join-Path $env:RUNNER_TEMP ('MiniTerm-codex-' + $taskId)
if ($InstallerPath) {
    Copy-Item -LiteralPath $taskLocalInstaller -Destination $taskInstaller
} else {
    Invoke-WebRequest -Uri "https://github.com/hojin-v/MiniTerm/releases/download/v$taskVersion/MiniTerm-$taskVersion-windows-x64-setup.exe" -OutFile $taskInstaller
}
if ((Get-FileHash -LiteralPath $taskInstaller -Algorithm SHA256).Hash -ine $taskExpectedHash) { throw 'Publisher installer hash mismatch.' }
New-Item -ItemType Directory -Path $taskData | Out-Null
$taskSettings = Join-Path $taskData 'settings.json'
[IO.File]::WriteAllText($taskSettings, '{"schemaVersion":1,"theme":"light"}', [Text.UTF8Encoding]::new($false))
$taskSettingsHash = (Get-FileHash -LiteralPath $taskSettings -Algorithm SHA256).Hash
function Invoke-NativeInstaller([string]$Executable, [string[]]$Arguments) {
    $taskStart = [Diagnostics.ProcessStartInfo]::new()
    $taskStart.FileName = $Executable
    $taskStart.UseShellExecute = $false
    $taskStart.CreateNoWindow = $true
    foreach ($taskArgument in $Arguments) { $taskStart.ArgumentList.Add($taskArgument) }
    $taskChild = [Diagnostics.Process]::Start($taskStart)
    try {
        if (-not $taskChild.WaitForExit(45000)) {
            $taskDetails = ' Registry exists: ' + (Test-Path $taskRegistry) + ' App copied: ' + (Test-Path -LiteralPath (Join-Path $taskInstall 'miniterm.exe')) + ' Start menu folder exists: ' + (Test-Path -LiteralPath (Join-Path $env:APPDATA 'Microsoft/Windows/Start Menu/Programs'))
            $taskChild.Kill($true); $taskChild.WaitForExit()
            throw ('Native installer did not finish unattended.' + $taskDetails)
        }
        if ($taskChild.ExitCode -ne 0) { throw ('Native installer returned ' + $taskChild.ExitCode) }
    } finally { $taskChild.Dispose() }
}
Invoke-NativeInstaller $taskInstaller @('/S',('/D=' + $taskInstall))
$taskEntry = Get-ItemProperty -LiteralPath $taskRegistry
if ($taskEntry.DisplayName -ne 'MiniTerm' -or $taskEntry.Publisher -ne 'Hnode' -or $taskEntry.DisplayVersion -ne $taskVersion -or $taskEntry.InstallLocation -ine $taskInstall) { throw 'Installed Apps & Features metadata is incorrect.' }
if ($taskEntry.QuietUninstallString -ne ('"' + (Join-Path $taskInstall 'Uninstall.exe') + '" /S')) { throw 'The installer must register the NSIS silent removal command used by WinGet.' }
Invoke-NativeInstaller (Join-Path $taskInstall 'miniterm.exe') @('--check-update-safe')
Invoke-NativeInstaller (Join-Path $taskInstall 'Uninstall.exe') @('/S')
# NSIS copies the uninstaller to a temporary child before deleting its original executable.
# Wait for the observable removal to finish rather than treating the launcher's exit as completion.
$taskRemovalDeadline = [DateTime]::UtcNow.AddSeconds(15)
while (((Test-Path $taskRegistry) -or (Test-Path -LiteralPath (Join-Path $taskInstall 'miniterm.exe'))) -and [DateTime]::UtcNow -lt $taskRemovalDeadline) { Start-Sleep -Milliseconds 100 }
if ((Test-Path $taskRegistry) -or (Test-Path -LiteralPath (Join-Path $taskInstall 'miniterm.exe')) -or (Get-FileHash -LiteralPath $taskSettings -Algorithm SHA256).Hash -ne $taskSettingsHash) { throw ('Removal or saved-settings preservation failed. Registry exists: ' + (Test-Path $taskRegistry) + '; app exists: ' + (Test-Path -LiteralPath (Join-Path $taskInstall 'miniterm.exe')) + '; settings hash matches: ' + ((Get-FileHash -LiteralPath $taskSettings -Algorithm SHA256).Hash -eq $taskSettingsHash)) }
$taskManifestHashes = if ($InstallerPath) {
    'RELEASE_MANIFEST.json:' + (Get-FileHash -LiteralPath (Join-Path $taskRoot 'artifacts/RELEASE_MANIFEST.json') -Algorithm SHA256).Hash.ToLowerInvariant()
} else {
    Get-ChildItem -LiteralPath $taskDirectory -File -Filter '*.yaml' | Sort-Object Name | ForEach-Object { $_.Name + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
}
$taskHasher = [Security.Cryptography.SHA256]::Create()
try { $taskManifestHash = [BitConverter]::ToString($taskHasher.ComputeHash([Text.Encoding]::UTF8.GetBytes(($taskManifestHashes -join "`n") + "`n"))).Replace('-', '').ToLowerInvariant() } finally { $taskHasher.Dispose() }
$taskResult = @{packageIdentifier='Hnode.MiniTerm';packageVersion=$taskVersion;manifestSha256=$taskManifestHash;installerSha256=$taskExpectedHash.ToLowerInvariant();installed=$true;uninstalled=$true;settingsPreserved=$true;executableLoaded=$true;sourceCommit=$env:GITHUB_SHA;scope='Independent /S functional test of the hash-verified publisher NSIS installer on an ephemeral Windows runner. Does not prove WinGet catalog installation or SmartScreen reputation.'}
New-Item -ItemType Directory -Path (Join-Path $taskRoot 'artifacts') -Force | Out-Null
[IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/native-installer-verification.json'), ($taskResult | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
Write-Output ('::notice title=Native installer verified::' + ($taskResult | ConvertTo-Json -Compress))
