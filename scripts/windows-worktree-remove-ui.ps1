param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select', 'reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$Worktree,
    [Parameter(Mandatory)][string]$Branch,
    [Parameter(Mandatory)][string]$Head,
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
        if ($app.HasExited) { throw 'Desktop exited before showing a window' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Window unavailable' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus window' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    [void](Wait-Name 'Current conversation actions: Clean worktree QA')
    [void](Wait-Name 'ORIGINAL WORKTREE REMOVAL HISTORY 9153')
    if ($Phase -eq 'select') {
        Invoke-Control (Wait-Name 'Conversation actions: Clean worktree QA')
        Invoke-Control (Wait-Name 'Remove isolated worktree…')
        # Preview identity is exposed by label values; require both the exact
        # owned path and the branch/head before invoking the destructive action.
        foreach ($expected in @($Worktree, "$Branch · $Head")) {
            [void](Wait-Control {
                param($c)
                if ($c.Current.Name -ceq $expected) { return $true }
                $pattern = $null
                if ($c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
                    return $pattern.Current.Value -ceq $expected
                }
                return $false
            } "preview identity $expected")
        }
        $result.checks += @('preview owned path', 'preview branch and head')
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-worktree-remove-preview.png')
        Invoke-Control (Wait-Name 'Confirm worktree removal')
        [void](Wait-Name 'Worktree removed. The original conversation history and Git branch were kept.')
        if (Test-Path -LiteralPath $Worktree) { throw 'Confirmed worktree still exists' }
        $result.checks += 'confirmed removal'
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-worktree-remove-confirmed.png')
    } else {
        [void](Wait-Name 'This historical session is read only. Your draft is preserved; sending, resuming and model changes are unavailable.')
        $editor = Wait-Name 'Conversation messages'
        if ($editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne 'Removal draft remains intact') { throw 'Draft changed' }
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        do {
            $send = @(All-Controls | Where-Object { $_.Current.Name -ceq 'Send message' -and $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button })
            if ($send.Count -eq 1 -and -not $send[0].Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($send.Count -ne 1 -or $send[0].Current.IsEnabled) { throw 'Read-only send was not disabled' }
        if (Test-Path -LiteralPath $Worktree) { throw 'Worktree reappeared on cold launch' }
        $result.checks += @('original history visible', 'original session selected', 'read-only notice', 'draft retained', 'send disabled', 'worktree remains absent')
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-worktree-remove-cold.png')
    }
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit' }
    $result.checks += 'normal exit observed'
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try { $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -First 200) } catch {}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory "windows-worktree-remove-$Phase-failure.png") } catch { $result.captureError = $_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Worktree removal fixture failed: $($result.error)" }
