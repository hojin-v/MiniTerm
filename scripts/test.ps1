param([switch]$Pty, [string]$WslDistribution, [switch]$WindowsBridge, [switch]$CodexNotify, [switch]$AgentBash)
$ErrorActionPreference = 'Stop'
if ($AgentBash) {
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--test','bash_conpty','git_agent_bash','--','--ignored','--nocapture','--test-threads=1')
} elseif ($CodexNotify) {
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
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked','--test','bash_conpty','--','--ignored','--nocapture','--test-threads=1','--skip','wsl_','--skip','git_windows_bridge','--skip','git_codex_notify','--skip','git_agent_bash')
} else {
    & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('test','--locked')
}
