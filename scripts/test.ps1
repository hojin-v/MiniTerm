param([switch]$Pty, [string]$WslDistribution, [switch]$WindowsBridge, [switch]$CodexNotify)
$ErrorActionPreference = 'Stop'
if ($CodexNotify) {
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--test','bash_conpty','git_codex_notify','--','--ignored','--nocapture','--test-threads=1')
} elseif ($WindowsBridge) {
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--test','bash_conpty','git_windows_bridge','--','--ignored','--nocapture','--test-threads=1')
} elseif ($WslDistribution) {
    $taskPreviousDistribution = $env:MINITERM_WSL_TEST_DISTRO
    try {
        $env:MINITERM_WSL_TEST_DISTRO = $WslDistribution
        & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--test','bash_conpty','wsl_','--','--ignored','--nocapture','--test-threads=1')
    } finally { $env:MINITERM_WSL_TEST_DISTRO = $taskPreviousDistribution }
} elseif ($Pty) {
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--test','bash_conpty','--','--ignored','--nocapture','--test-threads=1','--skip','wsl_','--skip','git_windows_bridge','--skip','git_codex_notify')
} else {
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked')
}
