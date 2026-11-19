param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select', 'reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$FirstWorktree,
    [Parameter(Mandatory)][string]$SecondWorktree,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result = [ordered]@{ status = 'failed'; phase = $Phase; checks = @(); error = $null; controls = @() }
$root = $null
try {
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) { throw 'Unexpected desktop executable' }
    $end = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited before showing the window' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Window unavailable' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus window' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    foreach ($item in @(
        @{ title = 'Dirty worktree QA'; path = $FirstWorktree },
        @{ title = 'Other worktree QA'; path = $SecondWorktree }
    )) {
        Invoke-Control (Wait-Name ("Pi · $($item.title) · Conversation"))
        [void](Wait-Name ("Current conversation actions: $($item.title)"))
        Invoke-Control (Wait-Name 'Reference project file')
        [void](Wait-Name ('Isolated worktree execution directory: ' + $item.path))
        $result.checks += "picker uses $($item.title) worktree"
        [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
        [void](Wait-Name 'Reference project file')
    }
    Invoke-Control (Wait-Name 'Pi · Dirty worktree QA · Conversation')
    [void](Wait-Name 'Current conversation actions: Dirty worktree QA')
    Invoke-Control (Wait-Name 'Conversation actions: Dirty worktree QA')
    Invoke-Control (Wait-Name 'Remove isolated worktree…')
    [void](Wait-Control {
        param($c)
        ([string]$c.Current.Name).Contains('Worktree has uncommitted, untracked or ignored files')
    } 'dirty worktree error')
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $buttons = @(All-Controls | Where-Object {
            $_.Current.Name -ceq 'Confirm worktree removal' -and
            $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button
        })
        if ($buttons.Count -eq 1 -and -not $buttons[0].Current.IsEnabled) { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($buttons.Count -ne 1 -or $buttons[0].Current.IsEnabled) { throw 'Dirty removal was not disabled' }
    $result.checks += @('dirty removal rejected', 'confirmation disabled')
    Save-NativeWindow (Join-Path $EvidenceDirectory "windows-worktree-dirty-$Phase.png")
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit' }
    $result.checks += 'normal exit observed'
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try { $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -First 200) } catch {}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory "windows-worktree-dirty-$Phase-failure.png") } catch { $result.captureError = $_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Worktree fixture failed: $($result.error)" }
