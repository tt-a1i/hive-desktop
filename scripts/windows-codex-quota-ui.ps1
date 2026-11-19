param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$FixtureRoot,
    [Parameter(Mandatory)][string]$OutputPath
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result = [ordered]@{ status = 'failed'; checks = @(); error = $null; controls = @() }
$root = $null

try {
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) { throw 'Desktop process path differs from fixture.' }
    $end = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited early.' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Fixture window unavailable.' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus fixture window.' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    Invoke-Control (Wait-Name 'Settings')
    Invoke-Control (Wait-Name 'External programs')
    Invoke-Control (Wait-Name 'Read Codex account limits')
    [void](Wait-Name 'Account limits loaded')
    [void](Wait-Name 'Account allows ordinary usage')
    $windowLabel = Wait-Control { param($c) $c.Current.Name -match '^Primary window: 78% remaining.*300 minutes' } 'quota window'
    $checked = Wait-Control { param($c) ([string]$c.Current.Name).StartsWith('Checked at:', [StringComparison]::Ordinal) } 'read time'
    $names = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
    if (@($names | Where-Object { $_ -match 'PRIVATE_FIXTURE_' }).Count -ne 0) { throw 'Private fixture account data reached the UI.' }
    $result.checks += @{ status = 'ok'; window = $windowLabel.Current.Name; checkedAt = $checked.Current.Name; privateDataHidden = $true }
    [IO.File]::WriteAllText((Join-Path $FixtureRoot 'ui-observed'), 'observed')
    $end = [DateTime]::UtcNow.AddSeconds(60)
    do {
        if (Test-Path -LiteralPath (Join-Path $FixtureRoot 'close-ui')) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $end)
    if (-not (Test-Path -LiteralPath (Join-Path $FixtureRoot 'close-ui'))) { throw 'Process verification did not complete.' }
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit.' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit within 30 seconds.' }
    $result.checks += @{ exitObserved = $true }
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString() -replace 'PRIVATE_FIXTURE_[A-Z_]+', '[redacted]'
    if ($root) {
        try {
            $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } |
                ForEach-Object { $_ -replace 'PRIVATE_FIXTURE_[A-Z_]+', '[redacted]' } | Select-Object -First 120)
        } catch {}
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Quota fixture failed: $($result.error)" }
