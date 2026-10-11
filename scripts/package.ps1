param([switch]$SkipBuild, [string]$ReleaseTag = '')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskArtifacts = Join-Path $taskRoot 'artifacts'
$taskVersionMatch = [regex]::Match((Get-Content (Join-Path $taskRoot 'Cargo.toml') -Raw -Encoding UTF8),'(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"')
if (-not $taskVersionMatch.Success) { throw 'Package version not found' }
$taskVersion = $taskVersionMatch.Groups[1].Value
$taskLabel = $taskVersion
$taskChannel = 'local-alpha'
if ($ReleaseTag) {
    if ($ReleaseTag -notmatch '^v([0-9]+\.[0-9]+\.[0-9]+)-alpha\.([1-9][0-9]*)$' -or $Matches[1] -ne $taskVersion) { throw 'Expected v<Cargo version>-alpha.<number>' }
    $taskLabel = $ReleaseTag.Substring(1)
    $taskChannel = 'tester-alpha'
}
$taskNotes = if ($ReleaseTag) { Join-Path $taskRoot ('docs/releases/' + $ReleaseTag + '.md') } else { Join-Path $taskRoot 'docs/releases/v0.1.0-alpha.1.md' }
if (-not (Test-Path -LiteralPath $taskNotes)) { throw 'Release notes must exist before packaging.' }
if (-not $SkipBuild) { & (Join-Path $PSScriptRoot 'build.ps1') }
$taskBuildPath = Join-Path $taskArtifacts 'BINARY_BUILD_INFO.json'
if (-not (Test-Path -LiteralPath $taskBuildPath)) { throw 'Run build.ps1 before packaging with -SkipBuild.' }
$taskBuild = Get-Content -LiteralPath $taskBuildPath -Raw -Encoding UTF8 | ConvertFrom-Json
$taskBinaryHash = (Get-FileHash -LiteralPath (Join-Path $taskArtifacts 'miniterm.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
if ($taskBuild.executableSha256 -ne $taskBinaryHash -or $taskBuild.sourceFingerprint -ne (& (Join-Path $PSScriptRoot 'source-fingerprint.ps1'))) { throw 'The executable does not match the current source. Run build.ps1.' }
& (Join-Path $PSScriptRoot 'licenses.ps1') | Out-Null
$taskNsis = & (Join-Path $PSScriptRoot 'bootstrap-nsis.ps1')
$taskCommit = & git -C $taskRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0) { throw 'Commit the source before preparing a release package.' }
$taskDirty = [bool](& git -C $taskRoot status --porcelain --untracked-files=normal)
$taskInfo = @{ version=$taskVersion; releaseVersion=$taskLabel; releaseTag=$ReleaseTag; target='x86_64-pc-windows-msvc'; rust='1.90.0'; builtUtc=$taskBuild.builtUtc; packagedUtc=[DateTime]::UtcNow.ToString('o'); sourceCommit=$taskCommit; sourceDirty=$taskDirty; sourceFingerprint=$taskBuild.sourceFingerprint; executableSha256=$taskBinaryHash; cargoLockSha256=(Get-FileHash (Join-Path $taskRoot 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant(); nsis='3.13'; signed=$false; channel=$taskChannel; msysCandidateBundled=$false; shell='Separately installed Git for Windows Bash'; knownIssues='STATUS.md' }
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'BUILD_INFO.json'),($taskInfo | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
Copy-Item -LiteralPath (Join-Path $taskRoot 'LICENSE') -Destination (Join-Path $taskArtifacts 'LICENSE')
Copy-Item -LiteralPath (Join-Path $taskRoot 'docs/PACKAGE_README.md') -Destination (Join-Path $taskArtifacts 'README.md')
foreach ($taskDocument in @('TESTER_GUIDE.md','STATUS.md')) { Copy-Item -LiteralPath (Join-Path $taskRoot ('docs/' + $taskDocument)) -Destination (Join-Path $taskArtifacts $taskDocument) }
Copy-Item -LiteralPath $taskNotes -Destination (Join-Path $taskArtifacts 'RELEASE_NOTES.md')
Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $taskNsis) 'COPYING') -Destination (Join-Path $taskArtifacts 'NSIS-LICENSE.txt')
if (-not (Test-Path -LiteralPath (Join-Path $taskArtifacts 'COMPATIBILITY.json'))) { [IO.File]::WriteAllText((Join-Path $taskArtifacts 'COMPATIBILITY.json'),'{"allPassed":false,"acceptedForAlpha":false,"scope":"Not run; this is not a verified release."}',[Text.UTF8Encoding]::new($false)) }
$taskComponents = @('miniterm.exe','LICENSE','README.md','THIRD_PARTY_NOTICES.html','BUILD_INFO.json','NSIS-LICENSE.txt','TESTER_GUIDE.md','RELEASE_NOTES.md','STATUS.md','COMPATIBILITY.json')
$taskComponentChecksums = foreach ($taskFile in $taskComponents) { $taskHash=(Get-FileHash -LiteralPath (Join-Path $taskArtifacts $taskFile) -Algorithm SHA256).Hash.ToLowerInvariant(); "$taskHash  $taskFile" }
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'COMPONENT_SHA256SUMS'),($taskComponentChecksums -join "`n") + "`n",[Text.UTF8Encoding]::new($false))
Push-Location -LiteralPath (Join-Path $taskRoot 'installer')
try { & $taskNsis /V2 "/DARTIFACTS=$taskArtifacts" "/DVERSION=$taskVersion" "/DRELEASE_LABEL=$taskLabel" miniterm.nsi; if ($LASTEXITCODE -ne 0) { throw 'NSIS packaging failed' } } finally { Pop-Location }
$taskStage = Join-Path $taskArtifacts ('package-' + [Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $taskStage | Out-Null
foreach ($taskFile in $taskComponents) { Copy-Item -LiteralPath (Join-Path $taskArtifacts $taskFile) -Destination (Join-Path $taskStage $taskFile) }
Copy-Item -LiteralPath (Join-Path $taskArtifacts 'COMPONENT_SHA256SUMS') -Destination (Join-Path $taskStage 'SHA256SUMS')
$taskZipName = "MiniTerm-$taskLabel-windows-x64.zip"
Compress-Archive -LiteralPath @((Get-ChildItem -LiteralPath $taskStage -File).FullName) -DestinationPath (Join-Path $taskArtifacts $taskZipName) -Force
$taskFiles = @('miniterm.exe',"MiniTerm-$taskLabel-windows-x64-setup.exe",$taskZipName,'BUILD_INFO.json','THIRD_PARTY_NOTICES.html','TESTER_GUIDE.md','RELEASE_NOTES.md','STATUS.md','COMPATIBILITY.json')
$taskChecksums = foreach ($taskFile in $taskFiles) { $taskHash=(Get-FileHash -LiteralPath (Join-Path $taskArtifacts $taskFile) -Algorithm SHA256).Hash.ToLowerInvariant(); "$taskHash  $taskFile" }
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'SHA256SUMS'),($taskChecksums -join "`n") + "`n",[Text.UTF8Encoding]::new($false))
$taskManifestFiles = foreach ($taskFile in ($taskFiles + @('SHA256SUMS'))) { @{name=$taskFile;sha256=(Get-FileHash -LiteralPath (Join-Path $taskArtifacts $taskFile) -Algorithm SHA256).Hash.ToLowerInvariant();bytes=(Get-Item -LiteralPath (Join-Path $taskArtifacts $taskFile)).Length} }
$taskManifest = @{releaseTag=$ReleaseTag;sourceCommit=$taskCommit;sourceDirty=$taskDirty;files=@($taskManifestFiles)}
[IO.File]::WriteAllText((Join-Path $taskArtifacts 'RELEASE_MANIFEST.json'),($taskManifest | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
Write-Output (Join-Path $taskArtifacts "MiniTerm-$taskLabel-windows-x64-setup.exe")
Write-Output (Join-Path $taskArtifacts $taskZipName)
