param(
    [Parameter(Mandatory)] [int] $AppProcessId,
    [Parameter(Mandatory)] [string] $WorkerFile,
    [Parameter(Mandatory)] [string] $WorkerPrompt,
    [Parameter(Mandatory)] [string] $ExistingDraft,
    [Parameter(Mandatory)] [string] $TesterDraft,
    [Parameter(Mandatory)] [string] $TasksFile,
    [Parameter(Mandatory)] [string] $DispatchId,
    [Parameter(Mandatory)] [string] $DataDirectory,
    [Parameter(Mandatory)] [string] $OutputPath
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class PreviewWindow {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll", EntryPoint = "PostMessageW")] public static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("dwmapi.dll")] public static extern int DwmFlush();
    public delegate bool ChildWindowCallback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr window, ChildWindowCallback callback, IntPtr data);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr window, System.Text.StringBuilder name, int size);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    public static IntPtr[] DescendantWindows(IntPtr parent) {
        var children = new System.Collections.Generic.List<IntPtr>();
        EnumChildWindows(parent, (window, data) => { children.Add(window); return children.Count < 64; }, IntPtr.Zero);
        return children.ToArray();
    }
}
'@
[void][PreviewWindow]::SetProcessDPIAware()
$app = Get-Process -Id $AppProcessId
$window = $app.MainWindowHandle
if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') {
    throw "Could not identify the fixture Hive window (title '$($app.MainWindowTitle)')."
}
function Save-FixtureWindow([string] $FilePath, [IntPtr] $Handle = $window) {
    $rect = New-Object PreviewWindow+Rect
    if (-not [PreviewWindow]::GetWindowRect($Handle, [ref]$rect)) { throw 'Window bounds unavailable.' }
    $image = New-Object System.Drawing.Bitmap ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
    $hash = [System.Security.Cryptography.SHA256]::Create()
    try {
        # AccessKit publishes the new controls before the GPU presents them.
        # Let PrintWindow request a repaint, then sample across compositor frames.
        $deadline = [DateTime]::UtcNow.AddSeconds(6)
        $previous = $null
        $stable = 0
        do {
            if ([PreviewWindow]::DwmFlush() -ne 0) { throw 'Could not synchronize with the desktop compositor.' }
            $graphics = [System.Drawing.Graphics]::FromImage($image)
            try {
                $dc = $graphics.GetHdc()
                try {
                    if (-not [PreviewWindow]::PrintWindow($Handle, $dc, 2)) { throw 'PrintWindow failed.' }
                } finally { $graphics.ReleaseHdc($dc) }
            } finally { $graphics.Dispose() }
            $stream = New-Object System.IO.MemoryStream
            try {
                $image.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
                $stream.Position = 0
                $digest = [Convert]::ToBase64String($hash.ComputeHash($stream))
            } finally { $stream.Dispose() }
            if ($digest -eq $previous) { $stable++ } else { $stable = 0 }
            $previous = $digest
            if ($stable -ge 2) {
                [void][System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($FilePath))
                $image.Save($FilePath, [System.Drawing.Imaging.ImageFormat]::Png)
                Write-Host "Saved fixture window $($image.Width) x $($image.Height): $FilePath."
                return
            }
        } while ([DateTime]::UtcNow -lt $deadline)
        throw "Fixture window did not settle for $FilePath."
    } finally { $hash.Dispose(); $image.Dispose() }
}

Add-Type -AssemblyName UIAutomationClient
$root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
function Find-Action([string] $Name) {
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    return $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
function Save-FixtureFailure([string] $Reason) {
    Write-Host "Fixture interaction failed: $Reason"
    foreach ($label in @('外部程序', '选择 外部 Pi 可执行文件', '选择 Codex 可执行文件', '选择 Claude Code 可执行文件', 'Codex 可执行文件路径', '重新读取', '保存设置')) {
        try {
            $control = Find-Action $label
            if ($null -eq $control) { Write-Host "Control '$label': absent"; continue }
            $state = $control.Current
            Write-Host "Control '$label': enabled=$($state.IsEnabled), offscreen=$($state.IsOffscreen), bounds=$($state.BoundingRectangle)"
        } catch { Write-Warning "Could not inspect fixture control '$label': $_" }
    }
    try { Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-native-failure.png') }
    catch { Write-Warning "Could not capture the fixture failure: $_" }
}
function Wait-Action([string] $Name, [int] $TimeoutSeconds = 8) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $action = Find-Action $Name
        if ($null -ne $action -and $action.Current.IsEnabled) { return $action }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Could not reach the fixture action '$Name'."
}
function Wait-ActionsAbsent([string[]] $Names) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $remaining = @($Names | Where-Object { $null -ne (Find-Action $_) })
        if ($remaining.Count -eq 0) { return }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Fixture controls did not disappear: $($remaining -join ', ')."
}

