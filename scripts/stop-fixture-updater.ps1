param([Parameter(Mandatory=$true)][string]$Fixture)
$ErrorActionPreference='Stop'
$taskRoot=Split-Path -Parent $PSScriptRoot
$taskFixture=[IO.Path]::GetFullPath($Fixture)
$taskMarker=Get-Content -LiteralPath (Join-Path $taskFixture 'fixture-owned.json') -Encoding UTF8 -Raw | ConvertFrom-Json
if((Split-Path -Parent $taskFixture) -ne (Join-Path $taskRoot 'artifacts') -or $taskMarker.root -ne $taskFixture -or $taskMarker.kind -notin @('miniterm-local-scroll','miniterm-local-shared-bash','miniterm-local-reconnect')){throw 'Expected an owned fixture marker.'}
$taskHome=[IO.Path]::GetFullPath($taskMarker.home)
$taskBases=@($env:LOCALAPPDATA,[IO.Path]::GetTempPath().TrimEnd('\'))
$taskOwned=$false
foreach($taskBase in $taskBases){if($taskHome -match ('^'+[regex]::Escape($taskBase)+'\\mt-[bsr]-[0-9a-f]{8}\\h$')){$taskOwned=$true}}
if(-not $taskOwned){throw 'Expected an isolated short CODEX_HOME.'}
$taskPidFile=Join-Path $taskHome 'app-server-daemon/daemon-updater.pid'
$taskStopped=$false
if(Test-Path -LiteralPath $taskPidFile){
 $taskRecord=Get-Content -LiteralPath $taskPidFile -Encoding UTF8 -Raw | ConvertFrom-Json
 $taskProcess=Get-Process -Id ([int]$taskRecord.pid) -ErrorAction SilentlyContinue
 if($taskProcess){
  if($taskProcess.StartTime.ToFileTimeUtc().ToString() -ne $taskRecord.processStartTime){@{home=$taskHome;stopped=$false;stalePid=$true;passed=$true}|ConvertTo-Json -Compress;return}
  $taskInfo=Get-CimInstance Win32_Process -Filter ('ProcessId='+[int]$taskRecord.pid)
  if($taskInfo.Name -ne 'codex.exe' -or $taskInfo.CommandLine -notmatch ' app-server daemon pid-update-loop$' -or -not $taskInfo.ExecutablePath.StartsWith($taskHome+'\packages\',[StringComparison]::OrdinalIgnoreCase)){throw 'Updater identity differs from the fixture PID record.'}
  Stop-Process -Id $taskProcess.Id -ErrorAction Stop
  $taskStopped=$true
 }
}
@{home=$taskHome;stopped=$taskStopped;passed=$true}|ConvertTo-Json -Compress
