param(
    [string]$ManifestDirectory = 'winget/manifests/h/hojin-v/MiniTerm/0.1.0-alpha.1',
    [string]$GhPath = '',
    [switch]$ValidateOnly
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskDirectory = if ([IO.Path]::IsPathRooted($ManifestDirectory)) { $ManifestDirectory } else { Join-Path $taskRoot $ManifestDirectory }
& (Join-Path $PSScriptRoot 'verify-winget.ps1') -ManifestDirectory $taskDirectory -VerifyInstaller
$taskFiles = @(Get-ChildItem -LiteralPath $taskDirectory -File)
$taskExpected = @('hojin-v.MiniTerm.yaml','hojin-v.MiniTerm.installer.yaml','hojin-v.MiniTerm.locale.en-US.yaml','hojin-v.MiniTerm.locale.ko-KR.yaml')
if ($taskFiles.Count -ne 4 -or @($taskFiles | Where-Object { $_.Name -notin $taskExpected }).Count) { throw 'Only the four MiniTerm manifests may be submitted.' }
$taskVersion = [regex]::Match([IO.File]::ReadAllText((Join-Path $taskDirectory 'hojin-v.MiniTerm.yaml')), '(?m)^PackageVersion: (\S+)\s*$').Groups[1].Value
if ($taskVersion -notmatch '^[0-9]+\.[0-9]+\.[0-9]+-alpha\.[1-9][0-9]*$') { throw 'Invalid package version.' }
$taskTitle = "New package: hojin-v.MiniTerm version $taskVersion"
$taskBody = [IO.File]::ReadAllText((Join-Path $taskRoot 'docs/WINGET_PR.md')).Replace('0.1.0-alpha.1', $taskVersion)
$taskArtifacts = Join-Path $taskRoot 'artifacts/winget-preparation'
New-Item -ItemType Directory -Path $taskArtifacts -Force | Out-Null
$taskBodyPath = Join-Path $taskArtifacts 'pull-request.md'
[IO.File]::WriteAllText($taskBodyPath, $taskBody, [Text.UTF8Encoding]::new($false))
if ($ValidateOnly) { Write-Output "$taskTitle; four manifests; PR body: $taskBodyPath. No fork or PR created."; return }
if (-not $GhPath) {
    $taskGhCommand = Get-Command gh -ErrorAction SilentlyContinue
    $GhPath = if ($taskGhCommand) { $taskGhCommand.Source } else { Join-Path $taskRoot '.tools/github-cli/bin/gh.exe' }
}
if (-not (Test-Path -LiteralPath $GhPath)) { throw 'GitHub CLI is required. Authenticate using gh auth login; do not store a token in this repository.' }
& $GhPath auth status --hostname github.com
if ($LASTEXITCODE -ne 0) { throw 'GitHub authentication is required before creating the registration PR.' }
function Invoke-GhJson([string[]]$Arguments) {
    $taskOutput = & $GhPath @Arguments
    if ($LASTEXITCODE -ne 0) { throw 'GitHub API request failed.' }
    return (($taskOutput -join "`n") | ConvertFrom-Json)
}
function Post-GhJson([string]$Path, $Body) {
    $taskJsonPath = Join-Path $taskArtifacts ('request-' + [guid]::NewGuid().ToString() + '.json')
    [IO.File]::WriteAllText($taskJsonPath, ($Body | ConvertTo-Json -Depth 12 -Compress), [Text.UTF8Encoding]::new($false))
    return (Invoke-GhJson @('api','--hostname','github.com','--method','POST',$Path,'--input',$taskJsonPath))
}
$taskUser = Invoke-GhJson @('api','--hostname','github.com','user')
$taskSearch = Invoke-GhJson @('api','--hostname','github.com','--method','GET','search/issues','-f',('q=repo:microsoft/winget-pkgs is:pr is:open "hojin-v.MiniTerm" "' + $taskVersion + '"'))
$taskDuplicates = @($taskSearch.items | Where-Object { $_.title -like '*hojin-v.MiniTerm*' -and $_.title -like ('*' + $taskVersion + '*') })
if ($taskDuplicates.Count) { Write-Output ('An open registration PR already exists: ' + $taskDuplicates[0].html_url); return }
$taskBody = $taskBody.Replace('- [ ] Checked that there aren''t other open pull requests', '- [x] Checked that there aren''t other open pull requests')
$taskCiHeaders = @{Accept='application/vnd.github+json';'User-Agent'='MiniTerm-winget-submission'}
$taskCi = Invoke-RestMethod -Uri 'https://api.github.com/repos/hojin-v/MiniTerm/actions/workflows/winget.yml/runs?branch=main&per_page=1' -Headers $taskCiHeaders
if (-not $taskCi.workflow_runs.Count -or $taskCi.workflow_runs[0].conclusion -ne 'success') { throw 'WinGet install/removal CI must succeed before registration is submitted.' }
$taskEvidenceDirectory = Join-Path $taskArtifacts ('ci-evidence-' + [guid]::NewGuid().ToString())
& $GhPath run download $taskCi.workflow_runs[0].id --repo hojin-v/MiniTerm --name winget-installation-evidence --dir $taskEvidenceDirectory
if ($LASTEXITCODE -ne 0) { throw 'Cannot verify WinGet installation evidence.' }
$taskEvidence = Get-Content -LiteralPath (Join-Path $taskEvidenceDirectory 'winget-install-verification.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskManifestHashes = $taskFiles | Sort-Object Name | ForEach-Object { $_.Name + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
$taskHasher = [Security.Cryptography.SHA256]::Create()
try { $taskManifestHash = [BitConverter]::ToString($taskHasher.ComputeHash([Text.Encoding]::UTF8.GetBytes(($taskManifestHashes -join "`n") + "`n"))).Replace('-', '').ToLowerInvariant() } finally { $taskHasher.Dispose() }
if ($taskEvidence.packageVersion -ne $taskVersion -or $taskEvidence.manifestSha256 -ne $taskManifestHash -or -not $taskEvidence.installed -or -not $taskEvidence.uninstalled -or -not $taskEvidence.settingsPreserved -or -not $taskEvidence.executableLoaded) { throw 'CI evidence does not prove installation/removal of these exact manifests.' }
$taskBody = $taskBody.Replace('- [ ] Tested with `winget install', '- [x] Tested with `winget install') + "`n`nInstallation validation: " + $taskCi.workflow_runs[0].html_url + "`n"
[IO.File]::WriteAllText($taskBodyPath, $taskBody, [Text.UTF8Encoding]::new($false))
& $GhPath repo fork microsoft/winget-pkgs --clone=false
if ($LASTEXITCODE -ne 0) { throw 'Could not create or find the community repository fork.' }
$taskFork = $taskUser.login + '/winget-pkgs'
$taskForkInfo = Invoke-GhJson @('api','--hostname','github.com',('repos/' + $taskFork))
if (-not $taskForkInfo.fork -or $taskForkInfo.parent.full_name -ne 'microsoft/winget-pkgs') { throw 'The target repository is not a fork of microsoft/winget-pkgs.' }
$taskUpstream = Invoke-GhJson @('api','--hostname','github.com','repos/microsoft/winget-pkgs')
$taskBase = Invoke-GhJson @('api','--hostname','github.com',('repos/microsoft/winget-pkgs/git/ref/heads/' + $taskUpstream.default_branch))
$taskBaseCommit = Invoke-GhJson @('api','--hostname','github.com',('repos/microsoft/winget-pkgs/git/commits/' + $taskBase.object.sha))
$taskTreeEntries = @()
foreach ($taskFile in $taskFiles) {
    $taskBlob = Post-GhJson ('repos/' + $taskFork + '/git/blobs') @{content=[IO.File]::ReadAllText($taskFile.FullName);encoding='utf-8'}
    $taskTreeEntries += @{path=('manifests/h/hojin-v/MiniTerm/' + $taskVersion + '/' + $taskFile.Name);mode='100644';type='blob';sha=$taskBlob.sha}
}
$taskTree = Post-GhJson ('repos/' + $taskFork + '/git/trees') @{base_tree=$taskBaseCommit.tree.sha;tree=$taskTreeEntries}
$taskAuthor = @{name=$taskUser.login;email=([string]$taskUser.id + '+' + $taskUser.login + '@users.noreply.github.com')}
$taskCommit = Post-GhJson ('repos/' + $taskFork + '/git/commits') @{message=$taskTitle;tree=$taskTree.sha;parents=@($taskBase.object.sha);author=$taskAuthor;committer=$taskAuthor}
$taskBranch = 'miniterm-' + $taskVersion + '-' + [guid]::NewGuid().ToString('N').Substring(0,8)
Post-GhJson ('repos/' + $taskFork + '/git/refs') @{ref=('refs/heads/' + $taskBranch);sha=$taskCommit.sha} | Out-Null
$taskPr = Post-GhJson 'repos/microsoft/winget-pkgs/pulls' @{title=$taskTitle;body=$taskBody;head=($taskUser.login + ':' + $taskBranch);base=$taskUpstream.default_branch;maintainer_can_modify=$true}
$taskResult = @{packageIdentifier='hojin-v.MiniTerm';packageVersion=$taskVersion;pullRequestUrl=$taskPr.html_url;pullRequestNumber=$taskPr.number;fork=$taskFork;branch=$taskBranch;status='submitted; awaiting Microsoft validation, review and catalog publication'}
[IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/WINGET_SUBMISSION.json'), ($taskResult | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
Write-Output $taskPr.html_url
