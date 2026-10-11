param([Parameter(Mandatory=$true)][string]$ReleaseTag, [switch]$ValidateOnly)
$ErrorActionPreference = 'Stop'
if ($ReleaseTag -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+-alpha\.[1-9][0-9]*$') { throw 'Only explicit alpha tags can be published.' }
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskArtifacts = Join-Path $taskRoot 'artifacts'
$taskManifest = Get-Content -LiteralPath (Join-Path $taskArtifacts 'RELEASE_MANIFEST.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskBuild = Get-Content -LiteralPath (Join-Path $taskArtifacts 'BUILD_INFO.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskCompatibility = Get-Content -LiteralPath (Join-Path $taskArtifacts 'COMPATIBILITY.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskCommit = & git -C $taskRoot rev-parse HEAD
$taskWorkingDirty = [bool](& git -C $taskRoot status --porcelain --untracked-files=normal)
if ($taskWorkingDirty) { throw 'The current source working tree is dirty. Commit it and rebuild/repackage before publication.' }
if ($LASTEXITCODE -ne 0 -or $taskManifest.releaseTag -ne $ReleaseTag -or $taskBuild.releaseTag -ne $ReleaseTag -or $taskManifest.sourceCommit -ne $taskCommit -or $taskBuild.sourceCommit -ne $taskCommit -or $taskManifest.sourceDirty -or $taskBuild.sourceDirty) { throw 'Release provenance does not match a clean committed source.' }
if (-not $taskCompatibility.acceptedForAlpha -or $taskCompatibility.profile -ne 'default installed Git auto detection') { throw 'Default Git compatibility must be recorded and accepted for this alpha.' }
$taskLabel = $ReleaseTag.Substring(1)
$taskExpected = @('miniterm.exe',"MiniTerm-$taskLabel-windows-x64-setup.exe","MiniTerm-$taskLabel-windows-x64.zip",'BUILD_INFO.json','THIRD_PARTY_NOTICES.html','TESTER_GUIDE.md','RELEASE_NOTES.md','STATUS.md','COMPATIBILITY.json','SHA256SUMS')
if (@($taskManifest.files).Count -ne $taskExpected.Count -or @($taskManifest.files.name | Sort-Object -Unique).Count -ne $taskExpected.Count) { throw 'Unexpected release file list.' }
foreach ($taskFile in $taskManifest.files) {
    if ($taskFile.name -notin $taskExpected -or [IO.Path]::GetFileName($taskFile.name) -ne $taskFile.name) { throw 'Unexpected release file.' }
    $taskPath = Join-Path $taskArtifacts $taskFile.name
    if ((Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskFile.sha256 -or (Get-Item -LiteralPath $taskPath).Length -ne $taskFile.bytes) { throw "Release file checksum mismatch: $($taskFile.name)" }
}
if ($ValidateOnly) { Write-Output "Validated $ReleaseTag, source $taskCommit, $($taskExpected.Count) assets. No upload performed."; return }
if ($env:GITHUB_REPOSITORY -ne 'hojin-v/MiniTerm' -or $env:GITHUB_EVENT_NAME -ne 'push' -or $env:GITHUB_REF -ne ('refs/tags/' + $ReleaseTag) -or $env:GITHUB_SHA -ne $taskCommit -or -not $env:GITHUB_TOKEN) { throw 'Publication requires the matching repository tag workflow and its scoped token.' }
$taskHeaders = @{Authorization=('Bearer ' + $env:GITHUB_TOKEN);Accept='application/vnd.github+json';'X-GitHub-Api-Version'='2026-03-10';'User-Agent'='MiniTerm-alpha-publisher'}
$taskApi = 'https://api.github.com/repos/hojin-v/MiniTerm/releases'
$taskMarker = '<!-- miniterm-alpha-publisher -->'
$taskRelease = $null
try { $taskRelease = Invoke-RestMethod -Uri ($taskApi + '/tags/' + $ReleaseTag) -Headers $taskHeaders } catch { if ([int]$_.Exception.Response.StatusCode -ne 404) { throw } }
if ($taskRelease -and -not $taskRelease.draft) {
    foreach ($taskFile in $taskManifest.files) {
        $taskAsset = @($taskRelease.assets | Where-Object { $_.name -eq $taskFile.name })
        if ($taskAsset.Count -ne 1 -or $taskAsset[0].digest -ne ('sha256:' + $taskFile.sha256)) { throw 'The published release is immutable to this workflow. Use a new tag.' }
    }
    Write-Output $taskRelease.html_url
    return
}
if ($taskRelease -and ($taskRelease.body -notlike ('*' + $taskMarker + '*') -or $taskRelease.target_commitish -ne $taskCommit)) { throw 'An existing draft is not owned by this publisher/source. It was preserved.' }
if (-not $taskRelease) {
    $taskBody = @{tag_name=$ReleaseTag;target_commitish=$taskCommit;name=('MiniTerm ' + $ReleaseTag);body=([IO.File]::ReadAllText((Join-Path $taskArtifacts 'RELEASE_NOTES.md')) + "`n`n" + $taskMarker + "`nSource commit: " + $taskCommit);draft=$true;prerelease=$true;make_latest='false'} | ConvertTo-Json
    $taskRelease = Invoke-RestMethod -Method Post -Uri $taskApi -Headers $taskHeaders -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($taskBody))
}
$taskUploadBase = $taskRelease.upload_url.Split('{')[0]
foreach ($taskFile in $taskManifest.files) {
    $taskAsset = @($taskRelease.assets | Where-Object { $_.name -eq $taskFile.name })
    if ($taskAsset.Count -gt 1) { throw 'Duplicate draft asset names.' }
    if ($taskAsset.Count -eq 1 -and $taskAsset[0].digest -eq ('sha256:' + $taskFile.sha256)) { continue }
    if ($taskAsset.Count -eq 1) { Invoke-RestMethod -Method Delete -Uri ($taskApi + '/assets/' + $taskAsset[0].id) -Headers $taskHeaders | Out-Null }
    $taskUploaded = Invoke-RestMethod -Method Post -Uri ($taskUploadBase + '?name=' + [Uri]::EscapeDataString($taskFile.name)) -Headers $taskHeaders -ContentType 'application/octet-stream' -InFile (Join-Path $taskArtifacts $taskFile.name)
    if ($taskUploaded.state -ne 'uploaded' -or $taskUploaded.size -ne $taskFile.bytes -or ($taskUploaded.digest -and $taskUploaded.digest -ne ('sha256:' + $taskFile.sha256))) { throw "Upload verification failed: $($taskFile.name). Draft remains unpublished." }
}
$taskCompleteDraft = Invoke-RestMethod -Uri ($taskApi + '/' + $taskRelease.id) -Headers $taskHeaders
if (@($taskCompleteDraft.assets).Count -ne $taskExpected.Count) { throw 'Draft contains missing or unexpected assets. It remains unpublished.' }
foreach ($taskFile in $taskManifest.files) {
    $taskAsset = @($taskCompleteDraft.assets | Where-Object { $_.name -eq $taskFile.name })
    if ($taskAsset.Count -ne 1 -or $taskAsset[0].size -ne $taskFile.bytes -or $taskAsset[0].digest -ne ('sha256:' + $taskFile.sha256)) { throw 'Draft asset verification is incomplete. It remains unpublished.' }
}
$taskPublishBody = @{draft=$false;prerelease=$true;make_latest='false'} | ConvertTo-Json
$taskPublished = Invoke-RestMethod -Method Patch -Uri ($taskApi + '/' + $taskRelease.id) -Headers $taskHeaders -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($taskPublishBody))
if ($taskPublished.draft -or -not $taskPublished.prerelease -or @($taskPublished.assets).Count -ne $taskExpected.Count) { throw 'Final prerelease verification failed.' }
Write-Output $taskPublished.html_url
