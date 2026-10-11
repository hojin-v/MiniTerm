$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskTools = Join-Path $taskRoot '.tools'
$taskCompiler = Join-Path $taskTools 'nsis/nsis-3.13/makensis.exe'
if (Test-Path -LiteralPath $taskCompiler) { Write-Output $taskCompiler; return }
New-Item -ItemType Directory -Path $taskTools -Force | Out-Null
$taskArchive = Join-Path $taskTools 'nsis-3.13.zip'
$taskExpected = 'ba63dffc4410ee89193e1cb5a41989991bd77c61068da17e3156d136b7b0b3d8'
$taskUrl = 'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.13/nsis-3.13.zip'
for ($taskAttempt = 0; $taskAttempt -lt 3; $taskAttempt++) {
    Invoke-WebRequest -Uri $taskUrl -OutFile $taskArchive -UseBasicParsing
    $taskBytes = [System.IO.File]::ReadAllBytes($taskArchive)
    if ($taskBytes.Length -gt 2 -and $taskBytes[0] -eq 80 -and $taskBytes[1] -eq 75) { break }
    $taskHtml = [System.Text.Encoding]::UTF8.GetString($taskBytes)
    $taskMatch = [regex]::Match($taskHtml,'data-release-url="([^"]+)"')
    if (-not $taskMatch.Success) { throw 'NSIS download was not a ZIP archive' }
    $taskUrl = [System.Net.WebUtility]::HtmlDecode($taskMatch.Groups[1].Value)
    if ([Uri]$taskUrl -notmatch '^https://downloads\.sourceforge\.net/') { throw 'Unexpected NSIS download host' }
}
if ((Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskExpected) { throw 'NSIS archive checksum mismatch' }
Expand-Archive -LiteralPath $taskArchive -DestinationPath (Join-Path $taskTools 'nsis') -Force
if (-not (Test-Path -LiteralPath $taskCompiler)) { throw 'NSIS compiler not found' }
Write-Output $taskCompiler
