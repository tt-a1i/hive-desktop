param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select','reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$Draft,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class GitPreviewWheel {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, UIntPtr extra);
}
'@
function Read-Geometry($Control) {
    $r = $Control.Current.BoundingRectangle
    return [ordered]@{x=$r.X;y=$r.Y;width=$r.Width;height=$r.Height;offscreen=$Control.Current.IsOffscreen}
}
function Visible-InWindow($Control) {
    $r = $Control.Current.BoundingRectangle
    return (-not $Control.Current.IsOffscreen -and -not $r.IsEmpty -and
        $root.Current.BoundingRectangle.Contains($r.X + $r.Width / 2, $r.Y + $r.Height / 2))
}
function Read-Focus {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($null -eq $focused) { return $null }
    return [ordered]@{name=$focused.Current.Name;processId=$focused.Current.ProcessId;geometry=(Read-Geometry $focused)}
}
function Observe-Focus {
    try {
        $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
        if ($null -eq $focused) { return @{valid=$false;name=$null} }
        $id = ($focused.GetRuntimeId() -join ',')
        $inTree = $id -ne '' -and @(All-Controls | Where-Object { ($_.GetRuntimeId() -join ',') -ceq $id }).Count -gt 0
        $actionable = $focused.Current.ProcessId -eq $AppProcessId -and $focused.Current.IsEnabled -and
            $focused.Current.IsKeyboardFocusable -and (Visible-InWindow $focused)
        return [ordered]@{name=$focused.Current.Name;processId=$focused.Current.ProcessId;geometry=(Read-Geometry $focused);inTree=$inTree;actionable=$actionable;valid=($inTree -and $actionable)}
    } catch { return @{valid=$false;error=$_.Exception.Message} }
}
function Wait-UsableFocus([int]$Seconds) {
    $until = [DateTime]::UtcNow.AddSeconds($Seconds)
    $first = Observe-Focus
    $last = $first
    do {
        if ($last.valid) { break }
        Start-Sleep -Milliseconds 150
        $last = Observe-Focus
    } while ([DateTime]::UtcNow -lt $until)
    return @{first=$first;final=$last;valid=$last.valid}
}
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
    # Startup can initially show an empty new thread before navigation restores.
    # Require the original conversation and exact draft before continuing.
    [void](Wait-Name 'Pi · Git preview fixture · Conversation')
    $composer = Wait-Control {
        param($c)
        $pattern = $null
        $c.Current.Name -ceq 'Conversation messages' -and
            $c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern) -and
            $pattern.Current.Value -ceq $Draft
    } 'original conversation draft'
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
        # Scroll the drawer with the patch still expanded, outside its nested
        # patch viewport. Keep every observed position and focus for review.
        $cancel = Wait-Name 'Cancel'
        $result.stages.expandedCancelBefore = Read-Geometry $cancel
        $result.stages.focusBeforeScroll = Read-Focus
        $wheelAnchor = Wait-Name 'Staged changes in the entire repository'
        $anchor = $wheelAnchor.Current.BoundingRectangle
        if ($anchor.IsEmpty -or $wheelAnchor.Current.IsOffscreen) { throw 'Patch toggle is not visible for drawer scrolling.' }
        $wheelX = [int]($anchor.X - 8)
        $wheelY = [int]($anchor.Y + $anchor.Height / 2)
        if (-not $root.Current.BoundingRectangle.Contains($wheelX, $wheelY)) { throw 'Drawer gutter is outside the window.' }
        if (-not [GitPreviewWheel]::SetCursorPos($wheelX, $wheelY)) { throw 'Could not position mouse for drawer scrolling.' }
        $result.stages.wheelAnchor = @{control=(Read-Geometry $wheelAnchor);x=$wheelX;y=$wheelY}
        $result.stages.wheelSamples = @()
        for ($i = 0; $i -lt 12 -and -not (Visible-InWindow $cancel); $i++) {
            [GitPreviewWheel]::mouse_event(0x0800, 0, 0, -120, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds 200
            $cancel = Wait-Name 'Cancel'
            $result.stages.wheelSamples += ,(Read-Geometry $cancel)
        }
        $result.stages.expandedCancelAfter = Read-Geometry $cancel
        if (-not (Visible-InWindow $cancel)) { throw 'Drawer scroll did not reveal Cancel with the patch expanded.' }
        [void](Wait-Name 'Hide staged patch')
        Invoke-Control $cancel
        [void](Wait-Name 'Commit…')
        $result.stages.focusAfterCancel = Wait-UsableFocus 3
        if (-not $result.stages.focusAfterCancel.valid) {
            # A normal Tab probes whether keyboard navigation can recover.
            if ([NativeFixtureWindow]::GetForegroundWindow() -ne $window) { throw 'Fixture lost foreground before Tab.' }
            [System.Windows.Forms.SendKeys]::SendWait('{TAB}')
            $result.stages.focusAfterTab = Wait-UsableFocus 3
        }
        $result.stages.cancelledExpandedPatch = $true
        if (-not $result.stages.focusAfterCancel.valid -or $result.stages.focusAfterCancel.final.name -cne 'Commit…') {
            throw 'Cancel did not restore focus to the visible Commit action.'
        }
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
