param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('create','retry','reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$FixtureRoot,
    [Parameter(Mandatory)][string]$Draft,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SetupFixturePointer {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, UIntPtr extra);
}
'@
function Click-Control($Control) {
    $inv = $null
    if ($Control.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$inv)) {
        Invoke-Control $Control
        return
    }
    $bounds = $Control.Current.BoundingRectangle
    $x = [int]($bounds.X + $bounds.Width / 2)
    $y = [int]($bounds.Y + $bounds.Height / 2)
    if ($Control.Current.IsOffscreen -or $bounds.IsEmpty -or
        -not $root.Current.BoundingRectangle.Contains([double]$x, [double]$y)) {
        throw "Control outside window: $($Control.Current.Name)"
    }
    if (-not [SetupFixturePointer]::SetCursorPos($x, $y)) { throw 'Could not position pointer.' }
    [SetupFixturePointer]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [SetupFixturePointer]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
}
$result = [ordered]@{ status = 'failed'; phase = $Phase; checks = @(); processes = @(); controls = @(); error = $null }
$root = $null

function Wait-Draft {
    [void](Wait-Control {
        param($control)
        if ($control.Current.Name -cne 'Conversation messages') { return $false }
        $pattern = $null
        return $control.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern) -and
            $pattern.Current.Value -ceq $Draft
    } 'the original conversation draft')
}

function Wait-SetupButton([string]$Name) {
    return Wait-Control {
        param($control)
        $control.Current.Name -ceq $Name -and
            $control.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button
    } $Name
}

function Wait-SendState([bool]$Enabled) {
    $end = [DateTime]::UtcNow.AddSeconds(25)
    do {
        $send = @(All-Controls | Where-Object { $_.Current.Name -ceq 'Send message' -and
            $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and -not $_.Current.IsOffscreen })
        if ($send.Count -eq 1 -and $send[0].Current.IsEnabled -eq $Enabled) { return }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    throw "Send message did not become enabled=$Enabled"
}

function Wait-Setup([string]$Status) {
    return Wait-Control {
        param($control)
        $control.Current.Name -like "Worktree setup: $Status *Windows setup fixture*"
    } "worktree setup status $Status"
}

function Release-OwnedScript([int]$Attempt) {
    $directory = Join-Path (Join-Path $FixtureRoot 'attempts') $Attempt
    $processFile = Join-Path $directory 'process.json'
    $end = [DateTime]::UtcNow.AddSeconds(20)
    $report = $null
    do {
        if (Test-Path -LiteralPath $processFile) {
            try { $report = Get-Content -Raw -LiteralPath $processFile | ConvertFrom-Json } catch {}
        }
        if ($report) { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if (-not $report -or [int]$report.pid -le 0) { throw 'Owned setup process report unavailable' }
    $leaf = Get-CimInstance Win32_Process -Filter "ProcessId=$([int]$report.pid)"
    if (-not $leaf -or $leaf.Name -ine 'node.exe' -or $leaf.ParentProcessId -le 0) { throw 'Setup fixture Node process unavailable' }
    $shell = Get-CimInstance Win32_Process -Filter "ProcessId=$($leaf.ParentProcessId)"
    if (-not $shell -or $shell.Name -ine 'cmd.exe') { throw "The actual parent of the setup process was not cmd.exe (observed $($shell.Name))" }
    $result.processes = @($shell, $leaf) | ForEach-Object {
        [ordered]@{ pid = $_.ProcessId; parentPid = $_.ParentProcessId; image = $_.Name; started = $_.CreationDate.ToUniversalTime().ToString('o') }
    }
    $observed = [ordered]@{ cmdPid = $shell.ProcessId; workerPid = $leaf.ProcessId; started = @($result.processes | ForEach-Object { $_.started }) }
    [IO.File]::WriteAllText((Join-Path $directory 'observed.json'), ($observed | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
    New-Item -ItemType File -Path (Join-Path $directory 'release') | Out-Null
}

function Assert-ScriptExited {
    $end = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $remaining = @($result.processes | Where-Object {
            $instance = Get-CimInstance Win32_Process -Filter "ProcessId=$($_.pid)"
            $instance -and $instance.CreationDate.ToUniversalTime().ToString('o') -ceq $_.started
        })
        if ($remaining.Count -eq 0) { return }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    throw 'An owned process with its original creation time remains alive'
}

try {
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) { throw 'Unexpected desktop executable' }
    $end = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited before showing a window' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Window unavailable' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus window' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    Wait-Draft

    if ($Phase -eq 'create') {
        Click-Control (Wait-Name 'New conversation: choose an Agent')
        Click-Control (Wait-Name 'New isolated worktree conversation…')
        Click-Control (Wait-Name 'Built-in Pi')
        [void](Wait-Name 'Skip setup this time: no')
        Invoke-Control (Wait-SetupButton 'Create isolated worktree conversation')
        [void](Wait-Setup 'Running')
        Wait-Draft
        Wait-SendState $false
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-worktree-setup-running.png')
        $result.checks += 'worktree created from window, running setup disables send and keeps draft'
        Release-OwnedScript 1
        [void](Wait-Setup 'Failed')
        [void](Wait-Name 'Retry original setup script')
        Wait-SendState $false
        Wait-Draft
        Assert-ScriptExited
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-worktree-setup-failed.png')
        $result.checks += 'native failure appeared automatically and blocked send; original script processes exited'
    } elseif ($Phase -eq 'retry') {
        [void](Wait-Setup 'Failed')
        Wait-SendState $false
        Wait-Draft
        if (Test-Path -LiteralPath (Join-Path $FixtureRoot 'attempts\2')) { throw 'Setup reran during cold recovery' }
        Invoke-Control (Wait-Name 'Retry original setup script')
        [void](Wait-Setup 'Running')
        Wait-SendState $false
        Wait-Draft
        Release-OwnedScript 2
        [void](Wait-Setup 'Succeeded')
        Wait-SendState $false
        Wait-Draft
        Assert-ScriptExited
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-worktree-setup-succeeded.png')
        Invoke-Control (Wait-Name 'Open conversation')
        Wait-SendState $true
        Wait-Draft
        $result.checks += 'cold failure did not replay; explicit retry finished; explicit open restored send'
    } else {
        Wait-SendState $true
        Wait-Draft
        $result.checks += 'cold reopen retained original draft in enabled composer'
    }

    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit' }
    $result.checks += 'normal exit observed'
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try {
            $result.controls = @(All-Controls | ForEach-Object {
                [ordered]@{ name = $_.Current.Name; enabled = $_.Current.IsEnabled; offscreen = $_.Current.IsOffscreen }
            } | Where-Object { $_.name } | Select-Object -First 200)
        } catch {}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory "windows-worktree-setup-$Phase-failure.png") } catch { $result.captureError = $_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Worktree setup fixture failed: $($result.error)" }
