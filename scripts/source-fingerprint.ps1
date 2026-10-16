$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskInputs = @('Cargo.toml','Cargo.lock','rust-toolchain.toml','build.rs','assets/miniterm.ico','assets/miniterm.svg','assets/brand_pixels.rs','src/prompt.bash') + @(Get-ChildItem -LiteralPath (Join-Path $taskRoot 'src') -Recurse -File -Filter '*.rs' | ForEach-Object { $_.FullName.Substring($taskRoot.Length + 1).Replace('\','/') })
$taskVendor = Join-Path $taskRoot 'vendor'
if (Test-Path -LiteralPath $taskVendor) { $taskInputs += @(Get-ChildItem -LiteralPath $taskVendor -Recurse -File | Where-Object { $_.Extension -in '.rs','.toml' } | ForEach-Object { $_.FullName.Substring($taskRoot.Length + 1).Replace('\','/') }) }
$taskLines = foreach ($taskInput in ($taskInputs | Sort-Object)) { $taskHash = (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskInput) -Algorithm SHA256).Hash.ToLowerInvariant(); "$taskInput $taskHash" }
$taskDigest = [Security.Cryptography.SHA256]::Create()
try { ([BitConverter]::ToString($taskDigest.ComputeHash([Text.Encoding]::UTF8.GetBytes(($taskLines -join "`n") + "`n")))).Replace('-','').ToLowerInvariant() } finally { $taskDigest.Dispose() }
