param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select','reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$OriginalText,
    [Parameter(Mandatory)][string]$EditedText,
    [Parameter(Mandatory)][string]$CanceledText,
    [Parameter(Mandatory)][string]$Draft,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result = [ordered]@{status='failed'; phase=$Phase; stages=[ordered]@{}; error=$null; controls=@()}
$app=$null; $root=$null
function Row-Action([string]$Text,[string]$Action) {
    $row = Wait-Control {
        param($c)
        $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Group -and
            ([string]$c.Current.Name).StartsWith($Text + ' ·', [StringComparison]::Ordinal)
    } ('queued row ' + $Text)
    $buttons = @($row.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -ceq $Action })
    if ($buttons.Count -ne 1) { throw "Queued row action ambiguous/unavailable: $Text / $Action" }
    return $buttons[0]
}
function Assert-Draft {
    [void](Wait-Control {
        param($c)
        $pattern=$null
        $c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$pattern) -and
            $pattern.Current.Value -ceq $Draft
    } 'independent composer draft')
    $result.stages.draftPreserved=$true
}
try {
    $app=Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) { throw 'Unexpected desktop executable' }
    $end=[DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited early' }
        $window=$app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Fixture window unavailable' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus fixture window' }
    $root=[System.Windows.Automation.AutomationElement]::FromHandle($window)
    Assert-Draft
    if ($Phase -eq 'select') {
        Invoke-Control (Row-Action $OriginalText 'Edit queued message')
        $input=Wait-Name 'Queued message text'
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($EditedText)
        Invoke-Control (Wait-Name 'Save changes')
        [void](Row-Action $EditedText 'Edit queued message')
        Assert-Draft
        Invoke-Control (Row-Action $CanceledText 'Cancel queued message')
        Invoke-Control (Wait-Name 'Confirm cancellation')
        $end=[DateTime]::UtcNow.AddSeconds(15)
        do {
            $remaining=@(All-Controls | Where-Object { ([string]$_.Current.Name).StartsWith($CanceledText + ' ·',[StringComparison]::Ordinal) })
            if ($remaining.Count -eq 0) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $end)
        if ($remaining.Count) { throw 'Canceled input remained in the executable queue' }
        $result.stages.edited=$true; $result.stages.canceled=$true
    } else {
        [void](Row-Action $EditedText 'Edit queued message')
        if (@(All-Controls | Where-Object { ([string]$_.Current.Name).StartsWith($CanceledText + ' ·',[StringComparison]::Ordinal) }).Count) {
            throw 'Canceled input returned after cold reopen'
        }
        Invoke-Control (Wait-Name 'Resume processing')
        [void](Wait-Control {param($c) $c.Current.Name -match 'Synthetic queued completion'} 'controlled resumed completion')
        $result.stages.explicitResume=$true
    }
    Assert-Draft
    Save-NativeWindow (Join-Path $EvidenceDirectory ("windows-queued-inputs-$Phase.png"))
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit' }
    $result.stages.exitObserved=$true; $result.status='passed'
} catch {
    $result.error=$_.Exception.ToString()
    if ($root) {
        try { $result.controls=@(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -First 180) } catch {}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory ("windows-queued-inputs-$Phase-failure.png")) } catch { $result.captureError=$_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath,($result | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Queued input fixture failed: $($result.error)" }