function Test-VisibleControl($Control) {
    $state = $Control.Current
    $bounds = $state.BoundingRectangle
    $windowBounds = $root.Current.BoundingRectangle
    return (-not $state.IsOffscreen -and -not $bounds.IsEmpty -and $bounds.Width -gt 0 -and $bounds.Height -gt 0 -and
        $windowBounds.Contains($bounds.X + $bounds.Width / 2, $bounds.Y + $bounds.Height / 2))
}
function Reveal-Action([string] $Name) {
    $action = Wait-Action $Name
    if (Test-VisibleControl $action) { return $action }
    # GPUI's AX Invoke clicks the node's center. Focus first lets the actual
    # viewport reveal an offscreen action before that coordinate click.
    $action.SetFocus()
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $action = Find-Action $Name
        if ($null -ne $action -and $action.Current.IsEnabled -and (Test-VisibleControl $action)) { return $action }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "The fixture action '$Name' did not enter the window after focus."
}
function Find-MessageQuote([string] $Message) {
    $groupType = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Group)
    $quoteName = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, '引用消息到草稿')
    $textType = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)
    $matches = @()
    foreach ($group in $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $groupType)) {
        if ($group.Current.Name -cne '你') { continue }
        $quotes = $group.FindAll([System.Windows.Automation.TreeScope]::Descendants, $quoteName)
        if ($quotes.Count -eq 0) { continue }
        if ($quotes.Count -ne 1) { throw 'Fixture message group has multiple quote actions.' }
        foreach ($body in $group.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textType)) {
            if ($body.Current.Name -ceq $Message) { $matches += $quotes[0]; continue }
            $pattern = $null
            if ($body.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
                if ($pattern.Current.IsReadOnly -and $pattern.Current.Value -ceq $Message) { $matches += $quotes[0] }
            } elseif ($body.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$pattern)) {
                $range = $pattern.DocumentRange
                if ($range.GetAttributeValue([System.Windows.Automation.TextPattern]::IsReadOnlyAttribute) -eq $true -and
                    ($range.GetText(-1) -replace "`r`n", "`n").TrimEnd("`r", "`n") -ceq $Message) { $matches += $quotes[0] }
            }
        }
    }
    if ($matches.Count -ne 1) { throw "Expected exactly one quote action with matching read-only fixture text; found $($matches.Count)." }
    return $matches[0]
}
function Invoke-MessageQuote([string] $Message) {
    $quote = Find-MessageQuote $Message
    if (-not $quote.Current.IsEnabled) { throw 'Fixture message quote action is disabled.' }
    if (-not (Test-VisibleControl $quote)) {
        $quote.SetFocus()
        $deadline = [DateTime]::UtcNow.AddSeconds(8)
        do {
            $quote = Find-MessageQuote $Message
            if ($quote.Current.IsEnabled -and (Test-VisibleControl $quote)) { break }
            Start-Sleep -Milliseconds 120
        } while ([DateTime]::UtcNow -lt $deadline)
    }
    if (-not (Test-VisibleControl $quote)) { throw 'Fixture message quote action did not become visible.' }
    $quote.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

function Wait-TesterHistoryAction {
    $buttonName = '打开原成员会话: Tester · 已提交: 核对测试入口'
    $condition = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty, $buttonName)),
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Button)))
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $buttons = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if ($buttons.Count -gt 1) { throw "Multiple Tester responsibility actions found: $($buttons.Count)." }
        if ($buttons.Count -eq 1 -and $buttons[0].Current.IsEnabled) { return $buttons[0] }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Could not reach the unique Tester responsibility action '$buttonName'."
}
function Invoke-TesterHistoryAction {
    $action = Wait-TesterHistoryAction
    if (-not (Test-VisibleControl $action)) {
        $action.SetFocus()
        $deadline = [DateTime]::UtcNow.AddSeconds(8)
        do {
            $action = Wait-TesterHistoryAction
            if (Test-VisibleControl $action) { break }
            Start-Sleep -Milliseconds 120
        } while ([DateTime]::UtcNow -lt $deadline)
    }
    if (-not (Test-VisibleControl $action)) { throw 'Tester original session action did not become visible.' }
    $action.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

function Invoke-Action([string] $Name) {
    Write-Host "Invoking fixture control: $Name"
    $action = Reveal-Action $Name
    $pattern = $null
    if (-not $action.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        $expand = $null; $toggle = $null
        $hasExpand = $action.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$expand)
        $hasToggle = $action.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$toggle)
        Write-Host "Fixture action unsupported: name=$Name, type=$($action.Current.ControlType.ProgrammaticName), enabled=$($action.Current.IsEnabled), offscreen=$($action.Current.IsOffscreen), expand=$hasExpand, toggle=$hasToggle"
        throw "Fixture action '$Name' does not support Invoke."
    }
    $pattern.Invoke()
}

