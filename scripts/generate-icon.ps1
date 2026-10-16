$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskCompiler = Join-Path $taskRoot '.tools/cargo/bin/rustc.exe'
if (Test-Path -LiteralPath $taskCompiler) {
    $env:CARGO_HOME = Join-Path $taskRoot '.tools/cargo'
    $env:RUSTUP_HOME = Join-Path $taskRoot '.tools/rustup'
} else { $taskCompiler = 'rustc' }
$taskOutput = Join-Path $taskRoot 'artifacts/generate-brand-icon.exe'
New-Item -ItemType Directory -Path (Split-Path -Parent $taskOutput) -Force | Out-Null
& $taskCompiler --edition=2021 (Join-Path $taskRoot 'assets/generate_icon.rs') -o $taskOutput
if ($LASTEXITCODE -ne 0) { throw 'Brand icon generator build failed.' }
& $taskOutput (Join-Path $taskRoot 'assets/miniterm.ico')
if ($LASTEXITCODE -ne 0) { throw 'Brand icon generation failed.' }
