param([Parameter(Mandatory)][int]$AppProcessId, [Parameter(Mandatory)][string]$Executable,
      [Parameter(Mandatory)][string]$OutputPath, [Parameter(Mandatory)][string]$EvidenceDirectory)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$result = [ordered]@{ status='failed'; stages=[ordered]@{}; ui=@(); processes=@(); error=$null }
$app = $null
$root = $null
$window = [IntPtr]::Zero
$owned = @()
function All-Controls { return @($root.FindAll($scope, [System.Windows.Automation.Condition]::TrueCondition)) }
function Find-Name([string]$name) { return @(All-Controls | Where-Object { $_.Current.Name -ceq $name }) }
function Wait-Name([string]$name, [int]$timeout=30) {
 $until=[DateTime]::UtcNow.AddSeconds($timeout)
 do {
  $items = @(Find-Name $name | Where-Object { $_.Current.IsEnabled })
  if ($items.Count -gt 0) { return $items[0] }
  Start-Sleep -Milliseconds 200
 } while ([DateTime]::UtcNow -lt $until)
 throw "UIA control unavailable: $name"
}
function In-Window($control) {
 $bounds = $control.Current.BoundingRectangle
 return (-not $control.Current.IsOffscreen -and -not $bounds.IsEmpty -and
  $root.Current.BoundingRectangle.Contains($bounds.X + $bounds.Width / 2, $bounds.Y + $bounds.Height / 2))
}
function Invoke-Control($control) {
 if (-not (In-Window $control)) {
  $control.SetFocus()
  Start-Sleep -Milliseconds 300
 }
 if (-not (In-Window $control)) { throw "Control outside window: $($control.Current.Name)" }
 $control.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}
