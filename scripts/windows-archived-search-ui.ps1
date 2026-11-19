param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Worktree,
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
    Invoke-Control (Wait-Name 'Settings')
    Invoke-Control (Wait-Name 'Archive conversation')
    $search = Wait-Name 'Search archived threads…'
    $search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('Cerulean Recovery 8DA7')
    [void](Wait-Name 'Independent history QA')
    $deadline = [DateTime]::UtcNow.AddSeconds(25)
    do {
        $snippet = @(All-Controls | Where-Object { $_.Current.Name -like '*Cerulean Recovery 8DA7*' -and (In-Window $_) })
        $open = @(Find-Name 'Open conversation' | Where-Object { $_.Current.IsEnabled -and (In-Window $_) })
        if ($snippet.Count -ge 1 -and $open.Count -eq 1) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($snippet.Count -lt 1 -or $open.Count -ne 1) { throw 'Archived body result or unique open action unavailable' }
    Save-Window 'windows-archived-search-result.png'
    $result.checks += 'archived body search and unique result'
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
