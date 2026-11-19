# Destructive only to this disposable runner's transient input-layout state.
param([Parameter(Mandatory)][string]$FixtureExe, [Parameter(Mandatory)][string]$ReceiptPath)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Requires a disposable GitHub-hosted runner.' }
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CiKeyboardLayout {
 [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int n,[Out] IntPtr[] list);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint tid);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern IntPtr LoadKeyboardLayoutW(string klid,uint flags);
 [DllImport("user32.dll",SetLastError=true)] public static extern bool UnloadKeyboardLayout(IntPtr layout);
 [DllImport("user32.dll",SetLastError=true)] public static extern IntPtr SendMessageTimeoutW(IntPtr hwnd,uint msg,IntPtr wp,IntPtr lp,uint flags,uint timeout,out IntPtr result);
}
'@
function Get-Layouts {
 $count=[CiKeyboardLayout]::GetKeyboardLayoutList(0,$null)
 if ($count -le 0) { throw 'Could not enumerate loaded layouts.' }
 $items=New-Object 'IntPtr[]' $count
 if ([CiKeyboardLayout]::GetKeyboardLayoutList($count,$items) -ne $count) { throw 'Loaded layout list changed during enumeration.' }
 return ,$items
}
function Layout-Thread([IntPtr]$Window) { [uint32]$proc=0; return [CiKeyboardLayout]::GetWindowThreadProcessId($Window,[ref]$proc) }
function Wait-Layout([uint32]$Thread,[IntPtr]$Layout) {
 $end=[DateTime]::UtcNow.AddSeconds(8)
 do { if ([CiKeyboardLayout]::GetKeyboardLayout($Thread) -eq $Layout) { return }; Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $end)
 throw 'Timed out waiting for target window layout.'
}
function Set-Layout([IntPtr]$Window,[IntPtr]$Layout) {
 if (-not [CiKeyboardLayout]::IsWindow($Window)) { throw 'Layout target window is unavailable.' }
 [IntPtr]$reply=[IntPtr]::Zero
 if ([CiKeyboardLayout]::SendMessageTimeoutW($Window,0x50,[IntPtr]::Zero,$Layout,2,3000,[ref]$reply) -eq [IntPtr]::Zero) { throw 'Layout change request failed or timed out.' }
 Wait-Layout (Layout-Thread $Window) $Layout
}
function Layout-IDs($Layouts) { return (($Layouts | ForEach-Object { $_.ToInt64().ToString('X16') } | Sort-Object) -join ',') }
$receipt=[ordered]@{ status='failed'; error=$null; cleanupErrors=@(); loadedGerman=$false; before=$null; after=$null; uiReceipt=$null }
$form=$null; $german=[IntPtr]::Zero; $before=@(); $oldWindow=[CiKeyboardLayout]::GetForegroundWindow(); $oldThread=0; $oldLayout=[IntPtr]::Zero; $formLayout=[IntPtr]::Zero; $formWindow=[IntPtr]::Zero
$uiReceipt=Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($ReceiptPath))) 'windows-shortcut-layout-ui.json'
try {
 $before=Get-Layouts; $receipt.before=Layout-IDs $before
 if ($oldWindow -ne [IntPtr]::Zero) { $oldThread=Layout-Thread $oldWindow; $oldLayout=[CiKeyboardLayout]::GetKeyboardLayout($oldThread) }
 $us=@($before | Where-Object { ($_.ToInt64() -band 0xffff) -eq 0x0409 } | Select-Object -First 1)
 if ($us.Count -ne 1) { throw 'US input layout must already be loaded on the runner.' }
 $form=[Windows.Forms.Form]::new(); $form.Text='Hive isolated keyboard layout fixture'; $form.Width=400; $form.Height=120; $form.Show(); $form.Activate(); [Windows.Forms.Application]::DoEvents()
 $formWindow=$form.Handle; $formThread=Layout-Thread $formWindow; $formLayout=[CiKeyboardLayout]::GetKeyboardLayout($formThread)
 if (-not [CiKeyboardLayout]::SetForegroundWindow($formWindow)) { throw 'Could not focus fixture-owned layout host.' }
 $end=[DateTime]::UtcNow.AddSeconds(8)
 do { [Windows.Forms.Application]::DoEvents(); if ([CiKeyboardLayout]::GetForegroundWindow() -eq $formWindow) { break }; Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $end)
 if ([CiKeyboardLayout]::GetForegroundWindow() -ne $formWindow) { throw 'Fixture-owned layout host did not receive focus.' }
 $de=@($before | Where-Object { ($_.ToInt64() -band 0xffff) -eq 0x0407 } | Select-Object -First 1)
 if ($de.Count -eq 0) {
   # Windows 8+ loads a layout system-wide. This is permitted only on the
   # disposable hosted runner and is undone below; do not use KLF_ACTIVATE.
   $german=[CiKeyboardLayout]::LoadKeyboardLayoutW('00000407',0x80)
   if ($german -eq [IntPtr]::Zero -or ($german.ToInt64() -band 0xffff) -ne 0x0407) { throw 'Could not load the exact German layout.' }
   $receipt.loadedGerman=$true
 } else { $german=$de[0] }
 $now=Get-Layouts
 if (-not @($now | Where-Object { $_ -eq $german }).Count) { throw 'German layout was not actually loaded.' }
 Set-Layout $formWindow $us[0]
 & (Join-Path $PSScriptRoot 'windows-shortcut-layout-ui.ps1') -FixtureExe $FixtureExe -ReceiptPath $uiReceipt
 $receipt.uiReceipt=Get-Content -Raw -LiteralPath $uiReceipt | ConvertFrom-Json
 if ($receipt.uiReceipt.status -ne 'PASS' -or -not $receipt.uiReceipt.restored) { throw 'Native UI receipt did not pass and restore.' }
 $receipt.status='passed'
} catch { $receipt.error=$_.Exception.ToString() }
finally {
 try { if ($formWindow -ne [IntPtr]::Zero -and [CiKeyboardLayout]::IsWindow($formWindow)) { Set-Layout $formWindow $formLayout } }
 catch { $receipt.cleanupErrors+= "Fixture layout restore: $_" }
 try { if ($form) { $form.Close(); $form.Dispose() } }
 catch { $receipt.cleanupErrors+= "Fixture window close: $_" }
 try {
   if ($oldWindow -ne [IntPtr]::Zero) {
     if (-not [CiKeyboardLayout]::IsWindow($oldWindow)) { throw 'Original foreground window no longer exists.' }
     if ([CiKeyboardLayout]::GetKeyboardLayout($oldThread) -ne $oldLayout) { Set-Layout $oldWindow $oldLayout }
     if ([CiKeyboardLayout]::GetKeyboardLayout($oldThread) -ne $oldLayout) { throw 'Original foreground layout did not restore.' }
     [void][CiKeyboardLayout]::SetForegroundWindow($oldWindow)
   }
 } catch { $receipt.cleanupErrors+= "Original layout restore: $_" }
 try {
   if ($receipt.loadedGerman -and -not [CiKeyboardLayout]::UnloadKeyboardLayout($german)) { throw "Unload failed: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())" }
   $after=Get-Layouts; $receipt.after=Layout-IDs $after
   if ($receipt.after -cne $receipt.before) { throw 'Loaded layout list differs from its original state.' }
 } catch { $receipt.cleanupErrors+= "Loaded layouts: $_" }
 if ($receipt.cleanupErrors.Count) { $receipt.status='failed' }
 try { [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($ReceiptPath))) | Out-Null; [IO.File]::WriteAllText($ReceiptPath,($receipt | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false)) }
 catch { Write-Error "Could not write layout receipt: $_" }
}
if ($receipt.status -ne 'passed') { throw "Windows layout fixture failed: $($receipt.error); cleanup: $($receipt.cleanupErrors -join '; ')" }
