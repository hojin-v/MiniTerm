param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArguments)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskCargo = Join-Path $taskRoot '.tools/cargo/bin/cargo.exe'
if (Test-Path -LiteralPath $taskCargo) {
    $env:CARGO_HOME = Join-Path $taskRoot '.tools/cargo'
    $env:RUSTUP_HOME = Join-Path $taskRoot '.tools/rustup'
    $env:PATH = (Join-Path $taskRoot '.tools/cargo/bin') + ';' + $env:PATH
} else {
    $taskCargo = 'cargo'
}
$env:CARGO_TARGET_DIR = Join-Path $taskRoot '.tools/target'
Push-Location -LiteralPath $taskRoot
try { & $taskCargo @CargoArguments; if ($LASTEXITCODE -ne 0) { throw "cargo failed ($LASTEXITCODE)" } } finally { Pop-Location }
