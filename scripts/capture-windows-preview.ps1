param(
    [Parameter(Mandatory)] [int] $AppProcessId,
    [Parameter(Mandatory)] [string] $WorkerFile,
    [Parameter(Mandatory)] [string] $WorkerPrompt,
    [Parameter(Mandatory)] [string] $ExistingDraft,
    [Parameter(Mandatory)] [string] $TasksFile,
    [Parameter(Mandatory)] [string] $DispatchId,
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
function Wait-Action([string] $Name, [int] $TimeoutSeconds = 8) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $action = Find-Action $Name
        if ($null -ne $action -and $action.Current.IsEnabled) { return $action }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Could not reach the fixture action '$Name'."
}
function Invoke-Action([string] $Name) {
    Write-Host "Invoking fixture control: $Name"
    $action = Wait-Action $Name
    $action.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
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
Invoke-Action '外部程序'
[void](Wait-Action '选择 Codex 可执行文件')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-program-settings.png')
if ($null -ne (Find-Action '工作区终端')) { throw 'Workspace navigation is covering preferences.' }
$codexPath = 'C:\Fixture Programs\codex.exe'
$commandInput = Wait-Action 'Codex 可执行文件路径'
$commandInput.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($codexPath)
Invoke-Action '保存设置'
Invoke-Action '角色模板'
[void](Wait-Action '角色模板名称')
Invoke-Action '外部程序'
Wait-EditorText 'Codex 可执行文件路径' $codexPath
[void](Wait-Action '选择 Codex 可执行文件')
$save = Find-Action '保存设置'
if ($null -eq $save -or $save.Current.IsEnabled) { throw 'Saved paths still appear modified after returning from another section.' }
Invoke-Action '角色模板'
[void](Wait-Action '角色模板名称')
Invoke-Action '返回会话'
[void](Wait-Action '聊天消息')
Invoke-Action '团队成员'
[void](Wait-Action '打开 Reviewer 的成员会话')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-team.png')
$offline = "$WorkerFile.offline"
Move-Item -LiteralPath $WorkerFile -Destination $offline
try {
    Invoke-Action '打开 Reviewer 的成员会话'
    [void](Wait-Action '重新打开会话')
    $model = Find-Action '选择会话模型'
    if ($null -eq $model -or $model.Current.IsEnabled) { throw 'Unavailable history must disable model selection.' }
} finally {
    Move-Item -LiteralPath $offline -Destination $WorkerFile
}
Invoke-Action '重新打开会话'
[void](Wait-Action '返回团队成员')
[void](Wait-Action '展开工作过程，2 项工具已完成')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-member-session.png')
Invoke-Action '展开工作过程，2 项工具已完成'
Invoke-Action '展开 read 工具详情，完成 README.md'
[void](Wait-Action '收起 read 工具详情，完成 README.md')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-tool-details.png')
Wait-EditorText '聊天消息' $ExistingDraft
Invoke-Action '引用消息到草稿'
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
