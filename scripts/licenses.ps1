$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
$taskMetadata = & (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArguments @('metadata','--locked','--format-version','1','--filter-platform','x86_64-pc-windows-msvc') | ConvertFrom-Json
$taskParts = New-Object System.Collections.Generic.List[string]
$taskParts.Add('<!doctype html><meta charset="utf-8"><title>MiniTerm third-party notices</title><style>body{font:15px system-ui;margin:3rem;max-width:70rem}pre{white-space:pre-wrap}summary{cursor:pointer}</style><h1>MiniTerm third-party notices</h1><p>Locked Windows dependency metadata and licenses distributed in each crate. Build tools and dependencies may be included conservatively.</p>')
$taskMissing = New-Object System.Collections.Generic.List[string]
$taskSupplement = Get-Content (Join-Path $taskRoot 'licenses/SOURCES.json') -Raw -Encoding UTF8 | ConvertFrom-Json
foreach ($taskPackage in ($taskMetadata.packages | Sort-Object name,version)) {
    if ($taskPackage.name -eq 'miniterm') { continue }
    $taskPath = Split-Path -Parent $taskPackage.manifest_path
    $taskFiles = @(Get-ChildItem -LiteralPath $taskPath -Recurse -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|COPYRIGHT|NOTICE|OFL|UFL)([._-].*)?$' -or ($taskPackage.name -eq 'epaint_default_fonts' -and $_.Extension -eq '.txt') })
    foreach ($taskSource in @($taskSupplement | Where-Object { $_.packages -contains $taskPackage.name -and $_.version -eq $taskPackage.version })) {
        $taskLicensePath = Join-Path (Join-Path $taskRoot 'licenses') $taskSource.path
        if ((Get-FileHash -LiteralPath $taskLicensePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskSource.sha256) { throw "Supplemental license checksum mismatch: $($taskSource.path)" }
        $taskFiles += Get-Item -LiteralPath $taskLicensePath
    }
    if ($taskPackage.license_file) {
        $taskExplicit = Join-Path $taskPath $taskPackage.license_file
        if (Test-Path -LiteralPath $taskExplicit) { $taskFiles += Get-Item -LiteralPath $taskExplicit }
    }
    $taskFiles = @($taskFiles | Sort-Object FullName -Unique)
    if ($taskFiles.Count -eq 0) { $taskMissing.Add("$($taskPackage.name) $($taskPackage.version)") }
    $taskName = [System.Net.WebUtility]::HtmlEncode("$($taskPackage.name) $($taskPackage.version) — $($taskPackage.license)")
    $taskParts.Add("<details><summary>$taskName</summary>")
    foreach ($taskFile in $taskFiles) {
        $taskText = [System.Net.WebUtility]::HtmlEncode([System.IO.File]::ReadAllText($taskFile.FullName))
        $taskFileName = [System.Net.WebUtility]::HtmlEncode($taskFile.Name)
        $taskParts.Add("<h3>$taskFileName</h3><pre>$taskText</pre>")
    }
    $taskParts.Add('</details>')
}
if ($taskMissing.Count -gt 0) { throw "Missing dependency license text: $($taskMissing -join ', ')" }
$taskOutput = Join-Path $taskRoot 'artifacts/THIRD_PARTY_NOTICES.html'
New-Item -ItemType Directory -Path (Split-Path -Parent $taskOutput) -Force | Out-Null
[System.IO.File]::WriteAllText($taskOutput,($taskParts -join "`n"),[System.Text.UTF8Encoding]::new($false))
Write-Output $taskOutput
