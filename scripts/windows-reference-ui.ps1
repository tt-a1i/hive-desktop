param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select', 'reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$Sha256,
    [Parameter(Mandatory)][string]$Worktree,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ReferenceFixtureWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
'@
[void][ReferenceFixtureWindow]::SetProcessDPIAware()
$result = [ordered]@{
    status = 'failed'
    phase = $Phase
    stages = [ordered]@{}
    error = $null
    controls = @()
}
$app = $null
$root = $null

function All-Controls {
    return @($root.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition
    ))
}

function Wait-Control([scriptblock]$Match, [string]$Label, [int]$Timeout = 30) {
    $end = [DateTime]::UtcNow.AddSeconds($Timeout)
    do {
        $items = @(All-Controls | Where-Object { $_.Current.IsEnabled -and (& $Match $_) })
        if ($items.Count -eq 1) { return $items[0] }
        if ($items.Count -gt 1) { throw "Ambiguous control: $Label ($($items.Count))" }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    throw "Control unavailable: $Label"
}

function Wait-Name([string]$Name, [int]$Timeout = 30) {
    return Wait-Control { param($c) $c.Current.Name -ceq $Name } $Name $Timeout
}

function Invoke-Control($Control) {
    $bounds = $Control.Current.BoundingRectangle
    if ($Control.Current.IsOffscreen -or $bounds.IsEmpty -or
        -not $root.Current.BoundingRectangle.Contains(
            $bounds.X + $bounds.Width / 2, $bounds.Y + $bounds.Height / 2
        )) {
        $Control.SetFocus()
        $end = [DateTime]::UtcNow.AddSeconds(5)
        do {
            Start-Sleep -Milliseconds 100
            $bounds = $Control.Current.BoundingRectangle
        } while (($Control.Current.IsOffscreen -or $bounds.IsEmpty -or
            -not $root.Current.BoundingRectangle.Contains(
                $bounds.X + $bounds.Width / 2, $bounds.Y + $bounds.Height / 2
            )) -and [DateTime]::UtcNow -lt $end)
    }
    if ($Control.Current.IsOffscreen -or $bounds.IsEmpty -or
        -not $root.Current.BoundingRectangle.Contains(
            $bounds.X + $bounds.Width / 2, $bounds.Y + $bounds.Height / 2
        )) {
        throw "Control outside window: $($Control.Current.Name)"
    }
    $Control.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

try {
    $app = Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) {
        throw 'Desktop process path differs from the extracted fixture.'
    }
    $end = [DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited early.' }
        $window = $app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') {
        throw 'Fixture window unavailable.'
    }
    if (-not [ReferenceFixtureWindow]::SetForegroundWindow($window)) {
        throw 'Could not focus fixture window.'
    }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    [void](Wait-Name 'Reference project file')
    if ($Phase -eq 'select') {
        Invoke-Control (Wait-Name 'Reference project file')
        [void](Wait-Name ('Isolated worktree execution directory: ' + $Worktree))
        $result.stages.pickerUsesWorktree = $true
        $search = Wait-Name 'Search project files'
        $search.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('shared-reference.txt')
        Invoke-Control (Wait-Name 'Reference project file: shared-reference.txt')
        $result.stages.selected = $true
    }
    $attachment = Wait-Control {
        param($c)
        $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
            $c.Current.Name.StartsWith('Project file snapshot: shared-reference.txt', [StringComparison]::Ordinal)
    } 'snapshot attachment'
    if (-not $attachment.Current.Name.Contains($Sha256)) {
        throw 'Snapshot attachment SHA-256 differs from the worktree bytes.'
    }
    $result.stages.snapshotLabel = $true
    if ($Phase -eq 'reopen') {
        Invoke-Control $attachment
        [void](Wait-Name ('SHA-256: ' + $Sha256))
        [void](Wait-Name 'Source chat: Worktree reference')
        Invoke-Control (Wait-Name 'Check source file')
        [void](Wait-Name 'Source file is missing; the saved snapshot is still available')
        $result.stages.sourceMissing = $true
        Invoke-Control (Wait-Name 'Preview attachment copy')
        [void](Wait-Control { param($c) $c.Current.Name -match 'WORKTREE_SNAPSHOT_CONTENT' } 'snapshot content')
        if (@(All-Controls | Where-Object { $_.Current.Name -match 'MAIN_ROOT_CONTENT' }).Count -ne 0) {
            throw 'Root file content appeared in the snapshot preview.'
        }
        $result.stages.previewWorktreeContent = $true
    }
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal fixture exit.' }
    if (-not $app.WaitForExit(30000) -or $app.ExitCode -ne 0) {
        throw 'Fixture did not exit normally.'
    }
    $result.stages.normalExit = $true
    $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try {
            $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } |
                Where-Object { $_ } | Select-Object -First 150)
        } catch {}
        try {
            $rect = New-Object ReferenceFixtureWindow+Rect
            if ([ReferenceFixtureWindow]::GetWindowRect($window, [ref]$rect)) {
                $bitmap = [Drawing.Bitmap]::new(($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
                $graphics = [Drawing.Graphics]::FromImage($bitmap)
                try {
                    $dc = $graphics.GetHdc()
                    try { [void][ReferenceFixtureWindow]::PrintWindow($window, $dc, 2) }
                    finally { $graphics.ReleaseHdc($dc) }
                    $bitmap.Save((Join-Path $EvidenceDirectory ("windows-project-reference-$Phase-failure.png")))
                } finally {
                    $graphics.Dispose()
                    $bitmap.Dispose()
                }
            }
        } catch { $result.captureError = $_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Reference fixture failed: $($result.error)" }
