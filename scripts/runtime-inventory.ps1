param([Parameter(Mandatory=$true)][string]$RuntimeDirectory, [Parameter(Mandatory=$true)][string]$ArtifactsDirectory)
$ErrorActionPreference = 'Stop'
$taskRuntime = (Get-Item -LiteralPath $RuntimeDirectory).FullName.TrimEnd('\')
$taskFiles = @(Get-ChildItem -LiteralPath $taskRuntime -File -Recurse | Sort-Object FullName | ForEach-Object {
    @{path=('runtime/' + $_.FullName.Substring($taskRuntime.Length + 1).Replace('\','/'));sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant();bytes=$_.Length}
})
[IO.File]::WriteAllText((Join-Path $ArtifactsDirectory 'BASH_RUNTIME_FILES.json'),(@{schemaVersion=1;files=$taskFiles} | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
$taskSums = @($taskFiles | ForEach-Object { $_.sha256 + '  ' + $_.path })
[IO.File]::WriteAllText((Join-Path $ArtifactsDirectory 'RUNTIME_SHA256SUMS'),($taskSums -join "`n") + "`n",[Text.UTF8Encoding]::new($false))
$taskDelete = @($taskFiles | ForEach-Object { '  Delete "$INSTDIR\' + $_.path.Replace('/','\').Replace('$','$$') + '"' })
$taskDelete += @(Get-ChildItem -LiteralPath $taskRuntime -Directory -Recurse | Sort-Object { $_.FullName.Length } -Descending | ForEach-Object { '  RMDir "$INSTDIR\runtime\' + $_.FullName.Substring($taskRuntime.Length + 1).Replace('$','$$') + '"' })
$taskDelete += '  RMDir "$INSTDIR\runtime"'
[IO.File]::WriteAllText((Join-Path $ArtifactsDirectory 'runtime-delete.nsh'),($taskDelete -join "`n") + "`n",[Text.UTF8Encoding]::new($false))
