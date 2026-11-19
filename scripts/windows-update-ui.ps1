param(
    [Parameter(Mandatory)] [ValidateSet('Install', 'VerifyAndClose', 'Processes', 'StopFixture')] [string] $Action,
    [int] $AppProcessId,
    [string] $Executable,
    [string] $Draft,
    [string] $ReceiptLabel,
    [string] $Directory
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

if ($Action -in @('Processes', 'StopFixture')) {
    $prefix = [System.IO.Path]::GetFullPath($Directory).TrimEnd('\') + '\'
    $owned = @(Get-Process -Name 'hive-desktop', 'node' -ErrorAction SilentlyContinue | Where-Object {
        $_.Path -and $_.Path.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)
    })
    if ($Action -eq 'Processes') {
        $records = @($owned | ForEach-Object {
            [pscustomobject]@{ pid = $_.Id; path = $_.Path; window = ($_.MainWindowHandle -ne [IntPtr]::Zero) }
        })
        ConvertTo-Json -InputObject $records -Compress
        exit
    }
    foreach ($process in $owned) {
        if (-not $process.HasExited) { $process.Kill() }
    }
    exit
}

$app = Get-Process -Id $AppProcessId
if ($app.Path -ine $Executable) { throw 'The window process does not belong to the fixture installation.' }
$deadline = [DateTime]::UtcNow.AddSeconds(20)
do {
    $app.Refresh()
    if ($app.HasExited) { throw 'The fixture desktop exited before showing its window.' }
    $window = $app.MainWindowHandle
    if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
    Start-Sleep -Milliseconds 150
} while ([DateTime]::UtcNow -lt $deadline)
if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'The fixture did not open a Hive window.' }

Add-Type -AssemblyName UIAutomationClient
$root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
$scope = [System.Windows.Automation.TreeScope]::Descendants
function Find-Controls([string] $Name) {
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    return $root.FindAll($scope, $condition)
}
function Wait-Control([string] $Name, [int] $Timeout = 10, [switch] $Last) {
    $end = [DateTime]::UtcNow.AddSeconds($Timeout)
    do {
        $items = @(Find-Controls $Name | Where-Object { $_.Current.IsEnabled })
        if ($items.Count -gt 0) {
            if ($Last) { return $items[-1] }
            return $items[0]
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    # Keep the original assertion and deadline. The modal's accessible state is
    # useful only on failure; successful runs stay quiet.
    try {
        $nodes = @($root.FindAll($scope, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $_.Current.Name } | Select-Object -Last 60 | ForEach-Object {
                [pscustomobject]@{
                    name = $_.Current.Name.Substring(0, [Math]::Min(160, $_.Current.Name.Length))
                    type = $_.Current.ControlType.ProgrammaticName
                    enabled = $_.Current.IsEnabled
                    offscreen = $_.Current.IsOffscreen
                }
            })
        $evidence = [pscustomobject]@{ phase = 'waiting for control'; expected = $Name; processId = $AppProcessId; nodes = $nodes }
        [Console]::Error.WriteLine('[windows-update-uia] ' + (ConvertTo-Json -InputObject $evidence -Depth 4 -Compress))
    } catch {
        [Console]::Error.WriteLine('[windows-update-uia] could not capture controls: ' + $_.Exception.Message)
    }
    throw "The expected control '$Name' did not become available."
}
function Test-InWindow($Control) {
    $bounds = $Control.Current.BoundingRectangle
    return (-not $Control.Current.IsOffscreen -and -not $bounds.IsEmpty -and
        $root.Current.BoundingRectangle.Contains($bounds.X + $bounds.Width / 2, $bounds.Y + $bounds.Height / 2))
}
function Invoke-Control([string] $Name, [int] $Timeout = 10, [switch] $Last) {
    $control = Wait-Control $Name $Timeout -Last:$Last
    if (-not (Test-InWindow $control)) {
        # The AccessKit action targets the rendered control. Focusing an action
        # offscreen first lets GPUI reveal it in its settings scroll view.
        $control.SetFocus()
        $end = [DateTime]::UtcNow.AddSeconds(5)
        do {
            if (Test-InWindow $control) { break }
            Start-Sleep -Milliseconds 125
            $control = Wait-Control $Name -Last:$Last
        } while ([DateTime]::UtcNow -lt $end)
    }
    if (-not (Test-InWindow $control)) { throw "The expected action '$Name' was outside the window." }
    $control.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}
function Wait-Draft {
    $end = [DateTime]::UtcNow.AddSeconds(12)
    do {
        $editor = Wait-Control '聊天消息'
        $value = $editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value
        if ($value -ceq $Draft) { return }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    throw 'The reopened composer did not contain the fixture draft.'
}

Wait-Draft
Invoke-Control '设置'
Invoke-Control '关于与维护'
if ($Action -eq 'Install') {
    Invoke-Control '检查更新'
    Invoke-Control '下载更新' 40
    Invoke-Control '安装并重启' 150
    # The modal exposes its buttons in Windows UIA but its title is not exposed.
    # Require the modal-only button set, so the identically named settings row
    # cannot be mistaken for the confirmation action.
    $end = [DateTime]::UtcNow.AddSeconds(10)
    $confirm = $null
    do {
        $buttons = @($root.FindAll($scope, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
                $_.Current.Name -and $_.Current.IsEnabled -and (Test-InWindow $_) })
        $accept = @($buttons | Where-Object { $_.Current.Name -ceq '安装并重启' })
        $cancel = @($buttons | Where-Object { $_.Current.Name -ceq '取消' })
        if ($buttons.Count -eq 2 -and $accept.Count -eq 1 -and $cancel.Count -eq 1) {
            $confirm = $accept[0]
            break
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($null -eq $confirm) { throw 'The update confirmation dialog did not expose its expected two buttons.' }
    $titleVisible = @((Find-Controls '安装更新并重启？') | Where-Object { $_.Current.IsEnabled -and (Test-InWindow $_) }).Count -gt 0
    [Console]::Out.WriteLine('HIVE_UPDATE_TITLE_ACCESSIBLE=' + $titleVisible.ToString().ToLowerInvariant())
    $confirm.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
} else {
    $end = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $visible = @($root.FindAll($scope, [System.Windows.Automation.Condition]::TrueCondition) |
            Where-Object { $_.Current.Name.StartsWith($ReceiptLabel, [System.StringComparison]::Ordinal) })
        if ($visible.Count -gt 0) { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($visible.Count -eq 0) { throw "The reopened About UI did not display '$ReceiptLabel'." }
    if (-not $app.CloseMainWindow()) { throw 'Could not close the relaunched fixture window.' }
}