function Open-CurrentConversationMenu {
    # The current header is distinct from sidebar row actions. Its initial
    # title is the real team member name, not a test-specific display alias.
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $buttons = @($root.FindAll([System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.Condition]::TrueCondition) | Where-Object {
                $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
                $_.Current.Name.StartsWith('当前会话操作：')
            })
        if ($buttons.Count -eq 1) {
            Invoke-Action $buttons[0].Current.Name
            return
        }
        if ($buttons.Count -gt 1) { throw 'The owned window exposes multiple current conversation headers.' }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The current conversation header menu was not available.'
}

function Toggle-Action([string] $Name) {
    Write-Host "Toggling fixture control: $Name"
    $action = Wait-Action $Name
    $action.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
}

function Wait-EditorText([string] $Name, [string] $Expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $editor = Find-Action $Name
        if ($null -ne $editor) {
            $pattern = $editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
            if (($pattern.Current.Value -replace "`r`n", "`n") -ceq $Expected) { return }
        }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "The editor '$Name' did not match the expected text."
}

function Wait-EnabledState([string] $Name, [bool] $Expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $action = Find-Action $Name
        if ($null -ne $action -and $action.Current.IsEnabled -eq $Expected) { return }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Control '$Name' did not reach enabled=$Expected."
}

function Wait-Appearance([string] $Mode, [string] $Label) {
    $path = Join-Path $DataDirectory 'desktop-appearance.json'
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        if (Test-Path -LiteralPath $path) {
            $saved = ConvertFrom-Json -InputObject (Get-Content -LiteralPath $path -Raw -Encoding UTF8)
            # Theme changes must preserve the current font preference. The
            # application now writes both fields rather than a legacy string.
            if ($saved.mode -ceq $Mode -and $saved.font_size -eq 16) {
                $statesMatch = $true
                foreach ($choice in @('跟随系统', '浅色', '深色')) {
                    $control = Find-Action "外观：$choice"
                    if ($null -eq $control) { $statesMatch = $false; break }
                    $toggle = $control.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
                    $expected = if ($choice -ceq $Label) { [System.Windows.Automation.ToggleState]::On } else { [System.Windows.Automation.ToggleState]::Off }
                    if ($toggle.Current.ToggleState -ne $expected) { $statesMatch = $false; break }
                }
                if ($statesMatch) { return }
            }
        }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Appearance '$Mode' was not persisted and selected in the native window."
}

