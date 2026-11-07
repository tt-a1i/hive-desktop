param(
    [Parameter(Mandatory)] [int] $AppProcessId,
    [Parameter(Mandatory)] [string] $WorkerFile,
    [Parameter(Mandatory)] [string] $WorkerPrompt,
    [Parameter(Mandatory)] [string] $ExistingDraft,
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
function Wait-Action([string] $Name) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
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

function Wait-ComposerText([string] $Expected) {
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $editor = Find-Action '聊天消息'
        if ($null -ne $editor) {
            $pattern = $editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
            if (($pattern.Current.Value -replace "`r`n", "`n") -ceq $Expected) { return }
        }
        Start-Sleep -Milliseconds 120
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The visible chat draft did not match the expected session text.'
}

# Only operate on this test app's window and the temporary Reviewer created by the startup check.
Save-FixtureWindow $OutputPath
[void](Wait-Action '打开或收起工作区与会话导航')
if ($null -eq (Find-Action '团队成员')) {
    Invoke-Action '打开或收起工作区与会话导航'
}
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
Wait-ComposerText $ExistingDraft
Invoke-Action '引用消息到草稿'
$quoted = "> $WorkerPrompt`n`n$ExistingDraft"
Wait-ComposerText $quoted
Invoke-Action '返回团队成员'
[void](Wait-Action '打开 Reviewer 的成员会话')
Invoke-Action 'Pi · 新会话 · 会话'
Wait-ComposerText ''
Invoke-Action 'Pi · Reviewer · 成员'
Wait-ComposerText $quoted
[void](Wait-Action '收起 read 工具详情，完成 README.md')
Write-Host 'Verified isolated and restored quoted draft and expanded tool across native sessions.'
Invoke-Action '收起工作过程，2 项工具已完成'
[void](Wait-Action '展开工作过程，2 项工具已完成')
Invoke-Action '设置'
Invoke-Action '外部程序'
[void](Wait-Action '选择 Codex 可执行文件')
Save-FixtureWindow (Join-Path ([System.IO.Path]::GetDirectoryName($OutputPath)) 'windows-program-settings.png')
