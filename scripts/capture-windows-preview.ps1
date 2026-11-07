param(
    [Parameter(Mandatory)] [int] $AppProcessId,
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
}
'@
[void][PreviewWindow]::SetProcessDPIAware()
$app = Get-Process -Id $AppProcessId
$window = $app.MainWindowHandle
if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') {
    throw "Could not identify the fixture Hive window (title '$($app.MainWindowTitle)')."
}
$rect = New-Object PreviewWindow+Rect
if (-not [PreviewWindow]::GetWindowRect($window, [ref]$rect)) { throw 'Window bounds unavailable.' }
$image = New-Object System.Drawing.Bitmap ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
try {
    $graphics = [System.Drawing.Graphics]::FromImage($image)
    try {
        $dc = $graphics.GetHdc()
        try {
            if (-not [PreviewWindow]::PrintWindow($window, $dc, 2)) { throw 'PrintWindow failed.' }
        } finally { $graphics.ReleaseHdc($dc) }
    } finally { $graphics.Dispose() }
    [void][System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($OutputPath))
    $image.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Host "Saved fixture window $($image.Width) x $($image.Height)."
} finally { $image.Dispose() }
