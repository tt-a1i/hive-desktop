param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Worktree,
    [string]$Scenario = 'archived-search',
    [string]$SessionId = '',
    [string]$OriginAgentId = '',
    [string]$Phase = '',
    [string]$StateDirectory = '',
    [string]$WorkspaceId = '',
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$result = [ordered]@{ status='failed'; checks=@(); processes=@(); error=$null }
$root = $null
$window = [IntPtr]::Zero
$owned = @()
function All-Controls {
    return @($root.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition))
}
function Find-Name([string]$Name) {
    return @(All-Controls | Where-Object { $_.Current.Name -ceq $Name })
}
function Wait-Name([string]$Name, [int]$Timeout = 25) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    do {
        $items = @(Find-Name $Name | Where-Object { $_.Current.IsEnabled })
        if ($items.Count -gt 0) { return $items[0] }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "UIA control unavailable: $Name"
}
function Wait-Button([string]$Name) {
    $deadline = [DateTime]::UtcNow.AddSeconds(25)
    do {
        $items = @(Find-Name $Name | Where-Object { $_.Current.IsEnabled -and $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button })
        if ($items.Count -eq 1) { return $items[0] }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Unique button unavailable: $Name"
}
function In-Window($Control) {
    $bounds = $Control.Current.BoundingRectangle
    return (-not $Control.Current.IsOffscreen -and -not $bounds.IsEmpty -and
        $root.Current.BoundingRectangle.Contains($bounds.X + $bounds.Width / 2, $bounds.Y + $bounds.Height / 2))
}
function Invoke-Control($Control) {
    if (-not (In-Window $Control)) {
        $Control.SetFocus()
        $deadline = [DateTime]::UtcNow.AddSeconds(8)
        do {
            if (In-Window $Control) { break }
            Start-Sleep -Milliseconds 120
        } while ([DateTime]::UtcNow -lt $deadline)
    }
    if (-not (In-Window $Control)) { throw "Control outside window: $($Control.Current.Name)" }
    $Control.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}
function Wait-ExactValue([string]$Name, [string]$Expected, [int]$Timeout = 15) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    $last = '<unavailable>'
    do {
        $items = @(Find-Name $Name)
        if ($items.Count -eq 1) {
            $pattern = $null
            if ($items[0].TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
                $last = $pattern.Current.Value -replace "`r`n", "`n"
                if ($last -ceq $Expected) { return $items[0] }
            }
        }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "UIA value for '$Name' did not reach expected text; last value: '$last'."
}
function Wait-VisibleOriginal([string]$Name, [int]$Timeout = 25) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    $count = 0
    do {
        $items = @(Find-Name $Name)
        $count = $items.Count
        if ($count -eq 1 -and (In-Window $items[0])) { return $items[0] }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Unique original message was not visible in the window; matching controls: $count."
}
function Wait-DisabledButton([string]$Name, [int]$Timeout = 15) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    $count = 0
    do {
        $items = @(Find-Name $Name | Where-Object {
            $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button
        })
        $count = $items.Count
        if ($count -eq 1 -and -not $items[0].Current.IsEnabled) { return }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Unique disabled button '$Name' unavailable; matching controls: $count."
}
# Reading-position checks use multiple distinct native text rows and their
# vertical geometry above the composer, not a single long message.
function Reading-Viewport {
    $editors = @(Find-Name 'Conversation messages')
    if ($editors.Count -ne 1) { return @() }
    $bottom = $editors[0].Current.BoundingRectangle.Top
    $windowBounds = $root.Current.BoundingRectangle
    $rows = @()
    foreach ($control in (All-Controls)) {
        $name = $control.Current.Name
        if ($name -cnotmatch '^READING_ROW_([0-9]{2})$') { continue }
        $bounds = $control.Current.BoundingRectangle
        if ($control.Current.IsOffscreen -or $bounds.IsEmpty -or $bounds.Top -lt ($windowBounds.Top + 70) -or
            $bounds.Bottom -gt $bottom -or -not (In-Window $control)) { continue }
        $rows += [ordered]@{ name = $name; y = [math]::Round($bounds.Top - $windowBounds.Top, 1) }
    }
    return @($rows | Sort-Object { $_.y })
}
function Wait-ReadingViewport([int]$Timeout = 25) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    $previous = ''
    do {
        $rows = @(Reading-Viewport)
        $current = ConvertTo-Json -InputObject $rows -Compress -Depth 5
        if ($rows.Count -ge 2 -and $current -ceq $previous) { return $rows }
        $previous = $current
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'At least two stable reading rows did not appear in the viewport'
}
function Wait-ReadingMatch($Expected, [int]$Timeout = 25) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    do {
        $actual = @(Reading-Viewport)
        $pairs = @()
        foreach ($wanted in $Expected) {
            $same = @($actual | Where-Object { $_.name -ceq $wanted.name })
            if ($same.Count -eq 1 -and [math]::Abs($same[0].y - $wanted.y) -le 8) {
                $pairs += [ordered]@{ name = $wanted.name; expectedY = $wanted.y; actualY = $same[0].y }
            }
        }
        if ($pairs.Count -ge 2) { return $pairs }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Fewer than two original reading rows returned to their prior viewport positions'
}
function Wait-ReadingAnchor([int]$Timeout = 20) {
    $path = Join-Path $StateDirectory 'desktop-reading-positions.json'
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    do {
        if (Test-Path -LiteralPath $path) {
            try {
                $entries = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($path))
                $matching = @(foreach ($entry in $entries) {
                    if ($entry.workspace -ceq $WorkspaceId -and $entry.session -ceq $SessionId) { $entry }
                })
                if ($matching.Count -eq 1 -and $matching[0].row.Message -cmatch '^row-([0-9]+)$') {
                    $index = [int]$Matches[1]
                    if ($index -gt 5 -and $index -lt 75 -and $matching[0].offset -ge 0) { return $matching[0] }
                }
            } catch { }
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'A valid middle-of-history anchor was not saved for this session'
}
function Move-ReadingViewport {
    $before = @(Wait-ReadingViewport)
    $target = Wait-VisibleOriginal $before[0].name
    $bounds = $target.Current.BoundingRectangle
    [void][ArchivedFixtureWindow]::SetForegroundWindow($window)
    [void][ArchivedFixtureWindow]::SetCursorPos([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))
    [ArchivedFixtureWindow]::mouse_event(0x0800, 0, 0, -120, [UIntPtr]::Zero)
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $after = @(Reading-Viewport)
        foreach ($row in $before) {
            $same = @($after | Where-Object { $_.name -ceq $row.name })
            if ($same.Count -eq 1 -and [math]::Abs($same[0].y - $row.y) -gt 5) { return }
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The native scroll wheel did not move the conversation viewport'
}
function Wait-ChangedReadingAnchor($Prior, [int]$Timeout = 20) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Timeout)
    do {
        try {
            $anchor = Wait-ReadingAnchor 2
            if ($anchor.row.Message -cne $Prior.row.Message -or
                [math]::Abs($anchor.offset - $Prior.offset) -gt 0.5) { return $anchor }
        } catch { }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The reading position file did not update after the second scroll'
}
function Save-Window([string]$Name) {
    $rect = New-Object ArchivedFixtureWindow+Rect
    if (-not [ArchivedFixtureWindow]::GetWindowRect($window, [ref]$rect)) { throw 'Window bounds unavailable' }
    $bitmap = New-Object System.Drawing.Bitmap ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $dc = $graphics.GetHdc()
        try {
            if (-not [ArchivedFixtureWindow]::PrintWindow($window, $dc, 2)) { throw 'PrintWindow failed' }
        } finally { $graphics.ReleaseHdc($dc) }
        $bitmap.Save((Join-Path $EvidenceDirectory $Name))
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
try {
    Add-Type -AssemblyName UIAutomationTypes
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName System.Drawing
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ArchivedFixtureWindow {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, UIntPtr extra);
}
'@
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine $Executable) { throw 'Unexpected desktop executable' }
    $deadline = [DateTime]::UtcNow.AddSeconds(40)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited before showing a window' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($window -eq [IntPtr]::Zero) { throw 'Hive window unavailable' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    if ($Scenario -eq 'reading-position') {
        $baselinePath = Join-Path ([System.IO.Path]::GetDirectoryName($StateDirectory)) 'reading-baseline.json'
        [void](Wait-Name 'Current conversation actions: Reading target')
        [void](Wait-ExactValue 'Conversation messages' 'Reading position draft preserved')
        if ($Phase -eq 'first') {
            Invoke-Control (Wait-Name 'Find in current conversation')
            $find = Wait-Name 'Find messages and tools in this conversation'
            $find.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('READING_ROW_40')
            [void](Wait-ExactValue 'Find messages and tools in this conversation' 'READING_ROW_40')
            [void](Wait-VisibleOriginal 'READING_ROW_40')
            Invoke-Control (Wait-Name 'Close conversation search')
            # Two distinct wheel movements prove that the stored position was
            # updated after user scrolling, rather than merely pre-seeded.
            Move-ReadingViewport
            $priorAnchor = Wait-ReadingAnchor
            Move-ReadingViewport
            $anchor = Wait-ChangedReadingAnchor $priorAnchor
            # The position file is saved asynchronously. Wait for it to catch
            # up with the settled viewport, and retain both values on failure.
            $deadline = [DateTime]::UtcNow.AddSeconds(20)
            do {
                $baselineRows = @(Wait-ReadingViewport)
                $anchor = Wait-ReadingAnchor
                $result.initialViewport = $baselineRows
                $result.anchor = $anchor
                $anchorIndex = [int]($anchor.row.Message -replace '^row-', '')
                $firstIndex = [int]($baselineRows[0].name -replace '^READING_ROW_', '')
                $lastIndex = [int]($baselineRows[-1].name -replace '^READING_ROW_', '')
                if ($anchorIndex -ge ($firstIndex - 1) -and $anchorIndex -le $lastIndex) { break }
                Start-Sleep -Milliseconds 150
            } while ([DateTime]::UtcNow -lt $deadline)
            if ($anchorIndex -lt ($firstIndex - 1) -or $anchorIndex -gt $lastIndex) {
                throw "Saved anchor $anchorIndex does not belong to visible rows $firstIndex through $lastIndex"
            }
            [System.IO.File]::WriteAllText($baselinePath, (@{ anchor=$anchor; rows=$baselineRows } | ConvertTo-Json -Depth 10), [System.Text.UTF8Encoding]::new($false))
            $result.initialViewport = $baselineRows
            $result.anchor = $anchor
            $result.checks += @('real scroll wheel moved viewport', 'middle message anchor saved')
            Save-Window 'windows-reading-before-switch.png'
            Invoke-Control (Wait-Name 'Pi · Reading other · Conversation')
            [void](Wait-Name 'Current conversation actions: Reading other')
            [void](Wait-VisibleOriginal 'OTHER_CONVERSATION_SENTINEL')
            Invoke-Control (Wait-Name 'Pi · Reading target · Conversation')
            [void](Wait-Name 'Current conversation actions: Reading target')
            $result.switchedBack = @(Wait-ReadingMatch $baselineRows)
            [void](Wait-ExactValue 'Conversation messages' 'Reading position draft preserved')
            $result.checks += @('other conversation opened', 'multiple rows returned after switch', 'draft retained after switch')
            Save-Window 'windows-reading-after-switch.png'
        } elseif ($Phase -eq 'cold') {
            $baseline = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($baselinePath))
            $result.coldViewport = @(Wait-ReadingMatch $baseline.rows)
            [void](Wait-ExactValue 'Conversation messages' 'Reading position draft preserved')
            $result.anchor = Wait-ReadingAnchor
            if ($result.anchor.row.Message -cne $baseline.anchor.row.Message -or
                [math]::Abs($result.anchor.offset - $baseline.anchor.offset) -gt 2) { throw 'Cold reading anchor or offset changed' }
            $result.checks += @('multiple rows returned after cold launch', 'anchor and offset retained', 'draft retained after cold launch')
            Save-Window 'windows-reading-cold.png'
        } else { throw "Unknown reading phase: $Phase" }
    } elseif ($Scenario -eq 'handoff-range') {
        [void](Wait-Name 'Current conversation actions: Handoff source QA')
        $draft = Wait-Name 'Conversation messages'
        if ($draft.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne 'Preserve this handoff draft') { throw 'Initial draft changed' }
        $last = Wait-Name 'SELECTED last question'
        $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
        $group = $last
        $action = $null
        while ($null -ne $group -and $group -ne $root) {
            $matches = @($group.FindAll([System.Windows.Automation.TreeScope]::Descendants,
                (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, 'Continue with another agent…'))))
            if ($matches.Count -eq 1) { $action = $matches[0]; break }
            if ($matches.Count -gt 1) { throw 'Could not associate the last message with a unique handoff action' }
            $group = $walker.GetParent($group)
        }
        if ($null -eq $action) { throw 'Last message handoff action unavailable' }
        Invoke-Control $action
        Invoke-Control (Wait-Button 'SELECTED first statement')
        Invoke-Control (Wait-Button 'Offline target QA · pi · r1')
        Invoke-Control (Wait-Name 'Preview handoff')
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            $names = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
            $source = @($names | Where-Object { $_ -ceq "Source: $SessionId · $OriginAgentId" })
            $before = @($names | Where-Object { $_ -like '*1 persisted item(s) before the selected range were not included*' })
            $after = @($names | Where-Object { $_ -like '*1 persisted item(s) after the selected range were not included*' })
            if ($source.Count -and $before.Count -and $after.Count) { break }
            Start-Sleep -Milliseconds 200
        } while ([DateTime]::UtcNow -lt $deadline)
        if (-not ($source.Count -and $before.Count -and $after.Count)) { throw 'Preview source or selected-range omission text unavailable to UIA' }
        $result.checks += @('source session identity', 'one item omitted before', 'one item omitted after')
        $sourceNode = @(All-Controls | Where-Object { $_.Current.Name -like "Source: $SessionId*" })[0]
        $dialog = $sourceNode
        do {
            $dialog = $walker.GetParent($dialog)
            if ($null -eq $dialog -or $dialog -eq $root) { throw 'Preview dialog subtree unavailable' }
            $controls = @($dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition))
            $dialogNames = @($controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
        } while (-not @($dialogNames | Where-Object { $_ -like '*1 persisted item(s) after the selected range were not included*' }).Count)
        $previewText = @($controls | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text } | ForEach-Object { $_.Current.Name })
        foreach ($text in @('SELECTED first statement', 'SELECTED last question')) {
            if (-not @($previewText | Where-Object { $_ -ceq $text }).Count) { throw "Preview selected text unavailable: $text" }
        }
        # Range-selector buttons may name excluded messages; preview text may not.
        if (@($previewText | Where-Object { $_ -like '*BEFORE excluded private text*' -or $_ -like '*AFTER excluded private text*' }).Count) { throw 'Excluded source text leaked into preview' }
        if (-not @($dialogNames | Where-Object { $_ -like '*Selected persisted range:*first*last*' }).Count) { throw 'Selected range identity unavailable' }
        foreach ($item in @(@('assistant-text', 'first'), @('user-text', 'last'))) {
            $expected = "$($item[0]) · $SessionId · acp · $($item[1]) · $Worktree"
            if (-not @($dialogNames | Where-Object { $_ -ceq $expected }).Count) { throw "Fact source identity unavailable: $expected" }
        }
        $result.checks += @('selected first and last text', 'selected range identities', 'fact source identities', 'excluded text absent')
        Save-Window 'windows-handoff-range-preview.png'
        Add-Type -AssemblyName System.Windows.Forms
        [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
        Start-Sleep -Milliseconds 500
        if (@(Find-Name 'Preview handoff').Count) { throw 'Preview did not close on cancel' }
        $draft = Wait-Name 'Conversation messages'
        if ($draft.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne 'Preserve this handoff draft') { throw 'Draft changed after cancel' }
        [void](Wait-Name 'SELECTED last question')
        $result.checks += @('cancelled preview', 'draft and source history retained')
    } elseif ($Scenario -eq 'archived-search') {
        Invoke-Control (Wait-Name 'Settings')
        Invoke-Control (Wait-Name 'Archive conversation')
        $search = Wait-Name 'Search archived threads…'
        $search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('Cerulean Recovery 8DA7')
        [void](Wait-ExactValue 'Search archived threads…' 'Cerulean Recovery 8DA7')
        $deadline = [DateTime]::UtcNow.AddSeconds(25)
        $matchedGroup = $null
        $open = @()
        do {
            $groups = @(All-Controls | Where-Object {
                $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Group -and
                ([string]$_.Current.Name).StartsWith('Independent history QA · Archived worktree QA · ', [System.StringComparison]::Ordinal) -and
                $_.Current.Name.Contains('Cerulean Recovery 8DA7')
            })
            if ($groups.Count -eq 1) {
                $actions = $groups[0].FindAll([System.Windows.Automation.TreeScope]::Descendants,
                    [System.Windows.Automation.Condition]::TrueCondition)
                $open = @($actions | Where-Object {
                    ([string]$_.Current.Name).StartsWith('Open conversation: Independent history QA · Archived worktree QA · ', [System.StringComparison]::Ordinal) -and
                    $_.Current.Name.Contains('Cerulean Recovery 8DA7') -and $_.Current.IsEnabled -and (In-Window $_)
                })
                if ($open.Count -eq 1 -and (In-Window $groups[0])) {
                    $matchedGroup = $groups[0]
                    break
                }
            }
            Start-Sleep -Milliseconds 200
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($null -eq $matchedGroup) { throw "Archived result lacks a unique accessible title, project, body snippet or open action; matching groups: $($groups.Count)." }
        $result.archiveResultName = $matchedGroup.Current.Name
        Save-Window 'windows-archived-search-result.png'
        $result.checks += 'accessible archived result title, project and body snippet'
        Invoke-Control $open[0]
        [void](Wait-Name 'Current conversation actions: Independent history QA')
        [void](Wait-VisibleOriginal 'The durable transcript records Cerulean Recovery 8DA7 after the checkout was moved.')
        [void](Wait-Name 'This historical session is read only. Your draft is preserved; sending, resuming and model changes are unavailable.')
        [void](Wait-ExactValue 'Conversation messages' 'Offline archive draft preserved')
        Wait-DisabledButton 'Send message'
        if (Test-Path -LiteralPath $Worktree) { throw 'Missing worktree was recreated while the conversation was open' }
        $result.checks += @('original message in viewport', 'read-only notice', 'draft retained', 'send disabled', 'worktree still missing')
        Save-Window 'windows-archived-original-message.png'
    } else { throw "Unknown scenario: $Scenario" }
    $all = @(Get-CimInstance Win32_Process)
    $ids = @($AppProcessId)
    do {
        $before = $ids.Count
        $ids += @($all | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids } |
            ForEach-Object { [int]$_.ProcessId })
    } while ($ids.Count -gt $before)
    $owned = @($all | Where-Object { $_.ProcessId -in $ids })
    $result.processes = @($ids)
    if (-not $app.CloseMainWindow()) { throw 'Could not close fixture window' }
    $deadline = [DateTime]::UtcNow.AddSeconds(25)
    do {
        $alive = @($ids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        if ($alive.Count -eq 0) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($alive.Count -gt 0) { throw "Fixture processes still alive: $alive" }
    $result.processesExited = $true
    $result.status = 'passed'
} catch {
    $result.error = "$_"
    if ($null -ne $root) {
        try { $result.failureControls = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -First 200) }
        catch { $result.failureTreeError = "$_" }
        try { Save-Window 'windows-archived-search-failure.png' }
        catch { $result.failureScreenshotError = "$_" }
    }
    foreach ($item in $owned) {
        $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($item.ProcessId)" -ErrorAction SilentlyContinue
        if ($null -ne $current -and $current.CreationDate -eq $item.CreationDate) { Stop-Process -Id $item.ProcessId -Force -ErrorAction SilentlyContinue }
    }
    Write-Error $_
} finally {
    [System.IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 10), [System.Text.UTF8Encoding]::new($false))
}
