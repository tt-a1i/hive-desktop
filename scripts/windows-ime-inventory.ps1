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
 void GetLanguageProfileDescription(); void GetCurrentLanguage(); void ChangeCurrentLanguage();
 void GetLanguageList(out IntPtr languages, out uint count);
 void EnumLanguageProfiles(ushort langid, out IEnumLanguageProfiles profiles);
}
public static class ImeInventory {
 [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int count, [Out] IntPtr[] layouts);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
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
     while ((hr = enumeration.Next(1, out profile, out fetched)) == 0 && fetched == 1)
      entries.Add(new { language = id.ToString("X4"), clsid = profile.clsid.ToString(), category = profile.catid.ToString(), profile = profile.profile.ToString(), active = profile.active != 0 });
     if (hr < 0) Marshal.ThrowExceptionForHR(hr);
    } finally { if (enumeration != null) Marshal.ReleaseComObject(enumeration); }
   }
  } finally { if (languages != IntPtr.Zero) Marshal.FreeCoTaskMem(languages); Marshal.ReleaseComObject(profiles); }
  return new { loadedKeyboardLayouts = layoutIds, tsfLanguages = ids, chineseTsfProfiles = entries };
 }
}
'@
$receipt = [ordered]@{ status='failed'; meaning='Read-only inventory, not an IME interaction test'; windows=$null; desktop=$null; userLanguages=$null; chineseCapabilities=$null; capabilityError=$null; tsf=$null; error=$null }
$form = $null
try {
 $os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber
 $receipt.windows = $os
 $receipt.userLanguages = @(Get-WinUserLanguageList | Select-Object LanguageTag, InputMethodTips)
 try { $receipt.chineseCapabilities = @(Get-WindowsCapability -Online | Where-Object { $_.Name -match '^Language\..*~~~zh-' } | Select-Object Name, State) }
 catch { $receipt.capabilityError = $_.Exception.ToString() }
 $oldForeground = [ImeInventory]::GetForegroundWindow()
 $form = New-Object Windows.Forms.Form
 $form.Text = 'Hive IME inventory'; $form.Show(); $form.Activate(); [Windows.Forms.Application]::DoEvents()
 [void][ImeInventory]::SetForegroundWindow($form.Handle)
 [Windows.Forms.Application]::DoEvents()
 $receipt.desktop = @{ userInteractive=[Environment]::UserInteractive; previousForegroundPresent=($oldForeground -ne [IntPtr]::Zero); fixtureForeground=([ImeInventory]::GetForegroundWindow() -eq $form.Handle) }
 $receipt.tsf = [ImeInventory]::Snapshot()
 $receipt.status = 'passed'
} catch { $receipt.error = $_.Exception.ToString() }
finally {
 if ($form) { $form.Close(); $form.Dispose() }
 if ($oldForeground -ne [IntPtr]::Zero) { [void][ImeInventory]::SetForegroundWindow($oldForeground) }
 [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($ReceiptPath))) | Out-Null
 [IO.File]::WriteAllText($ReceiptPath, ($receipt | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
}
if ($receipt.status -ne 'passed') { throw "IME inventory failed: $($receipt.error)" }