function Save-ArtifactDiagnostics($Preview, [string] $Phase, [string] $Reason) {
    # Capture the owned WebView window before its finally block closes it. The
    # outer failure handler can only see the parent source viewer afterward.
    $directory = [System.IO.Path]::GetDirectoryName($OutputPath)
    $mode = if ($env:HIVE_WINDOWS_HTML_REOPEN_ONLY -eq '1') { 'cold' } else { 'initial' }
    $diagnosticsDeadline = [DateTime]::UtcNow.AddSeconds(8)
    $rows = New-Object 'System.Collections.Generic.List[object]'
    $nativeWindows = New-Object 'System.Collections.Generic.List[object]'
    function Add-ArtifactAutomationTree($Element, [string] $Source) {
        $queue = New-Object 'System.Collections.Generic.Queue[object]'
        $queue.Enqueue([pscustomobject]@{ Element = $Element; Depth = 0 })
        $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
        while ($queue.Count -gt 0 -and $rows.Count -lt 512 -and [DateTime]::UtcNow -lt $diagnosticsDeadline) {
            $entry = $queue.Dequeue()
            try {
                $current = $entry.Element.Current
                $rows.Add([pscustomobject]@{
                    Source = $Source; Depth = $entry.Depth; Name = $current.Name
                    AutomationId = $current.AutomationId; ControlType = $current.ControlType.ProgrammaticName
                    ProcessId = $current.ProcessId; NativeWindowHandle = $current.NativeWindowHandle
                    Enabled = $current.IsEnabled; Offscreen = $current.IsOffscreen
                    Bounds = $current.BoundingRectangle.ToString()
                })
                if ($entry.Depth -ge 20) { continue }
                $child = $walker.GetFirstChild($entry.Element)
                while ($null -ne $child -and $queue.Count -lt 512 -and [DateTime]::UtcNow -lt $diagnosticsDeadline) {
                    $queue.Enqueue([pscustomobject]@{ Element = $child; Depth = $entry.Depth + 1 })
                    $child = $walker.GetNextSibling($child)
                }
            } catch {
                $rows.Add([pscustomobject]@{ Source = $Source; Error = $_.Exception.Message })
            }
        }
    }
    try {
        $handle = [IntPtr]$Preview.Current.NativeWindowHandle
        if ($handle -ne [IntPtr]::Zero) {
            try { Save-FixtureWindow (Join-Path $directory "windows-html-preview-$mode-$Phase.png") $handle }
            catch { Write-Warning "Could not capture the owned HTML window: $_" }
        }
        $diagnosticsDeadline = [DateTime]::UtcNow.AddSeconds(8)
        Add-ArtifactAutomationTree $Preview 'owned-preview'
        if ($handle -ne [IntPtr]::Zero) {
            # Record native children separately. A native WebView provider can
            # be missing from the GPUI provider's descendant tree; do not change
            # the assertion lookup until Windows evidence establishes that.
            foreach ($childHandle in [PreviewWindow]::DescendantWindows($handle)) {
                if ([DateTime]::UtcNow -ge $diagnosticsDeadline) { break }
                $class = New-Object System.Text.StringBuilder 256
                [void][PreviewWindow]::GetClassName($childHandle, $class, $class.Capacity)
                [uint32]$owner = 0
                [void][PreviewWindow]::GetWindowThreadProcessId($childHandle, [ref]$owner)
                $nativeWindows.Add([pscustomobject]@{ Handle = $childHandle.ToInt64(); Class = $class.ToString(); ProcessId = $owner })
                if ($rows.Count -lt 512 -and [DateTime]::UtcNow -lt $diagnosticsDeadline) {
                    try {
                        $child = [System.Windows.Automation.AutomationElement]::FromHandle($childHandle)
                        Add-ArtifactAutomationTree $child "native-child-$($childHandle.ToInt64())"
                    } catch { $rows.Add([pscustomobject]@{ Source = 'native-child'; Error = $_.Exception.Message }) }
                }
            }
        }
        [pscustomobject]@{
            Mode = $mode; Phase = $Phase; Reason = $Reason; AppProcessId = $AppProcessId
            PreviewName = $Preview.Current.Name; PreviewHandle = $handle.ToInt64()
            NativeChildren = @($nativeWindows.ToArray()); RawAutomation = @($rows.ToArray())
            AutomationLimit = 512
        } | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 (Join-Path $directory "windows-html-preview-$mode-$Phase.json")
    } catch { Write-Warning "Could not preserve the HTML diagnostic evidence: $_" }
}

