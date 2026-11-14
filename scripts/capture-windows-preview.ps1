param(
    [Parameter(Mandatory)] [int] $AppProcessId,
    [Parameter(Mandatory)] [string] $WorkerFile,
    [Parameter(Mandatory)] [string] $WorkerPrompt,
    [Parameter(Mandatory)] [string] $ExistingDraft,
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
    [DllImport("dwmapi.dll")] public static extern int DwmFlush();
}
'@
[void][PreviewWindow]::SetProcessDPIAware()
$app = Get-Process -Id $AppProcessId
$window = $app.MainWindowHandle
if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') {
    throw "Could not identify the fixture Hive window (title '$($app.MainWindowTitle)')."
}
function Save-FixtureWindow([string] $FilePath) {
    $rect = New-Object PreviewWindow+Rect
    if (-not [PreviewWindow]::GetWindowRect($window, [ref]$rect)) { throw 'Window bounds unavailable.' }
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
                    if (-not [PreviewWindow]::PrintWindow($window, $dc, 2)) { throw 'PrintWindow failed.' }
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
    $bodyCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Message)
    $quoteCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, '引用消息到草稿')
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $bodies = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $bodyCondition)
        if ($bodies.Count -gt 1) { throw "Expected exactly one fixture message body; found $($bodies.Count)." }
        if ($bodies.Count -eq 1) { break }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($bodies.Count -ne 1) { throw 'Fixture message body did not appear in the accessibility tree.' }
    $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
    $group = $walker.GetParent($bodies[0])
    while ($null -ne $group -and $group.Current.ControlType -ne [System.Windows.Automation.ControlType]::Group) {
        $group = $walker.GetParent($group)
    }
    if ($null -eq $group) { throw 'Fixture message group is absent.' }
    $quotes = $group.FindAll([System.Windows.Automation.TreeScope]::Descendants, $quoteCondition)
    if ($quotes.Count -ne 1) { throw "Expected exactly one quote action in fixture message; found $($quotes.Count)." }
    return $quotes[0]
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

function Invoke-Action([string] $Name) {
    Write-Host "Invoking fixture control: $Name"
    $action = Reveal-Action $Name
    $action.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
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

try {
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
$savedNoticeName = '设置已保存。新启动的外部进程将使用更新后的配置。'
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
[void](Wait-Action '打开 Tester 的成员会话')
Invoke-Action '设置'
Invoke-Action '角色模板'
Invoke-Action '返回派单详情'
[void](Wait-Action '打开 Tester 的成员会话')
Invoke-Action '复制派单编号'
$deadline = [DateTime]::UtcNow.AddSeconds(8)
while ((Get-Clipboard -Raw) -cne $DispatchId) {
    if ([DateTime]::UtcNow -ge $deadline) { throw 'Copied dispatch ID differs from the source report.' }
    Start-Sleep -Milliseconds 120
}
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-related-dispatch.png')
Invoke-Action '打开 Tester 的成员会话'
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
Invoke-Action '返回协作概览'
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
Invoke-Action '重命名当前会话'
$titleInput = Wait-Action '会话名称'
$titleInput.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('启动方式核验')
Invoke-Action '保存名称'
Invoke-Action '重命名当前会话'
Wait-EditorText '会话名称' '启动方式核验'
Invoke-Action '取消'
Write-Host 'Renamed the lead session without changing its draft or team binding.'

Invoke-Action '搜索会话'
$search = Wait-Action '搜索会话名称、项目或 Agent'
$search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('Reviewer')
[void](Wait-Action '打开 Reviewer · Beta · Pi')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-session-search.png')
Invoke-Action '打开 Reviewer · Beta · Pi'
Wait-EditorText '聊天消息' $quoted
Invoke-Action '搜索会话'
$search = Wait-Action '搜索会话名称、项目或 Agent'
$search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('启动方式核验')
Invoke-Action '打开 启动方式核验 · Beta · Pi'
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
[void](Wait-Action '搜索会话' 20)
[void](Wait-Action '打开关联派单' 20)
Wait-EditorText '聊天消息' '后台恢复核验草稿'
$replacement = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $AppProcessId" | Where-Object { $_.Name -eq 'node.exe' })
if ($replacement.Count -ne 1 -or [int]$replacement[0].ProcessId -eq $previousNodeId) { throw 'Restart did not establish one replacement sidecar.' }
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-backend-recovered.png')
Write-Host 'Recovered the original lead session and draft after force-terminating its owned sidecar.'


# Metadata-only actions must preserve the current chat and unsent draft.
$chatIndex = Join-Path $DataDirectory 'chats/index.json'
$pinRecord = @(Get-Content -Raw $chatIndex | ConvertFrom-Json | Where-Object { $_.title -eq '启动方式核验' })
if ($pinRecord.Count -ne 1) { throw 'Could not identify the named fixture conversation for pinning.' }
$pinId = $pinRecord[0].id
$pinTimestamp = $pinRecord[0].updatedAt
function Wait-SessionPin([bool] $Expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $record = @(Get-Content -Raw $chatIndex | ConvertFrom-Json | Where-Object { $_.id -eq $pinId })
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
} catch {
    Save-FixtureFailure $_.Exception.Message
    throw
}
