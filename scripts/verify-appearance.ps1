param([string]$Executable = 'artifacts/miniterm.exe')
$ErrorActionPreference = 'Stop'
# Fixed DWM corners replaced radius sliders and the binary window region.
# Each probe owns its settings and IPC namespace, preserving live user sessions.
$taskOpaque = (& (Join-Path $PSScriptRoot 'verify-ux.ps1') -Executable $Executable -Transparency 0) | ConvertFrom-Json
$taskTransparent = (& (Join-Path $PSScriptRoot 'verify-ux.ps1') -Executable $Executable -Transparency 30) | ConvertFrom-Json
$taskResult = [pscustomobject]@{
    opaque = $taskOpaque
    transparent = $taskTransparent
    note = 'Fixed compositor corners, alpha, theme and viewport regression. Does not claim physical mouse, prompt GUI editing, IME or input-to-pixel validation.'
}
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskJson = $taskResult | ConvertTo-Json -Depth 8
[IO.File]::WriteAllText((Join-Path $taskRoot 'artifacts/appearance.json'),$taskJson,[Text.UTF8Encoding]::new($false))
$taskJson
