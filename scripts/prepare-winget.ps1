param([Parameter(Mandatory=$true)][string]$ReleaseTag)
$ErrorActionPreference = 'Stop'
if ($ReleaseTag -notmatch '^v([0-9]+\.[0-9]+\.[0-9]+-alpha\.[1-9][0-9]*)$') { throw 'Expected a published v<version>-alpha.<number> tag.' }
$taskVersion = $Matches[1]
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskHeaders = @{Accept='application/vnd.github+json';'User-Agent'='MiniTerm-winget-preparation'}
$taskRelease = Invoke-RestMethod -Uri ('https://api.github.com/repos/hojin-v/MiniTerm/releases/tags/' + $ReleaseTag) -Headers $taskHeaders
if ($taskRelease.draft -or $taskRelease.tag_name -ne $ReleaseTag) { throw 'The requested release is not publicly published.' }
$taskName = "MiniTerm-$taskVersion-windows-x64-setup.exe"
$taskInstaller = @($taskRelease.assets | Where-Object { $_.name -eq $taskName })
if ($taskInstaller.Count -ne 1 -or $taskInstaller[0].state -ne 'uploaded' -or $taskInstaller[0].digest -notmatch '^sha256:([a-f0-9]{64})$') { throw 'Expected one completely uploaded installer with a GitHub SHA256 digest.' }
$taskHash = $Matches[1].ToUpperInvariant()
$taskExpectedUrl = "https://github.com/hojin-v/MiniTerm/releases/download/$ReleaseTag/$taskName"
if ($taskInstaller[0].browser_download_url -ne $taskExpectedUrl) { throw 'Unexpected installer URL.' }
$taskTemplateVersion = '0.1.0-alpha.3'
$taskTemplate = Join-Path $taskRoot "winget/manifests/h/Hnode/MiniTerm/$taskTemplateVersion"
$taskOutput = Join-Path $taskRoot "artifacts/winget/manifests/h/Hnode/MiniTerm/$taskVersion"
New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
foreach ($taskFile in Get-ChildItem -LiteralPath $taskTemplate -Filter '*.yaml' -File) {
    $taskText = [IO.File]::ReadAllText($taskFile.FullName).Replace($taskTemplateVersion, $taskVersion)
    if ($taskFile.Name -eq 'Hnode.MiniTerm.installer.yaml') { $taskText = [regex]::Replace($taskText, '(?m)^(\s*InstallerSha256: )[^\r\n]+', ('${1}' + $taskHash)) }
    [IO.File]::WriteAllText((Join-Path $taskOutput $taskFile.Name), $taskText, [Text.UTF8Encoding]::new($false))
}
& (Join-Path $PSScriptRoot 'verify-winget.ps1') -ManifestDirectory $taskOutput -VerifyInstaller
Write-Output $taskOutput
