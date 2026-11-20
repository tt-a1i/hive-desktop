param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select','reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ModeFixturePointer {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, UIntPtr extra);
}
'@

$result = [ordered]@{ status = 'failed'; phase = $Phase; checks = @(); error = $null; controls = @() }
$root = $null
$separator = ' ' + [char]0x00B7 + ' '
$codexTitle = 'Codex mode QA'
$acpTitle = 'ACP mode QA'
$piTitle = 'Pi mode QA'
$modelPicker = 'Select conversation model and reasoning effort'

function Open-Conversation([string]$Backend, [string]$Title) {
    $prefix = "$Backend$separator$Title$separator"
    Invoke-Control (Wait-Control { param($c) $c.Current.Name.StartsWith($prefix, [StringComparison]::Ordinal) } $Title)
    [void](Wait-Name "Current conversation actions: $Title")
}

function Assert-Draft([string]$Text) {
    [void](Wait-Control {
        param($c)
        $value = $null
        $c.Current.Name -ceq 'Conversation messages' -and
          $c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$value) -and
          $value.Current.Value -ceq $Text
    } "composer draft '$Text'")
}

function Assert-NoNames([string]$Pattern) {
    $unexpected = @(All-Controls | Where-Object { $_.Current.Name -match $Pattern })
    if ($unexpected.Count) { throw "Unexpected mode controls: $($unexpected.Current.Name -join ', ')" }
}

function Click-ModeControl($Control) {
    # DropdownMenu opens from the native pointer handler. Locate both the
    # trigger and the menu item by their UI Automation names.
    $bounds = $Control.Current.BoundingRectangle
    $x = [int]($bounds.X + $bounds.Width / 2)
    $y = [int]($bounds.Y + $bounds.Height / 2)
    if ($Control.Current.IsOffscreen -or $bounds.IsEmpty -or
        -not $root.Current.BoundingRectangle.Contains([double]$x, [double]$y) -or
        [NativeFixtureWindow]::GetForegroundWindow() -ne $window) {
        throw "Mode control is outside the focused fixture: $($Control.Current.Name)"
    }
    if (-not [ModeFixturePointer]::SetCursorPos($x, $y)) { throw 'Could not position the fixture pointer.' }
    [ModeFixturePointer]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [ModeFixturePointer]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
}

function Set-Mode([string]$Axis, [string]$Current, [string]$Option, [string]$Other) {
    Click-ModeControl (Wait-Name "$Axis. $Current")
    $menu = Wait-Control {
        param($c)
        $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Menu
    } "$Axis option list"
    $interactiveTypes = @(
        [System.Windows.Automation.ControlType]::MenuItem,
        [System.Windows.Automation.ControlType]::CheckBox,
        [System.Windows.Automation.ControlType]::RadioButton
    )
    $items = @($menu.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition) | Where-Object {
            $_.Current.ControlType -in $interactiveTypes
        })
    $names = @($items | ForEach-Object { $_.Current.Name })
    if ($names.Count -ne 2 -or $Option -cnotin $names -or $Other -cnotin $names) {
        throw "Unexpected options for $Axis ($($names -join ', '))"
    }
    $result.checks += "$Axis offers exactly $($names -join ', ')"
    $choice = $items | Where-Object { $_.Current.Name -ceq $Option }
    if (-not $choice.Current.IsEnabled) { throw "Mode choice is disabled: $Option" }
    Click-ModeControl $choice
}

function Assert-Codex([string]$Mode, [string]$Permission) {
    [void](Wait-Name 'Current conversation actions: Codex mode QA')
    [void](Wait-Name "Conversation mode. $Mode")
    [void](Wait-Name "Codex access permissions. $Permission")
    Assert-NoNames '^ACP access permissions|^Connect ACP to choose a mode$'
    [void](Wait-Control {
        param($c)
        ([string]$c.Current.Name).TrimEnd("`r", "`n") -ceq 'Stored Codex mode history 49A'
    } 'Stored Codex mode history 49A')
    Assert-Draft 'Unsent Codex mode draft 49B'
}

function Assert-Pi {
    Open-Conversation 'Pi' $piTitle
    Assert-Draft 'Separate Pi draft 49C'
    Assert-NoNames '^Conversation mode\.|^(Codex|ACP) access permissions|^Connect ACP to choose a mode$'
    Click-ModeControl (Wait-Name $modelPicker)
    [void](Wait-Name 'This backend does not support conversation modes.')
    Assert-NoNames '^Conversation mode[:.]|^(Codex|ACP) access permissions|^Connect ACP to choose a mode$|^Stored Codex mode history 49A[\r\n]*$'
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    $result.checks += 'Pi reports no native mode and shows no permission control or Codex history'
}

try {
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) { throw 'Unexpected fixture executable.' }
    $end = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited before opening.' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Fixture window unavailable.' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus fixture window.' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)

    $codexMode = "Next turn: Plan${separator}Active now: Default collaboration"
    $codexPermission = "Next turn: Workspace write${separator}Active now: Read only"
    if ($Phase -eq 'select') {
        Assert-Codex 'Active now: Default collaboration' 'Active now: Read only'
        Set-Mode 'Conversation mode' 'Active now: Default collaboration' 'Plan' 'Default collaboration'
        Assert-Codex $codexMode 'Active now: Read only'
        Set-Mode 'Codex access permissions' 'Active now: Read only' 'Workspace write' 'Read only'
    } else {
        $codexPermission = 'Active now: Workspace write'
    }
    Assert-Codex $codexMode $codexPermission
    $result.checks += 'Codex shows independent collaboration and permission state with the original draft and history'

    Assert-Pi
    Open-Conversation 'ACP' $acpTitle
    Invoke-Control (Wait-Name 'Connect ACP to choose a mode')
    if ($Phase -eq 'select') {
        [void](Wait-Name 'Conversation mode. Active now: Peer Build')
        [void](Wait-Name 'ACP access permissions. Active now: Peer Read')
        Set-Mode 'Conversation mode' 'Active now: Peer Build' 'Peer Plan' 'Peer Build'
        [void](Wait-Name 'Conversation mode. Active now: Peer Plan')
        [void](Wait-Name 'ACP access permissions. Active now: Peer Read')
        Set-Mode 'ACP access permissions' 'Active now: Peer Read' 'Peer Write' 'Peer Read'
    }
    [void](Wait-Name 'Conversation mode. Active now: Peer Plan')
    [void](Wait-Name 'ACP access permissions. Active now: Peer Write')
    Assert-NoNames '^Codex access permissions|^Stored Codex mode history 49A[\r\n]*$'
    $result.checks += 'ACP shows distinct peer-confirmed collaboration and permission options'

    Open-Conversation 'Codex' $codexTitle
    Assert-Codex $codexMode $codexPermission
    $result.checks += 'Switching back restores the original Codex controls, draft and history'
    Save-NativeWindow (Join-Path $EvidenceDirectory "windows-session-modes-$Phase.png")
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit.' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit.' }
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try { $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ }) } catch {}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory "windows-session-modes-$Phase-failure.png") } catch {}
    }
} finally {
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Session modes fixture failed: $($result.error)" }
