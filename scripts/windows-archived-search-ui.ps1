param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Worktree,
    [string]$Scenario = 'archived-search',
    [string]$SessionId = '',
    [string]$OriginAgentId = '',
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
        Start-Sleep -Milliseconds 300
    }
    if (-not (In-Window $Control)) { throw "Control outside window: $($Control.Current.Name)" }
    $Control.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
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
    if ($Scenario -eq 'handoff-range') {
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
        $query = $search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value
        if ($query -cne 'Cerulean Recovery 8DA7') { throw 'Archive search input changed' }
        $deadline = [DateTime]::UtcNow.AddSeconds(25)
        $matchedGroup = $null
        $open = @()
        do {
            $groups = @(All-Controls | Where-Object {
                $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Group -and
                $_.Current.Name.StartsWith('Independent history QA · Archived worktree QA · ', [System.StringComparison]::Ordinal) -and
                $_.Current.Name.Contains('Cerulean Recovery 8DA7')
            })
            if ($groups.Count -gt 1) { throw 'Archived search exposed duplicate matching result groups' }
            if ($groups.Count -eq 1) {
                $actions = $groups[0].FindAll([System.Windows.Automation.TreeScope]::Descendants,
                    [System.Windows.Automation.Condition]::TrueCondition)
                $open = @($actions | Where-Object {
                    $_.Current.Name.StartsWith('Open conversation: Independent history QA · Archived worktree QA · ', [System.StringComparison]::Ordinal) -and
                    $_.Current.Name.Contains('Cerulean Recovery 8DA7') -and $_.Current.IsEnabled -and (In-Window $_)
                })
                if ($open.Count -eq 1 -and (In-Window $groups[0])) {
                    $matchedGroup = $groups[0]
                    break
                }
            }
            Start-Sleep -Milliseconds 200
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($null -eq $matchedGroup) { throw 'Archived result lacks an accessible title, project, body snippet or unique open action' }
        $result.archiveResultName = $matchedGroup.Current.Name
        Save-Window 'windows-archived-search-result.png'
        $result.checks += 'accessible archived result title, project and body snippet'
        Invoke-Control $open[0]
        [void](Wait-Name 'Current conversation actions: Independent history QA')
        $original = Wait-Name 'The durable transcript records Cerulean Recovery 8DA7 after the checkout was moved.'
        if (-not (In-Window $original)) { throw 'Original message is outside the visible window' }
        [void](Wait-Name 'This historical session is read only. Your draft is preserved; sending, resuming and model changes are unavailable.')
        $draft = Wait-Name 'Conversation messages'
        if ($draft.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -cne 'Offline archive draft preserved') { throw 'Draft changed' }
        $send = @(Find-Name 'Send message')
        if ($send.Count -ne 1 -or $send[0].Current.IsEnabled) { throw 'Read-only send control is missing or enabled' }
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
