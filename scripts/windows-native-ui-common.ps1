$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class NativeFixtureWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
'@
[void][NativeFixtureWindow]::SetProcessDPIAware()
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

function Save-NativeWindow([string]$Path) {
    $rect = New-Object NativeFixtureWindow+Rect
    if (-not [NativeFixtureWindow]::GetWindowRect($window, [ref]$rect)) { throw 'Window bounds unavailable' }
    $bitmap = [Drawing.Bitmap]::new(($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $dc = $graphics.GetHdc()
        try {
            if (-not [NativeFixtureWindow]::PrintWindow($window, $dc, 2)) { throw 'PrintWindow failed' }
        } finally { $graphics.ReleaseHdc($dc) }
        $bitmap.Save($Path)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