function Inspect-FixtureHtml {
    [void](Wait-Action '打开关联派单' 20)
    Invoke-Action '打开关联派单'
    Invoke-Action '预览文件: review.html'
    Invoke-Action '交互预览'
    $desktop = [System.Windows.Automation.AutomationElement]::RootElement
    $windowName = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, '隔离 HTML 预览 — review.html')
    $windowOwner = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $AppProcessId)
    $artifactWindow = New-Object System.Windows.Automation.AndCondition($windowName, $windowOwner)
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $preview = $desktop.FindFirst([System.Windows.Automation.TreeScope]::Children, $artifactWindow)
        if ($null -ne $preview) { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($null -eq $preview) { throw 'Could not find the owned HTML preview window.' }
    function Wait-ArtifactControl([string] $Name) {
        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty, $Name)
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        do {
            $element = $preview.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
            if ($null -ne $element) {
                Write-Host "Artifact assertion matched: $Name (process $($element.Current.ProcessId), $($element.Current.ControlType.ProgrammaticName))"
                return $element
            }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        throw "WebView2 did not expose '$Name'."
    }
    try {
        [void](Wait-ArtifactControl 'Artifact isolation verified')
        [void](Wait-ArtifactControl 'Packaged artifact counter 0')
        $fileSelection = Wait-ArtifactControl 'Check host file selection'
        $fileSelection.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        [void](Wait-ArtifactControl 'Host file selection blocked')
        $navigate = Wait-ArtifactControl 'Check blocked navigation'
        $navigate.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        [void](Wait-ArtifactControl 'Artifact retained after navigation')
        $increment = Wait-ArtifactControl 'Increment packaged artifact'
        $increment.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        [void](Wait-ArtifactControl 'Increment handler ran')
        [void](Wait-ArtifactControl 'Packaged artifact counter 1')
        Write-Host 'Opened the actual report in isolated WebView2 and interacted with its HTML.'
        Save-ArtifactDiagnostics $preview 'passed' 'All original isolation and interaction assertions passed'
    } catch {
        Save-ArtifactDiagnostics $preview 'failure' $_.Exception.Message
        throw
    } finally {
        $previewHandle = [IntPtr]$preview.Current.NativeWindowHandle
        if ($previewHandle -eq [IntPtr]::Zero -or -not [PreviewWindow]::PostMessage($previewHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) {
            throw 'Could not close the owned HTML preview window.'
        }
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ($null -eq $desktop.FindFirst([System.Windows.Automation.TreeScope]::Children, $artifactWindow)) { return }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The fixture HTML window remained open after close.'
}

function Close-FixtureMainWindow {
    if (-not [PreviewWindow]::PostMessage($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) {
        throw 'Could not close the fixture main window.'
    }
}

try {
if ($env:HIVE_WINDOWS_HTML_REOPEN_ONLY -eq '1') {
    Inspect-FixtureHtml
    Close-FixtureMainWindow
    return
}

# Only operate on this test app's window and the temporary Reviewer created by the startup check.
Save-FixtureWindow $OutputPath
[void](Wait-Action '打开或收起工作区与会话导航')
if ($null -eq (Find-Action '团队成员')) {
    Invoke-Action '打开或收起工作区与会话导航'
}
Invoke-Action '收起工作区列表'
[void](Wait-Action '展开工作区列表')
if ($null -ne (Find-Action 'Pi · 新会话 · 会话')) { throw 'Collapsed project tree still exposes its chats.' }
Invoke-Action '展开工作区列表'
[void](Wait-Action 'Pi · 新会话 · 会话')
Invoke-Action '设置'
Invoke-Action '外观'
Toggle-Action '外观：深色'
Wait-Appearance 'dark' '深色'
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-theme-dark.png')
Toggle-Action '外观：浅色'
Wait-Appearance 'light' '浅色'
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-theme-light.png')
Toggle-Action '外观：跟随系统'
Wait-Appearance 'system' '跟随系统'
Write-Host 'Persisted all appearance choices and verified their native toggle state.'
Invoke-Action '内置 Pi'
[void](Wait-Action '刷新账号状态')
[void](Wait-Action '打开 Pi 配置目录')
$providerSearch = Wait-Action '搜索 Pi 提供商'
$providerSearch.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('OpenAI')
[void](Wait-Action 'API 配置 · OpenAI')
[void](Wait-Action '订阅登录 · OpenAI')
Invoke-Action '清除搜索'
Wait-EditorText '搜索 Pi 提供商' ''
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-pi-accounts.png')
Write-Host 'Read the embedded Pi account catalog without starting a login.'
Invoke-Action '模型与接口'
Invoke-Action '添加提供商'
$providerId = Wait-Action '提供商 ID'
$providerId.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('windows-provider-draft')
Invoke-Action '外观'
Invoke-Action '模型与接口'
Wait-EditorText '提供商 ID' 'windows-provider-draft'
Invoke-Action '关闭表单'
Invoke-Action '放弃修改'
Wait-ActionsAbsent @('放弃当前表单修改？', '放弃修改')
[void](Wait-Action '添加提供商')
Invoke-Action '外部程序'
$programInput = Wait-Action 'Codex 可执行文件路径'
$programInput.SetFocus()
[void](Wait-Action '选择 Codex 可执行文件')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-program-settings.png')
if ($null -ne (Find-Action '更多工作区功能')) { throw 'Workspace navigation is covering preferences.' }
$codexPath = 'C:\Fixture Programs\codex.exe'
$commandInput = Wait-Action 'Codex 可执行文件路径'
$commandInput.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($codexPath)
Invoke-Action '保存设置'
# The busy action is named '处理中…'. A disabled '保存设置' means the actual
# response has restored that name and the saved inputs are no longer modified.
Wait-EnabledState '保存设置' $false
$savedNoticeName = '设置已保存。后续新建会话使用更新后的配置，已有会话保留固定配置。'
$savedNotice = Wait-Action $savedNoticeName
if (-not (Test-VisibleControl $savedNotice)) { throw 'The saved program settings feedback is outside the native window.' }
$savedNoticeCondition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::NameProperty, $savedNoticeName)
if ($root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $savedNoticeCondition).Count -ne 1) {
    throw 'Program settings save feedback must be announced exactly once.'
}
Invoke-Action '角色模板'
[void](Wait-Action '角色模板名称')
Invoke-Action '外部程序'
Wait-EditorText 'Codex 可执行文件路径' $codexPath
[void](Wait-Action '选择 Codex 可执行文件')
$save = Find-Action '保存设置'
if ($null -eq $save -or $save.Current.IsEnabled) { throw 'Saved paths still appear modified after returning from another section.' }
Invoke-Action '角色模板'
[void](Wait-Action '角色模板名称')
Invoke-Action '关闭设置'
Wait-ActionsAbsent @('关闭设置', '搜索设置')
[void](Wait-Action '聊天消息')
Invoke-Action '团队成员'
[void](Wait-Action '打开 Reviewer 的成员会话')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-team.png')
$offline = "$WorkerFile.offline"
Move-Item -LiteralPath $WorkerFile -Destination $offline
try {
    Invoke-Action '打开 Reviewer 的成员会话'
    [void](Wait-Action '重新打开会话')
    $model = Find-Action '选择会话模型与推理强度'
    if ($null -eq $model) { $model = Find-Action '选择会话模型' }
    if ($null -eq $model -or $model.Current.IsEnabled) { throw 'Unavailable history must disable model selection.' }
} finally {
    Move-Item -LiteralPath $offline -Destination $WorkerFile
}
Invoke-Action '重新打开会话'
[void](Wait-Action '返回团队成员')
[void](Wait-Action '展开工作过程，2 项工具已完成')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-member-session.png')
Invoke-Action '查找当前会话'
$query = Wait-Action '查找当前会话的消息和工具'
$query.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('先核对原生会话')
[void](Wait-Action '收起思考过程，第 1 段，思考过程')
[void](Wait-Action '复制思考过程，第 1 段')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-thinking-find.png')
$query = Wait-Action '查找当前会话的消息和工具'
$query.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('README.md')
[void](Wait-Action '收起 read 工具详情，完成 README.md')
Wait-EnabledState '下一条匹配消息' $true
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-conversation-find.png')
$query = Wait-Action '查找当前会话的消息和工具'
$query.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('no-such-hive-fixture-message')
Wait-EnabledState '下一条匹配消息' $false
Wait-EnabledState '上一条匹配消息' $false
# Reveal the first message again so the group header is in the visible area.
$query = Wait-Action '查找当前会话的消息和工具'
$query.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($WorkerPrompt)
Wait-EnabledState '下一条匹配消息' $true
Invoke-Action '关闭会话查找'
Wait-EditorText '聊天消息' $ExistingDraft
Invoke-Action '收起工作过程，2 项工具已完成'
[void](Wait-Action '展开工作过程，2 项工具已完成')
Write-Host 'Found a hidden tool argument through native controls, rejected an absent query, and preserved the composer.'
Invoke-Action '展开工作过程，2 项工具已完成'
Invoke-Action '展开 read 工具详情，完成 README.md'
[void](Wait-Action '收起 read 工具详情，完成 README.md')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-tool-details.png')
Wait-EditorText '聊天消息' $ExistingDraft
# Revisit the fixture message before locating its own quote action.
Invoke-Action '查找当前会话'
$query = Wait-Action '查找当前会话的消息和工具'
$query.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($WorkerPrompt)
Wait-EnabledState '下一条匹配消息' $true
Invoke-Action '关闭会话查找'
Invoke-MessageQuote $WorkerPrompt
$quoted = "> $WorkerPrompt`n`n$ExistingDraft"
Wait-EditorText '聊天消息' $quoted
Invoke-Action '返回团队成员'
[void](Wait-Action '打开 Reviewer 的成员会话')
Invoke-Action 'Pi · 新会话 · 会话'
Wait-EditorText '聊天消息' ''
Invoke-Action 'Pi · Reviewer · 成员 · 已暂停'
Wait-EditorText '聊天消息' $quoted
[void](Wait-Action '收起 read 工具详情，完成 README.md')
Write-Host 'Verified isolated and restored quoted draft and expanded tool across native sessions.'
Invoke-Action '收起工作过程，2 项工具已完成'
[void](Wait-Action '展开工作过程，2 项工具已完成')
Invoke-Action '打开主会话'
Invoke-Action '打开关联派单'
[void](Wait-Action '在文件资源管理器中显示：review.txt')
[void](Wait-TesterHistoryAction)
Invoke-Action '设置'
Invoke-Action '角色模板'
[void](Wait-Action '角色模板名称')
Invoke-Action '关闭设置'
Wait-ActionsAbsent @('关闭设置', '搜索设置')
[void](Wait-TesterHistoryAction)
Invoke-Action '复制派单编号'
$deadline = [DateTime]::UtcNow.AddSeconds(8)
while ((Get-Clipboard -Raw) -cne $DispatchId) {
    if ([DateTime]::UtcNow -ge $deadline) { throw 'Copied dispatch ID differs from the source report.' }
    Start-Sleep -Milliseconds 120
}
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-related-dispatch.png')
Invoke-TesterHistoryAction
Wait-EditorText '聊天消息' $TesterDraft
Invoke-Action '返回派单详情'
[void](Wait-Action '在文件资源管理器中显示：review.txt')
Invoke-Action '返回来源会话'
[void](Wait-Action '打开关联派单')
Write-Host 'Opened the actual reported dispatch from the orchestrator queue and returned to its chat.'
Invoke-Action '协作概览'
Invoke-Action '编辑任务'
$editor = Wait-Action '任务文档 Markdown'
$editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('My draft')
Wait-EditorText '任务文档 Markdown' 'My draft'
Invoke-Action '设置'
[void](Wait-Action '关闭设置')
Invoke-Action '关闭设置'
Wait-ActionsAbsent @('关闭设置', '搜索设置')
Wait-EditorText '任务文档 Markdown' 'My draft'
[System.IO.File]::WriteAllText($TasksFile, 'Agent update')
Invoke-Action '保存任务'
Invoke-Action '复制最新版'
$deadline = [DateTime]::UtcNow.AddSeconds(8)
while ((Get-Clipboard -Raw) -cne 'Agent update') {
    if ([DateTime]::UtcNow -ge $deadline) { throw 'Latest task preview did not contain the external edit.' }
    Start-Sleep -Milliseconds 120
}
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-task-conflict.png')
Wait-EditorText '任务文档 Markdown' 'My draft'
if ([System.IO.File]::ReadAllText($TasksFile) -cne 'Agent update') { throw 'Conflict replaced the external edit.' }
$editor = Wait-Action '任务文档 Markdown'
$editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('Agent update + My draft')
Wait-EditorText '任务文档 Markdown' 'Agent update + My draft'
Invoke-Action '用当前草稿更新文档'
[void](Wait-Action '编辑任务')
if ([System.IO.File]::ReadAllText($TasksFile) -cne 'Agent update + My draft') { throw 'Reviewed task draft was not saved.' }
Write-Host 'Verified conflict review and saving the reconciled task document in the native editor.'

Invoke-Action '返回会话'
Open-CurrentConversationMenu
Invoke-Action '重命名当前会话'
$titleInput = Wait-Action '会话名称'
$titleInput.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('启动方式核验')
Invoke-Action '保存名称'
Invoke-Action '当前会话操作：启动方式核验'
Invoke-Action '重命名当前会话'
Wait-EditorText '会话名称' '启动方式核验'
Invoke-Action '取消'
Write-Host 'Renamed the lead session without changing its draft or team binding.'

# The sidebar search is a real input; results are confined to its named list.
# Bind both targets to the fixture's actual records before selecting by full identity.
$searchInputName = '搜索会话名称、项目路径或 Agent'
function Read-ChatRecords([string] $Path) {
    # ConvertFrom-Json emits a JSON array as one pipeline object in Windows
    # PowerShell; explicitly enumerate each record before applying filters.
    $records = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($Path))
    foreach ($record in $records) { Write-Output $record }
}
$fixtureRecords = @(Read-ChatRecords (Join-Path $DataDirectory 'chats/index.json'))
$workerRecord = @($fixtureRecords | Where-Object { $_.sessionFile -ceq $WorkerFile })
$leadRecord = @($fixtureRecords | Where-Object { $_.title -ceq '启动方式核验' })
if ($workerRecord.Count -ne 1 -or $leadRecord.Count -ne 1) { throw 'Search targets did not identify exactly one fixture member and lead.' }
if ($workerRecord[0].title -cne 'Reviewer' -or $workerRecord[0].backend -cne 'pi' -or
    [string]::IsNullOrEmpty($workerRecord[0].agentId) -or $workerRecord[0].agentId.EndsWith(':orchestrator') -or
    $workerRecord[0].workspaceId -cne $leadRecord[0].workspaceId -or
    $leadRecord[0].agentId -cne "$($leadRecord[0].workspaceId):orchestrator" -or $leadRecord[0].backend -cne 'pi') {
    throw 'Search targets lost the fixture member/lead identity or backend binding.'
}
$betaPath = [System.IO.Path]::GetDirectoryName([System.IO.Path]::GetDirectoryName([System.IO.Path]::GetFullPath($TasksFile)))
$memberResultName = "Reviewer · Beta · $betaPath · 成员 · Pi"
$leadResultName = "启动方式核验 · Beta · $betaPath · 主控 · Pi"
$leadDraftBeforeSearch = ((Wait-Action '聊天消息').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -replace "`r`n", "`n")
function Wait-UniqueSidebarResult([string] $Name) {
    $listCondition = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '搜索会话')),
        (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::List)))
    $buttonCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $lists = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $listCondition)
        if ($lists.Count -gt 1) { throw 'Sidebar search exposed duplicate result lists.' }
        if ($lists.Count -eq 1) {
            $buttons = $lists[0].FindAll([System.Windows.Automation.TreeScope]::Descendants, $buttonCondition)
            if ($buttons.Count -eq 1 -and $buttons[0].Current.Name -ceq $Name -and $buttons[0].Current.IsEnabled) { return }
        }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Sidebar search did not expose exactly one enabled fixture result '$Name'."
}
$search = Wait-Action $searchInputName
$search.SetFocus()
$search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('Reviewer')
Wait-EditorText $searchInputName 'Reviewer'
Wait-UniqueSidebarResult $memberResultName
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-session-search.png')
Invoke-Action $memberResultName
[void](Wait-Action '当前会话操作：Reviewer')
Wait-EditorText $searchInputName ''
Wait-EditorText '聊天消息' $quoted
$search = Wait-Action $searchInputName
$search.SetFocus()
$search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('启动方式核验')
Wait-EditorText $searchInputName '启动方式核验'
Wait-UniqueSidebarResult $leadResultName
Invoke-Action $leadResultName
[void](Wait-Action '当前会话操作：启动方式核验')
Wait-EditorText $searchInputName ''
Wait-EditorText '聊天消息' $leadDraftBeforeSearch
[void](Wait-Action '打开关联派单')
Write-Host 'Searched member and lead conversations and preserved the isolated member draft.'

