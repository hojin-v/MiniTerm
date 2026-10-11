param(
    [string]$ManifestDirectory = 'winget/manifests/h/hojin-v/MiniTerm/0.1.0-alpha.1',
    [switch]$VerifyInstaller
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskDirectory = if ([IO.Path]::IsPathRooted($ManifestDirectory)) { [IO.Path]::GetFullPath($ManifestDirectory) } else { [IO.Path]::GetFullPath((Join-Path $taskRoot $ManifestDirectory)) }
& winget validate --manifest $taskDirectory --disable-interactivity
if ($LASTEXITCODE -ne 0) { throw 'WinGet manifest validation failed.' }
$taskManifest = Get-Content -LiteralPath (Join-Path $taskDirectory 'hojin-v.MiniTerm.installer.yaml') -Raw -Encoding UTF8
function Read-Field([string]$Name) {
    $taskMatches = [regex]::Matches($taskManifest, ('(?m)^\s*' + [regex]::Escape($Name) + ':\s*(\S+)\s*$'))
    if ($taskMatches.Count -ne 1) { throw "Expected exactly one $Name." }
    return $taskMatches[0].Groups[1].Value
}
$taskVersion = Read-Field 'PackageVersion'
if ($taskVersion -notmatch '^[0-9]+\.[0-9]+\.[0-9]+-alpha\.[1-9][0-9]*$') { throw 'Expected an explicit tester-alpha version.' }
$taskUrl = Read-Field 'InstallerUrl'
$taskHash = Read-Field 'InstallerSha256'
if ($taskUrl -ne "https://github.com/hojin-v/MiniTerm/releases/download/v$taskVersion/MiniTerm-$taskVersion-windows-x64-setup.exe" -or $taskHash -notmatch '^[A-Fa-f0-9]{64}$') { throw 'Expected a versioned MiniTerm publisher installer and SHA256.' }
if ($VerifyInstaller) {
    $taskDownload = Join-Path $taskRoot ('artifacts/winget-download-' + [guid]::NewGuid().ToString() + '.exe')
    New-Item -ItemType Directory -Path (Split-Path -Parent $taskDownload) -Force | Out-Null
    Invoke-WebRequest -UseBasicParsing -Uri $taskUrl -OutFile $taskDownload
    if ((Get-FileHash -LiteralPath $taskDownload -Algorithm SHA256).Hash -ine $taskHash) { throw 'Published WinGet installer checksum mismatch.' }
    Write-Output "Verified publisher installer SHA256: $taskHash"
}
Write-Output "Validated hojin-v.MiniTerm $taskVersion. No installation or WinGet settings changes performed."
