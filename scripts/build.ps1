$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'generate-icon.ps1')
& (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('build','--release','--locked')
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskArtifacts = Join-Path $taskRoot 'artifacts'
New-Item -ItemType Directory -Path $taskArtifacts -Force | Out-Null
$taskBinary = Join-Path $taskArtifacts 'miniterm.exe'
Copy-Item -LiteralPath (Join-Path $taskRoot '.tools/target/release/miniterm.exe') -Destination $taskBinary
$taskHash = (Get-FileHash -LiteralPath $taskBinary -Algorithm SHA256).Hash.ToLowerInvariant()
$taskBuildInfo = @{ executableSha256=$taskHash; sourceFingerprint=(& (Join-Path $PSScriptRoot 'source-fingerprint.ps1')); builtUtc=[DateTime]::UtcNow.ToString('o') }
[System.IO.File]::WriteAllText((Join-Path $taskArtifacts 'BINARY_BUILD_INFO.json'),($taskBuildInfo | ConvertTo-Json),[System.Text.UTF8Encoding]::new($false))
[System.IO.File]::WriteAllText((Join-Path $taskArtifacts 'SHA256SUMS'), "$taskHash  miniterm.exe`n", [System.Text.UTF8Encoding]::new($false))
Write-Output $taskBinary
