param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select', 'reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$Sha256,
    [Parameter(Mandatory)][string]$Worktree,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result = [ordered]@{
    status = 'failed'
    phase = $Phase
    stages = [ordered]@{}
    error = $null
    controls = @()
}
$app = $null
$root = $null


try {
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) {
        throw 'Desktop process path differs from the extracted fixture.'
    }
    $end = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited early.' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') {
        throw 'Fixture window unavailable.'
    }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) {
        throw 'Could not focus fixture window.'
    }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    [void](Wait-Name 'Reference project file')
    if ($Phase -eq 'select') {
        Invoke-Control (Wait-Name 'Reference project file')
        [void](Wait-Name ('Isolated worktree execution directory: ' + $Worktree))
        $result.stages.pickerUsesWorktree = $true
        $search = Wait-Name 'Search project files'
        $search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('shared-reference.txt')
        Invoke-Control (Wait-Name 'Reference project file: shared-reference.txt')
        $result.stages.selected = $true
    }
    $attachment = Wait-Control {
        param($c)
        $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
            ([string]$c.Current.Name).StartsWith('Project file snapshot: shared-reference.txt', [StringComparison]::Ordinal)
    } 'snapshot attachment'
    if (-not $attachment.Current.Name.Contains($Sha256)) {
        throw 'Snapshot attachment SHA-256 differs from the worktree bytes.'
    }
    $result.stages.snapshotLabel = $true
    if ($Phase -eq 'reopen') {
        Invoke-Control $attachment
        [void](Wait-Name ('SHA-256: ' + $Sha256))
        [void](Wait-Name 'Source chat: Worktree reference')
        Invoke-Control (Wait-Name 'Check source file')
        [void](Wait-Name 'Source file is missing; the saved snapshot is still available')
        $result.stages.sourceMissing = $true
        Invoke-Control (Wait-Name 'Preview attachment copy')
        [void](Wait-Control { param($c) $c.Current.Name -match 'WORKTREE_SNAPSHOT_CONTENT' } 'snapshot content')
        if (@(All-Controls | Where-Object { $_.Current.Name -match 'MAIN_ROOT_CONTENT' }).Count -ne 0) {
            throw 'Root file content appeared in the snapshot preview.'
        }
        $result.stages.previewWorktreeContent = $true
    }
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal fixture exit.' }
    if (-not $app.WaitForExit(30000)) {
        throw 'Fixture did not exit within 30 seconds.'
    }
    # The Node parent owns the process handle and checks its actual exit code.
    $result.stages.exitObserved = $true
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try {
            $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } |
                Where-Object { $_ } | Select-Object -First 150)
        } catch {}
        try {
            $rect = New-Object NativeFixtureWindow+Rect
            if ([NativeFixtureWindow]::GetWindowRect($window, [ref]$rect)) {
                $bitmap = [Drawing.Bitmap]::new(($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
                $graphics = [Drawing.Graphics]::FromImage($bitmap)
                try {
                    $dc = $graphics.GetHdc()
                    try { [void][NativeFixtureWindow]::PrintWindow($window, $dc, 2) }
                    finally { $graphics.ReleaseHdc($dc) }
                    $bitmap.Save((Join-Path $EvidenceDirectory ("windows-project-reference-$Phase-failure.png")))
                } finally {
                    $graphics.Dispose()
                    $bitmap.Dispose()
                }
            }
        } catch { $result.captureError = $_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Reference fixture failed: $($result.error)" }
