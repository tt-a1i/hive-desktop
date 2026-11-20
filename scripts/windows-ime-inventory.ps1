param([Parameter(Mandatory)][string]$ReceiptPath)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Requires a disposable GitHub-hosted runner.' }
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
[StructLayout(LayoutKind.Sequential)]
public struct LanguageProfile {
 public Guid clsid; public ushort langid; public Guid catid; public int active; public Guid profile;
}
[ComImport, Guid("3d61bf11-ac5f-42c8-a4cb-931bcc28c744"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IEnumLanguageProfiles {
 void Clone(out IEnumLanguageProfiles clone);
 [PreserveSig] int Next(uint count, out LanguageProfile profile, out uint fetched);
 void Reset(); void Skip(uint count);
}
[ComImport, Guid("1f02b6c5-7842-4ee6-8a0b-9a24183a95ca"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IProfiles {
 void Register(); void Unregister(); void AddLanguageProfile(); void RemoveLanguageProfile(); void EnumInputProcessorInfo();
 void GetDefaultLanguageProfile(); void SetDefaultLanguageProfile(); void ActivateLanguageProfile(); void GetActiveLanguageProfile();
 [PreserveSig] int GetLanguageProfileDescription(ref Guid clsid, ushort langid, ref Guid profile, [MarshalAs(UnmanagedType.BStr)] out string description);
 void GetCurrentLanguage(); void ChangeCurrentLanguage();
 void GetLanguageList(out IntPtr languages, out uint count);
 void EnumLanguageProfiles(ushort langid, out IEnumLanguageProfiles profiles);
}
[ComImport, Guid("71c6e74c-0f28-11d8-a82a-00065b84435c"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IProfileManager {
 [PreserveSig] int ActivateProfile(uint type, ushort langid, ref Guid clsid, ref Guid profile, IntPtr hkl, uint flags);
 [PreserveSig] int DeactivateProfile(uint type, ushort langid, ref Guid clsid, ref Guid profile, IntPtr hkl, uint flags);
}
public static class ImeInventory {
 [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int count, [Out] IntPtr[] layouts);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
 [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr ActivateKeyboardLayout(IntPtr layout, uint flags);
 [DllImport("imm32.dll")] public static extern IntPtr ImmGetContext(IntPtr window);
 [DllImport("imm32.dll")] public static extern bool ImmReleaseContext(IntPtr window, IntPtr context);
 [DllImport("imm32.dll", CharSet=CharSet.Unicode)] public static extern int ImmGetCompositionStringW(IntPtr context, uint index, IntPtr buffer, uint length);
 public static int CompositionBytes(IntPtr window) {
  var context = ImmGetContext(window); if (context == IntPtr.Zero) return -1;
  try { return ImmGetCompositionStringW(context, 8, IntPtr.Zero, 0); }
  finally { ImmReleaseContext(window, context); }
 }
 public static int ActivateChinese(string clsid, string profile) {
  var manager = (IProfileManager)Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("33c53a50-f456-4884-b049-85fd643ecfed")));
  var serviceId = new Guid(clsid); var profileId = new Guid(profile);
  try { return manager.ActivateProfile(1, 0x0804, ref serviceId, ref profileId, IntPtr.Zero, 0); }
  finally { Marshal.ReleaseComObject(manager); }
 }
 public static object Snapshot() {
  int count = GetKeyboardLayoutList(0, null); if (count <= 0) throw new Exception("Unable to enumerate loaded keyboard layouts");
  var layouts = new IntPtr[count]; if (GetKeyboardLayoutList(count, layouts) != count) throw new Exception("Loaded keyboard layouts changed during enumeration");
  var layoutIds = new List<string>(); foreach (var layout in layouts) layoutIds.Add(layout.ToInt64().ToString("X16"));
  var profiles = (IProfiles)Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("33c53a50-f456-4884-b049-85fd643ecfed")));
  IntPtr languages = IntPtr.Zero; uint languageCount = 0;
  var ids = new List<string>(); var entries = new List<object>();
  try {
   profiles.GetLanguageList(out languages, out languageCount);
   for (int i = 0; i < languageCount; i++) ids.Add(((ushort)Marshal.ReadInt16(languages, i * 2)).ToString("X4"));
   foreach (ushort id in new ushort[] { 0x0404, 0x0804, 0x0c04, 0x1004, 0x1404 }) {
    IEnumLanguageProfiles enumeration = null;
    try {
     profiles.EnumLanguageProfiles(id, out enumeration);
     LanguageProfile profile; uint fetched; int hr;
     while ((hr = enumeration.Next(1, out profile, out fetched)) == 0 && fetched == 1) {
      string description; int descriptionResult = profiles.GetLanguageProfileDescription(ref profile.clsid, id, ref profile.profile, out description);
      entries.Add(new { language = id.ToString("X4"), clsid = profile.clsid.ToString(), category = profile.catid.ToString(),
       keyboardInputProcessor = profile.catid == new Guid("34745c63-b2f0-4784-8b67-5e12c8701a31"),
       profile = profile.profile.ToString(), active = profile.active != 0, description = description,
       descriptionHresult = "0x" + unchecked((uint)descriptionResult).ToString("X8") });
     }
     if (hr < 0) Marshal.ThrowExceptionForHR(hr);
    } finally { if (enumeration != null) Marshal.ReleaseComObject(enumeration); }
   }
  } finally { if (languages != IntPtr.Zero) Marshal.FreeCoTaskMem(languages); Marshal.ReleaseComObject(profiles); }
  return new { loadedKeyboardLayouts = layoutIds, tsfLanguages = ids, chineseTsfProfiles = entries };
 }
}
'@
$receipt = [ordered]@{ status='failed'; meaning='Read-only inventory, not an IME interaction test'; windows=$null; desktop=$null; userLanguages=$null; chineseCapabilities=$null; capabilityError=$null; tsf=$null; control=$null; error=$null }
$form = $null
try {
 $os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber
 $receipt.windows = $os
 $languageList = Get-WinUserLanguageList
 $receipt.userLanguages = @(foreach ($language in $languageList) {
  [pscustomobject]@{ LanguageTag = $language.LanguageTag; InputMethodTips = @($language.InputMethodTips) }
 })
 try { $receipt.chineseCapabilities = @(Get-WindowsCapability -Online | Where-Object { $_.Name -match '^Language\..*~~~zh-' } | Select-Object Name, @{ Name='State'; Expression={ $_.State.ToString() } }) }
 catch { $receipt.capabilityError = $_.Exception.ToString() }
 $oldForeground = [ImeInventory]::GetForegroundWindow()
 $form = New-Object Windows.Forms.Form
 $form.Text = 'Hive IME inventory'; $form.Show(); $form.Activate(); [Windows.Forms.Application]::DoEvents()
 [void][ImeInventory]::SetForegroundWindow($form.Handle)
 [Windows.Forms.Application]::DoEvents()
 $receipt.desktop = @{ userInteractive=[Environment]::UserInteractive; previousForegroundPresent=($oldForeground -ne [IntPtr]::Zero); fixtureForeground=([ImeInventory]::GetForegroundWindow() -eq $form.Handle) }
 $receipt.tsf = [ImeInventory]::Snapshot()
 # Exercise signed HRESULT formatting on Windows PowerShell 5.1.
 $formatCheck = '0x{0:X8}' -f ([int]-2147024809)
 if ($formatCheck -cne '0x80070057') { throw "Unexpected HRESULT format: $formatCheck" }
 # Activate only on this fixture thread. Never enable a profile or change the
 # language list. A successful HRESULT alone is not evidence of composition.
 $oldLayout = [ImeInventory]::GetKeyboardLayout(0)
 $control = [ordered]@{ attempts=@(); hresultFormatCheck=$formatCheck; compositionBytes=$null; restored=$false; activeProfilesRestored=$false; error=$null }
 try {
  $textbox = New-Object Windows.Forms.TextBox
  $form.Controls.Add($textbox); $textbox.Focus() | Out-Null; [Windows.Forms.Application]::DoEvents()
  foreach ($profile in @($receipt.tsf.chineseTsfProfiles | Where-Object { $_.language -eq '0804' -and $_.keyboardInputProcessor })) {
   $hr = [ImeInventory]::ActivateChinese($profile.clsid, $profile.profile)
   $control.attempts += @{ clsid=$profile.clsid; profile=$profile.profile; hresult=('0x{0:X8}' -f $hr) }
   if ($hr -eq 0) {
    if (-not $textbox.Focused -or [ImeInventory]::GetForegroundWindow() -ne $form.Handle) { throw 'Fixture textbox lost foreground focus before input.' }
    [Windows.Forms.SendKeys]::SendWait('ni')
    $end = [DateTime]::UtcNow.AddSeconds(3)
    do { [Windows.Forms.Application]::DoEvents(); $control.compositionBytes = [ImeInventory]::CompositionBytes($textbox.Handle); if ($control.compositionBytes -gt 0) { break }; Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $end)
    break
   }
  }
 } catch { $control.error = $_.Exception.ToString() }
 finally {
  if ([ImeInventory]::GetKeyboardLayout(0) -ne $oldLayout) { [void][ImeInventory]::ActivateKeyboardLayout($oldLayout, 0) }
  $control.restored = [ImeInventory]::GetKeyboardLayout(0) -eq $oldLayout
  $after = [ImeInventory]::Snapshot()
  $beforeActive = @($receipt.tsf.chineseTsfProfiles | Where-Object { $_.active } | ForEach-Object { $_.profile } | Sort-Object) -join ','
  $afterActive = @($after.chineseTsfProfiles | Where-Object { $_.active } | ForEach-Object { $_.profile } | Sort-Object) -join ','
  $control.activeProfilesRestored = $beforeActive -ceq $afterActive
  $receipt.control = $control
 }
 if ($control.error) { throw "IME control failed: $($control.error)" }
 if (-not $control.restored -or -not $control.activeProfilesRestored) { throw 'Fixture input state failed to restore.' }
 $receipt.status = 'passed'
} catch { $receipt.error = $_.Exception.ToString() }
finally {
 if ($form) { $form.Close(); $form.Dispose() }
 if ($oldForeground -ne [IntPtr]::Zero) { [void][ImeInventory]::SetForegroundWindow($oldForeground) }
 [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($ReceiptPath))) | Out-Null
 [IO.File]::WriteAllText($ReceiptPath, ($receipt | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
}
if ($receipt.status -ne 'passed') { throw "IME inventory failed: $($receipt.error)" }
