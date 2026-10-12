param(
    [Parameter(Mandatory=$true)][string]$PatchedDll,
    [string]$ApplicationDirectory = ''
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskArtifacts = [IO.Path]::GetFullPath((Join-Path $taskRoot 'artifacts'))
$taskLock = Get-Content -LiteralPath (Join-Path $taskRoot 'runtime/BASH_RUNTIME.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($taskLock.schemaVersion -ne 1) { throw 'Unsupported runtime preparation schema.' }
$taskDll = (Get-Item -LiteralPath $PatchedDll).FullName
if ((Get-FileHash -LiteralPath $taskDll -Algorithm SHA256).Hash -ine $taskLock.patchedMsysSha256) { throw 'Patched MSYS DLL does not match the validated candidate.' }
$taskPatch = Join-Path $taskRoot 'docs/patches/msys-resize-signal-after-discard.patch'
if ((Get-FileHash -LiteralPath $taskPatch -Algorithm SHA256).Hash -ine $taskLock.patchSha256) { throw 'The recorded MSYS patch changed.' }
if (-not $ApplicationDirectory) { $ApplicationDirectory = Join-Path $taskArtifacts ('managed-bash-' + [guid]::NewGuid().ToString('N')) }
$taskApplication = [IO.Path]::GetFullPath($ApplicationDirectory)
if (-not $taskApplication.StartsWith($taskArtifacts + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime preparation must stay inside this workspace artifacts directory.' }
if (Test-Path -LiteralPath $taskApplication) { throw 'Use a fresh application directory; running or previous runtimes are never replaced.' }
$taskTools = Join-Path $taskRoot '.tools/7zip'
$taskCache = Join-Path $taskArtifacts 'runtime-preparation'
New-Item -ItemType Directory -Force -Path $taskTools,$taskCache | Out-Null
$taskSevenZip = Join-Path $taskTools '7zr.exe'
$taskArchive = Join-Path $taskCache 'PortableGit-2.50.1-64-bit.7z.exe'
foreach ($taskDownload in @(
    @{path=$taskSevenZip;uri=$taskLock.sevenZipUrl;sha=$taskLock.sevenZipSha256},
    @{path=$taskArchive;uri=$taskLock.portableGitUrl;sha=$taskLock.portableGitSha256}
)) {
    if (-not (Test-Path -LiteralPath $taskDownload.path)) { Invoke-WebRequest -Uri $taskDownload.uri -OutFile $taskDownload.path -UseBasicParsing }
    if ((Get-FileHash -LiteralPath $taskDownload.path -Algorithm SHA256).Hash -ine $taskDownload.sha) { throw 'Upstream download checksum mismatch.' }
}
$taskRuntime = Join-Path $taskApplication 'runtime/git'
New-Item -ItemType Directory -Path $taskRuntime -Force | Out-Null
$taskLog = Join-Path $taskApplication 'runtime-extraction.log'
# Extraction only: do not run the SFX installer or its post-install scripts.
& $taskSevenZip x $taskArchive ('-o' + $taskRuntime) -y *> $taskLog
if ($LASTEXITCODE -ne 0) { throw ('PortableGit extraction failed; see ' + $taskLog) }
$taskBash = Join-Path $taskRuntime 'usr/bin/bash.exe'
$taskRuntimeDll = Join-Path $taskRuntime 'usr/bin/msys-2.0.dll'
if ((Get-FileHash -LiteralPath $taskBash -Algorithm SHA256).Hash -ine $taskLock.bashSha256 -or (Get-FileHash -LiteralPath $taskRuntimeDll -Algorithm SHA256).Hash -ine $taskLock.stockMsysSha256) { throw 'The official Bash and MSYS baseline do not match the validated source.' }
foreach ($taskRequired in @('etc/profile','cmd/git.exe','usr/share/licenses')) {
    if (-not (Test-Path -LiteralPath (Join-Path $taskRuntime $taskRequired))) { throw ('PortableGit component missing: ' + $taskRequired) }
}
Copy-Item -LiteralPath $taskDll -Destination $taskRuntimeDll
Copy-Item -LiteralPath $taskPatch -Destination (Join-Path $taskRuntime 'miniterm-msys.patch')
$taskLicenseRoot = Join-Path $taskRoot 'licenses/msys2-runtime'
$taskLicenseSources = Get-Content -LiteralPath (Join-Path $taskLicenseRoot 'SOURCES.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($taskLicenseSources.sourceCommit -ne $taskLock.msysSourceCommit) { throw 'MSYS license source does not match the candidate source.' }
$taskRuntimeLicenses = Join-Path $taskRuntime 'usr/share/licenses/miniterm-msys'
New-Item -ItemType Directory -Path $taskRuntimeLicenses | Out-Null
foreach ($taskLicense in $taskLicenseSources.files) {
    if ($taskLicense.path -notin @('CYGWIN_LICENSE','COPYING.LIB','COPYING')) { throw 'Unexpected MSYS license file.' }
    $taskLicensePath = Join-Path $taskLicenseRoot $taskLicense.path
    if ((Get-FileHash -LiteralPath $taskLicensePath -Algorithm SHA256).Hash -ine $taskLicense.sha256) { throw 'MSYS license checksum mismatch.' }
    Copy-Item -LiteralPath $taskLicensePath -Destination (Join-Path $taskRuntimeLicenses $taskLicense.path)
}
Copy-Item -LiteralPath (Join-Path $taskLicenseRoot 'SOURCES.json') -Destination (Join-Path $taskRuntimeLicenses 'SOURCES.json')
$taskManifest = @{
    schemaVersion=1;architecture='x64';gitVersion=$taskLock.gitVersion
    bashSha256=$taskLock.bashSha256;msysSha256=$taskLock.patchedMsysSha256
    portableGitSha256=$taskLock.portableGitSha256;msysSourceCommit=$taskLock.msysSourceCommit
    patchSha256=$taskLock.patchSha256;status=$taskLock.status
}
[IO.File]::WriteAllText((Join-Path $taskRuntime 'miniterm-runtime.json'),($taskManifest | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $taskApplication 'runtime-preparation.json'),(@{applicationDirectory=$taskApplication;runtimeRoot=$taskRuntime;bash=$taskBash;runtimeSha256=$taskLock.patchedMsysSha256;source='Hash-verified official PortableGit plus the recorded MSYS v3 candidate; no installed Git files or user settings copied.';publicPackagingApproved=$false} | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
Write-Output $taskApplication
