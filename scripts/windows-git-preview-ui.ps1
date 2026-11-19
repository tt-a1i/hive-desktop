param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select','reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$Draft,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result = [ordered]@{status='failed';phase=$Phase;stages=[ordered]@{};error=$null;controls=@()}
$root = $null
try {
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) { throw 'Desktop executable identity mismatch.' }
    $end = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited early.' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Fixture window unavailable.' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Cannot focus fixture window.' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    $composer = Wait-Name 'Conversation messages'
    if ($composer.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne $Draft) { throw 'Initial chat draft differs.' }
    $result.stages.draftVisible = $true
    if ($Phase -eq 'select') {
        Invoke-Control (Wait-Name 'Git status and diff')
        [void](Wait-Name 'a000.txt · .M · ordinary')
        if (@(All-Controls | Where-Object { ([string]$_.Current.Name).StartsWith('z-staged.txt ·', [StringComparison]::Ordinal) }).Count -ne 0) { throw 'Staged target unexpectedly on first status page.' }
        $result.stages.targetOutsideFirstPage = $true
        Invoke-Control (Wait-Name 'Commit…')
        [void](Wait-Control {param($c) $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and ([string]$c.Current.Name).Contains('z-staged.txt') } 'off-page target in complete commit preview')
        $result.stages.offPageCommitPreview = $true
        Invoke-Control (Wait-Name 'Cancel')
        Invoke-Control (Wait-Name 'Next page')
        Invoke-Control (Wait-Name 'z-staged.txt · M. · ordinary')
        $result.stages.selectedStagedFile = $true
        Invoke-Control (Wait-Name 'Commit…')
        [void](Wait-Control {param($c) $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and ([string]$c.Current.Name).Contains('z-staged.txt') } 'selected staged file in commit preview')
        Invoke-Control (Wait-Name 'View staged patch')
        [void](Wait-Control {param($c) ([string]$c.Current.Name).Contains('+STAGED_PRIVATE_FIXTURE') } 'expected staged patch')
        $result.stages.patchVerified = $true
        Invoke-Control (Wait-Name 'Cancel')
        Invoke-Control (Wait-Name 'Close Git drawer')
        $composer = Wait-Name 'Conversation messages'
        if ($composer.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne $Draft) { throw 'Chat draft changed after cancel.' }
        $result.stages.cancelledAndDraftPreserved = $true
    }
    if (-not $app.CloseMainWindow()) { throw 'Could not close fixture.' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit.' }
    $result.stages.exitObserved = $true
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try { $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -First 150) } catch {}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory "windows-git-preview-$Phase-failure.png") } catch { $result.captureError=$_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath,($result | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Git preview fixture failed: $($result.error)" }