function Sample([string]$stage) {
 $samples = @()
 for ($i = 0; $i -lt 5; $i++) {
  $app.Refresh()
  if ($app.HasExited) { throw 'Desktop exited during sampling' }
  $samples += @{
   timestamp = [DateTime]::UtcNow.ToString('o')
   workingSetBytes = $app.WorkingSet64
   privateBytes = $app.PrivateMemorySize64
  }
  Start-Sleep -Milliseconds 500
 }
 $working = @($samples | ForEach-Object { $_.workingSetBytes } | Sort-Object)
 $private = @($samples | ForEach-Object { $_.privateBytes } | Sort-Object)
 $result.stages[$stage]=@{ samples=$samples; medianWorkingSetBytes=$working[2]; medianPrivateBytes=$private[2] }
}
function Observe-Image([string]$marker,[string]$dimensions) {
 $until = [DateTime]::UtcNow.AddSeconds(30)
 $button = $null
 do {
  $label=Wait-Name $marker
  $walker=[System.Windows.Automation.TreeWalker]::ControlViewWalker
  $node=$label
  for ($i=0;$i -lt 6 -and $null -ne $node;$i++) {
   $candidate=@($node.FindAll($scope, [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -ceq 'View image 1' -and (In-Window $_) })
   if ($candidate.Count -eq 1) {
    $button = $candidate[0]
    break
   }
   $node = $walker.GetParent($node)
  }
  if ($null -eq $button) { Start-Sleep -Milliseconds 200 }
 } while ($null -eq $button -and [DateTime]::UtcNow -lt $until)
 if ($null -eq $button) { throw "Could not identify visible image belonging to $marker" }
 Invoke-Control $button
 [void](Wait-Name $dimensions)
 $result.ui += @{ marker=$marker; imageButton='View image 1'; dimensions=$dimensions }
 [void][ImageFixtureWindow]::SetForegroundWindow($window)
 [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
 Start-Sleep -Milliseconds 300
}
function Search-First {
 Invoke-Control (Wait-Name 'Find in current conversation')
 $input = Wait-Name 'Find messages and tools in this conversation'
 $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('EARLY_HISTORY_IMAGE_01')
 [void](Wait-Name 'EARLY_HISTORY_IMAGE_01')
}
function Snapshot-Failure {
 if ($window -eq [IntPtr]::Zero) { return }
 try {
  $rect = New-Object ImageFixtureWindow+Rect
  if (-not [ImageFixtureWindow]::GetWindowRect($window, [ref]$rect)) { return }
  $img = New-Object System.Drawing.Bitmap ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
  $graphics = [System.Drawing.Graphics]::FromImage($img)
  try {
   $dc = $graphics.GetHdc()
   try {
    [void][ImageFixtureWindow]::PrintWindow($window, $dc, 2)
   } finally {
    $graphics.ReleaseHdc($dc)
   }
   $img.Save((Join-Path $EvidenceDirectory 'windows-image-failure.png'))
  } finally {
   $graphics.Dispose()
   $img.Dispose()
  }
  $result.failureControls = @(All-Controls | ForEach-Object { $_.Current.Name } |
   Where-Object { $_ } | Select-Object -First 200)
 } catch {
  $result.failureCaptureError = "$_"
 }
}
try {
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName UIAutomationClient
$scope = [System.Windows.Automation.TreeScope]::Descendants
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$result.dependencies = @{
 powerShellVersion = $PSVersionTable.PSVersion.ToString()
 treeScopeAssembly = [System.Windows.Automation.TreeScope].Assembly.FullName
 automationElementAssembly = [System.Windows.Automation.AutomationElement].Assembly.FullName
}
$expectedDimensions = "848 $([char]0x00d7) 569"
if ('848 × 569' -cne $expectedDimensions) {
 throw 'PowerShell did not decode the image dimension text as UTF-8'
}
$result.dependencies.dimensionTextDecoded = $true
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ImageFixtureWindow {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, UIntPtr extra);
}
'@
 $app = Get-Process -Id $AppProcessId
 if ($app.Path -ine $Executable) { throw 'Unexpected desktop executable path' }
 $until=[DateTime]::UtcNow.AddSeconds(40)
 do {
  $app.Refresh()
  if ($app.HasExited) { throw 'Desktop exited before showing window' }
  $window = $app.MainWindowHandle
  if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
  Start-Sleep -Milliseconds 200
 } while ([DateTime]::UtcNow -lt $until)
 if ($window -eq [IntPtr]::Zero) { throw 'No Hive window' }
 $root=[System.Windows.Automation.AutomationElement]::FromHandle($window)
 Observe-Image 'LATEST_HISTORY_IMAGE_48' '992 × 684'
 Sample 'coldTail'
 Search-First
 Observe-Image 'EARLY_HISTORY_IMAGE_01' '848 × 569'
 Sample 'early'
 # Exercise actual scrolling in the conversation viewport before returning to the tail.
 $bounds = $root.Current.BoundingRectangle
 [void][ImageFixtureWindow]::SetForegroundWindow($window)
 [void][ImageFixtureWindow]::SetCursorPos([int]($bounds.X+$bounds.Width*0.7),[int]($bounds.Y+$bounds.Height*0.5))
 [ImageFixtureWindow]::mouse_event(0x0800,0,0,-480,[UIntPtr]::Zero)
 Start-Sleep -Milliseconds 800
 if (@(Find-Name 'EARLY_HISTORY_IMAGE_01' | Where-Object { In-Window $_ }).Count -gt 0) { throw 'Scroll did not move the first image out of the viewport' }
 $result.ui += @{ scroll='wheel down'; firstImageLeftViewport=$true }
 Sample 'scrolled'
 # Navigate back to the tail after loading the first image.
 Invoke-Control (Wait-Name 'Jump to latest message')
 Observe-Image 'LATEST_HISTORY_IMAGE_48' '992 × 684'
 Sample 'returnedTail'
 $emptyName = 'Pi · Empty conversation · Conversation'
 $empty = @(Find-Name $emptyName | Where-Object {
  $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button
 })
 if ($empty.Count -ne 1) { throw "Expected one empty-conversation button, found $($empty.Count)" }
 Invoke-Control $empty[0]
 $until=[DateTime]::UtcNow.AddSeconds(20)
 do {
  $tail = @(Find-Name 'LATEST_HISTORY_IMAGE_48')
  if ($tail.Count -eq 0) { break }
  Start-Sleep -Milliseconds 200
 } while ([DateTime]::UtcNow -lt $until)
 if ($tail.Count -ne 0) { throw 'Photo conversation remained visible after switching' }
 [void](Wait-Name 'Current conversation actions: Empty conversation')
 $result.ui += @{ selectedConversation = 'Empty conversation' }
 Sample 'emptyConversation'
 # Remember only processes descended from this fixture, then verify they exit.
 $all = @(Get-CimInstance Win32_Process)
 $ids = @($AppProcessId)
 do {
  $before = $ids.Count
  $ids += @($all | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids } |
   ForEach-Object { [int]$_.ProcessId })
 } while ($ids.Count -gt $before)
 $owned = @($all | Where-Object { $_.ProcessId -in $ids })
 $result.processes = @($ids)
 if (-not $app.CloseMainWindow()) { throw 'Could not close fixture desktop' }
 $until=[DateTime]::UtcNow.AddSeconds(25)
 do {
  $alive = @($ids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
  if ($alive.Count -eq 0) { break }
  Start-Sleep -Milliseconds 200
 } while ([DateTime]::UtcNow -lt $until)
 if ($alive.Count -gt 0) { throw "Fixture processes still alive: $alive" }
 $result.processesExited = $true
 $result.status = 'passed'
} catch {
 $result.error = "$_"
 Snapshot-Failure
 # An already-exited desktop cannot be used with taskkill /T. Reap only
 # previously observed descendants whose process creation time still matches.
 foreach ($item in $owned) {
  $current=Get-CimInstance Win32_Process -Filter "ProcessId=$($item.ProcessId)" -ErrorAction SilentlyContinue
  if ($null -ne $current -and $current.CreationDate -eq $item.CreationDate) { Stop-Process -Id $item.ProcessId -Force -ErrorAction SilentlyContinue }
 }
 Write-Error $_
}
finally { [System.IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 12), [System.Text.UTF8Encoding]::new($false)) }
