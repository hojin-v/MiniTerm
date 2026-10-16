param([datetime]$Day = [datetime]'2026-10-13')
$ErrorActionPreference = 'Stop'
$taskIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (-not ([Security.Principal.WindowsPrincipal]$taskIdentity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Windows requires an administrator PowerShell for SRUM and protected WER metadata. This script does not elevate itself.'
}
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskOutput = Join-Path $taskRoot ('artifacts\battery-evidence-' + $Day.ToString('yyyyMMdd') + '-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskOutput | Out-Null
$taskStatus = [ordered]@{ collectedAt = (Get-Date).ToString('o'); day = $Day.ToString('yyyy-MM-dd'); output = $taskOutput }
foreach ($taskReport in @('srum', 'battery', 'sleepstudy')) {
    $taskArguments = switch ($taskReport) {
        'srum' { @('/srumutil', '/output', (Join-Path $taskOutput 'srum.csv'), '/csv') }
        'battery' { @('/batteryreport', '/output', (Join-Path $taskOutput 'battery-report.xml'), '/xml') }
        'sleepstudy' { @('/sleepstudy', '/output', (Join-Path $taskOutput 'sleepstudy.xml'), '/xml', '/duration', '3') }
    }
    $taskMessages = & powercfg.exe @taskArguments 2>&1
    $taskStatus[$taskReport] = @{ exitCode = $LASTEXITCODE; messages = @($taskMessages | ForEach-Object { $_.ToString() }) }
}
# Read report timestamps/signatures and dump file metadata; never read or copy dump contents.
$taskEvents = @(Get-WinEvent -FilterHashtable @{ LogName = 'Application'; ProviderName = 'Windows Error Reporting'; StartTime = $Day.Date; EndTime = $Day.Date.AddDays(1) } -ErrorAction SilentlyContinue)
$taskReports = @($taskEvents | ForEach-Object {
    $taskXml = [xml]$_.ToXml()
    $taskData = @{}
    foreach ($taskField in $taskXml.Event.EventData.Data) { $taskData[[string]$taskField.Name] = [string]$taskField.InnerText }
    if ($taskData.EventName -ne 'LiveKernelEvent') { return }
    $taskPath = Join-Path ($taskData.StorePath -replace '^\\\\\?\\', '') 'Report.wer'
    $taskMetadata = @{}
    $taskReadError = $null
    try {
        foreach ($taskLine in (Get-Content -LiteralPath $taskPath)) {
            if ($taskLine -match '^(EventTime|ReportIdentifier|EventType|Sig\[\d+\]\.(?:Name|Value)|DynamicSig\[\d+\]\.(?:Name|Value))=(.*)$') { $taskMetadata[$matches[1]] = $matches[2] }
        }
    } catch { $taskReadError = $_.Exception.Message }
    $taskOccurred = $null
    if ($taskMetadata.ContainsKey('EventTime')) { $taskOccurred = [datetime]::FromFileTimeUtc([int64]$taskMetadata.EventTime).ToLocalTime().ToString('o') }
    [pscustomobject]@{ reportedAt = $_.TimeCreated.ToString('o'); recordId = $_.RecordId; reportId = $taskData.ReportId; code = $taskData.P1; parameter1 = $taskData.P2; occurredAt = $taskOccurred; attachments = $taskData.AttachedFiles; metadata = $taskMetadata; readError = $taskReadError }
})
ConvertTo-Json -InputObject $taskReports -Depth 6 | Set-Content -LiteralPath (Join-Path $taskOutput 'kernel-report-metadata.json') -Encoding UTF8
$taskDumps = @(Get-ChildItem -LiteralPath 'C:\Windows\LiveKernelReports' -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.CreationTime.Date -eq $Day.Date -or $_.LastWriteTime.Date -eq $Day.Date } | Select-Object FullName, Length, @{n='createdAt';e={$_.CreationTime.ToString('o')}}, @{n='modifiedAt';e={$_.LastWriteTime.ToString('o')}})
ConvertTo-Json -InputObject $taskDumps -Depth 4 | Set-Content -LiteralPath (Join-Path $taskOutput 'dump-file-metadata.json') -Encoding UTF8
$taskStatus | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskOutput 'collection-status.json') -Encoding UTF8
$taskStatus | ConvertTo-Json -Depth 5
