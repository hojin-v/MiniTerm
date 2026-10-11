param([switch]$Battery, [switch]$Goal)
$ErrorActionPreference = 'Stop'
$taskArguments = @((Join-Path $PSScriptRoot 'verify-codex-ui.cjs'), 'plain')
if ($Battery) { $taskArguments += 'battery' }
if ($Goal) {
    if (-not $Battery) { $taskArguments += 'direct' }
    $taskSeed = & node (Join-Path $PSScriptRoot 'verify-codex-ui.cjs') 'plain' 'direct' 'goal-seed'
    if ($LASTEXITCODE -ne 0) { throw "Paused goal seed failed: $taskSeed" }
    $taskSeedResult = $taskSeed | ConvertFrom-Json
    if (-not $taskSeedResult.exited) { throw 'Seed CLI must exit before its own database is copied.' }
    $taskArguments += @('goal-resume', $taskSeedResult.root)
}
$taskCapture = & node @taskArguments
if ($LASTEXITCODE -ne 0) { throw "Native Codex UI capture failed: $taskCapture" }
$taskResult = $taskCapture | ConvertFrom-Json
$taskPreviousCapture = $env:MINITERM_CODEX_UI_CAPTURE
try {
    $env:MINITERM_CODEX_UI_CAPTURE = $taskResult.root
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--lib','native_codex_menu_capture_matches_reference_terminal_after_resize','--','--ignored','--nocapture')
    $taskResult | Add-Member -NotePropertyName parserMatchedReference -NotePropertyValue $true
    [IO.File]::WriteAllText((Join-Path $taskResult.root 'verification.json'), ($taskResult | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    $taskResult | ConvertTo-Json
} finally { $env:MINITERM_CODEX_UI_CAPTURE = $taskPreviousCapture }
