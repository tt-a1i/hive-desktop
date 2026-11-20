param(
 [Parameter(Mandatory)][int]$AppProcessId,
 [Parameter(Mandatory)][string]$Executable,
 [Parameter(Mandatory)][ValidateSet('select','reopen')][string]$Phase,
 [Parameter(Mandatory)][string]$CheckpointId,
 [Parameter(Mandatory)][string]$Draft,
 [Parameter(Mandatory)][string]$OutputPath,
 [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result=[ordered]@{status='failed';phase=$Phase;checks=@();error=$null;controls=@()}
$root=$null
function Assert-Draft {
 [void](Wait-Control {param($c) $p=$null; $c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$p) -and $p.Current.Value -ceq $Draft} 'unchanged independent draft')
}
try {
 $app=Get-Process -Id $AppProcessId
 if($app.Path -ine [IO.Path]::GetFullPath($Executable)){throw 'Unexpected desktop executable'}
 $end=[DateTime]::UtcNow.AddSeconds(30)
 do {
  $app.Refresh();if($app.HasExited){throw 'Desktop exited early'}
  $window=$app.MainWindowHandle
  if($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive'){break}
  Start-Sleep -Milliseconds 150
 }while([DateTime]::UtcNow -lt $end)
 if($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive'){throw 'Fixture window unavailable'}
 if(-not [NativeFixtureWindow]::SetForegroundWindow($window)){throw 'Could not focus fixture'}
 $root=[System.Windows.Automation.AutomationElement]::FromHandle($window)
 [void](Wait-Name 'Current conversation actions: Rewind lock source')
 Assert-Draft
 $end=[DateTime]::UtcNow.AddSeconds(30)
 do {
  $rewind=@(All-Controls | Where-Object {$_.Current.Name -ceq 'Rewind in place' -and $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and $_.Current.IsEnabled})
  if($rewind.Count -eq 2){break}
  Start-Sleep -Milliseconds 150
 }while([DateTime]::UtcNow -lt $end)
 if($rewind.Count -ne 2){throw 'Expected both native user history boundaries'}
 $result.checks+='two native user boundaries visible'
 if($Phase -eq 'select') {
  # The two-turn peer emits u1 then u2. Disk verification also checks exact u2.
  Invoke-Control $rewind[1]
  Invoke-Control (Wait-Name ('Checkpoint '+$CheckpointId))
  Invoke-Control (Wait-Name 'Preview rewind')
  Invoke-Control (Wait-Name 'Confirm rewind in place')
  $status=Wait-Control {param($c) ([string]$c.Current.Name).StartsWith('Operation status: failed '+[char]0x00B7+' ',[StringComparison]::Ordinal)} 'failed rewind operation status'
  $result.operationId=($status.Current.Name -split ' ')[-1]
  if($result.operationId -notmatch '^[a-f0-9-]{36}$'){throw 'Failed operation identity unavailable'}
  [void](Wait-Name '[CHECKPOINT_RESTORE_CONFLICT] Git index is locked by another operation')
  $end=[DateTime]::UtcNow.AddSeconds(10)
  do {
   $apply=@(All-Controls | Where-Object {$_.Current.Name -ceq 'Confirm rewind in place' -and $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button})
   if($apply.Count -eq 1 -and -not $apply[0].Current.IsEnabled){break}
   Start-Sleep -Milliseconds 100
  }while([DateTime]::UtcNow -lt $end)
  if($apply.Count -ne 1 -or $apply[0].Current.IsEnabled){throw 'Failed operation must disable reapplying the preview'}
  $result.checks+=@('selected original checkpoint','confirmed rewind through native UI','failed operation status and identity readable','exact foreign index lock rejection readable','failed preview cannot be reapplied')
  Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-rewind-lock-rejection.png')
  [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
  Assert-Draft
  $result.checks+='independent draft unchanged after cancel'
 } else {
  $result.checks+='cold reopen retains source and independent draft'
  Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-rewind-lock-reopen.png')
 }
 if(-not $app.CloseMainWindow()){throw 'Could not request normal exit'}
 if(-not $app.WaitForExit(30000)){throw 'Desktop did not exit'}
 $result.checks+='normal OS process exit observed';$result.status='passed'
}catch {
 $result.error=$_.Exception.ToString()
 if($root) {
  try {$result.controls=@(All-Controls | ForEach-Object {$v=$null;$value=$null;if($_.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$v)){$value=$v.Current.Value};[ordered]@{name=$_.Current.Name;value=$value;type=$_.Current.ControlType.ProgrammaticName;enabled=$_.Current.IsEnabled}})}catch{}
  try {Save-NativeWindow (Join-Path $EvidenceDirectory "windows-rewind-lock-$Phase-failure.png")}catch{$result.captureError=$_.Exception.Message}
 }
}finally {
 [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath)))|Out-Null
 [IO.File]::WriteAllText($OutputPath,($result|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
}
if($result.status -ne 'passed'){throw "Rewind lock fixture failed: $($result.error)"}