$draft = Wait-Action '聊天消息'
$draft.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('后台恢复核验草稿')
$ownedNode = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $AppProcessId" | Where-Object { $_.Name -eq 'node.exe' })
if ($ownedNode.Count -ne 1) { throw 'Could not identify exactly one owned fixture sidecar.' }
$previousNodeId = [int]$ownedNode[0].ProcessId
Stop-Process -Id $previousNodeId -Force
[void](Wait-Action '重新启动后台')
Invoke-Action '重新启动后台'
[void](Wait-Action $searchInputName 20)
Wait-EditorText $searchInputName ''
[void](Wait-Action '打开关联派单' 20)
Wait-EditorText '聊天消息' '后台恢复核验草稿'
$replacement = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $AppProcessId" | Where-Object { $_.Name -eq 'node.exe' })
if ($replacement.Count -ne 1 -or [int]$replacement[0].ProcessId -eq $previousNodeId) { throw 'Restart did not establish one replacement sidecar.' }
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-backend-recovered.png')
Write-Host 'Recovered the original lead session and draft after force-terminating its owned sidecar.'


# Metadata-only actions must preserve the current chat and unsent draft.
$chatIndex = Join-Path $DataDirectory 'chats/index.json'
$pinRecord = @(Read-ChatRecords $chatIndex | Where-Object { $_.title -eq '启动方式核验' })
if ($pinRecord.Count -ne 1) { throw 'Could not identify the named fixture conversation for pinning.' }
$pinId = $pinRecord[0].id
$pinTimestamp = $pinRecord[0].updatedAt
function Wait-SessionPin([bool] $Expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $record = @(Read-ChatRecords $chatIndex | Where-Object { $_.id -eq $pinId })
        if ($record.Count -eq 1 -and [bool]$record[0].pinned -eq $Expected) {
            if ($record[0].updatedAt -ne $pinTimestamp) { throw 'Pinning changed the activity timestamp.' }
            return
        }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Conversation pin state did not persist.'
}
Invoke-Action '启动方式核验 的会话操作'
Invoke-Action '置顶会话'
Wait-SessionPin $true
Wait-EditorText '聊天消息' '后台恢复核验草稿'
[void](Wait-Action '收起置顶会话')
if ($null -ne (Find-Action '收起工作区列表')) { Invoke-Action '收起工作区列表' }
Invoke-Action '启动方式核验 · Beta 的置顶会话操作'
Invoke-Action '取消置顶'
Wait-SessionPin $false
Wait-EditorText '聊天消息' '后台恢复核验草稿'
Write-Host 'Pinned and unpinned the named conversation without changing its timestamp or draft.'

if ($env:HIVE_WINDOWS_HTML_PREVIEW -eq '1') {
    Inspect-FixtureHtml
    Close-FixtureMainWindow
}
} catch {
    Save-FixtureFailure $_.Exception.Message
    throw
}
