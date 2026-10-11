$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskTools = Join-Path $taskRoot '.tools'
New-Item -ItemType Directory -Path $taskTools -Force | Out-Null
$taskInstaller = Join-Path $taskTools 'rustup-init.exe'
Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $taskInstaller
$env:CARGO_HOME = Join-Path $taskTools 'cargo'
$env:RUSTUP_HOME = Join-Path $taskTools 'rustup'
& $taskInstaller -y --no-modify-path --profile minimal --default-toolchain 1.90.0 --default-host x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Rust 설치 실패' }
