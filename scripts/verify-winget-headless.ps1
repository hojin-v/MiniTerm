param([string]$ManifestDirectory = 'winget/manifests/h/Hnode/MiniTerm/0.1.0-alpha.6')
$ErrorActionPreference = 'Stop'
# Only disposable hosted runners may adjust the interactive reputation prompt.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or -not $env:RUNNER_TEMP) { throw 'Headless installation is restricted to an ephemeral GitHub-hosted Windows runner.' }
& (Join-Path $PSScriptRoot 'verify-winget.ps1') -ManifestDirectory $ManifestDirectory -VerifyInstaller
$taskSettings = @(
    @{path='HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer';name='SmartScreenEnabled';value='Off';kind='String'},
    @{path='HKLM:\SOFTWARE\Policies\Microsoft\Windows\System';name='EnableSmartScreen';value=0;kind='DWord'},
    # Shell Attachment Manager has a separate unsigned-download prompt even
    # when SmartScreen is off. This disposable runner executes verified EXEs.
    @{path='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Associations';name='LowRiskFileTypes';value='.exe';kind='String'}
)
$taskSaved = @()
try {
    foreach ($taskSetting in $taskSettings) {
        $taskKey = Get-Item -LiteralPath $taskSetting.path -ErrorAction SilentlyContinue
        $taskExists = $taskKey -and $taskSetting.name -in $taskKey.GetValueNames()
        $taskSaved += @{path=$taskSetting.path;name=$taskSetting.name;exists=[bool]$taskExists;value=$(if ($taskExists) {$taskKey.GetValue($taskSetting.name)} else {$null});kind=$(if ($taskExists) {$taskKey.GetValueKind($taskSetting.name).ToString()} else {$null})}
        if (-not $taskKey) { New-Item -Path $taskSetting.path -Force | Out-Null }
        New-ItemProperty -LiteralPath $taskSetting.path -Name $taskSetting.name -Value $taskSetting.value -PropertyType $taskSetting.kind -Force | Out-Null
    }
    Write-Output '::notice title=Headless installer scope::SmartScreen and Attachment Manager EXE prompts are disabled only for this disposable runner test; file hashes and Defender remain enforced. This is not a reputation test.'
    & (Join-Path $PSScriptRoot 'verify-winget-install.ps1') -ManifestDirectory $ManifestDirectory
} finally {
    foreach ($taskSetting in $taskSaved) {
        if ($taskSetting.exists) {
            New-ItemProperty -LiteralPath $taskSetting.path -Name $taskSetting.name -Value $taskSetting.value -PropertyType $taskSetting.kind -Force | Out-Null
        } else {
            Remove-ItemProperty -LiteralPath $taskSetting.path -Name $taskSetting.name -ErrorAction SilentlyContinue
        }
    }
    Write-Output '::notice title=Headless installer cleanup::Original SmartScreen and Attachment Manager registry values restored.'
}
