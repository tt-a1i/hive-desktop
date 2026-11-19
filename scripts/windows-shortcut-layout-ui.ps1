# Run only on an isolated, interactive Windows test desktop.
# Both US and German input layouts must already be loaded. Never installs a layout.
param([Parameter(Mandatory)][string]$FixtureExe,
      [Parameter(Mandatory)][string]$ReceiptPath)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName UIAutomationClient
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class LayoutFixture {
    [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int n, [Out] IntPtr[] list);
    [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SendMessageTimeoutW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll")] public static extern short VkKeyScanExW(char ch, IntPtr layout);
    [DllImport("user32.dll")] public static extern uint MapVirtualKeyExW(uint code, uint type, IntPtr layout);
    [DllImport("user32.dll")] public static extern int ToUnicodeEx(uint vk, uint scan, byte[] state, StringBuilder output, int length, uint flags, IntPtr layout);
    [DllImport("user32.dll")] public static extern short GetAsyncKeyState(int key);
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx,dy; public uint data,flags,time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort vk,scan; public uint flags,time; public UIntPtr extra; }
    [StructLayout(LayoutKind.Explicit)] public struct UNION { [FieldOffset(0)] public MOUSEINPUT mouse; [FieldOffset(0)] public KEYBDINPUT key; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public UNION data; }
    [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint n, INPUT[] input, int size);
    public static string ShiftedSemicolon(IntPtr hkl, out ushort vk) {
        short key = VkKeyScanExW(';', hkl); if (key == -1) { vk=0; return null; }
        vk=(ushort)(key & 255); var state=new byte[256]; state[0x10]=0x80;
        var text=new StringBuilder(8); int n=ToUnicodeEx(vk,MapVirtualKeyExW(vk,0,hkl),state,text,8,4,hkl);
        return n == 1 ? text.ToString() : null;
    }
    public static void Key(ushort vk, bool up) {
        var input=new INPUT { type=1, data=new UNION { key=new KEYBDINPUT { vk=vk, flags=up ? 2u : 0u } } };
        if (SendInput(1,new INPUT[]{input},Marshal.SizeOf(typeof(INPUT))) != 1) throw new System.ComponentModel.Win32Exception();
    }
}
'@
function Thread-Of([IntPtr]$Window) { [uint32]$pidValue=0; $tid=[LayoutFixture]::GetWindowThreadProcessId($Window,[ref]$pidValue); return $tid }
function Wait-Until([scriptblock]$Condition, [string]$Label, [int]$Seconds=12) {
    $end=[DateTime]::UtcNow.AddSeconds($Seconds)
    do { if (& $Condition) { return }; Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $end)
    throw "Timed out: $Label"
}
function Request-Layout([IntPtr]$Window,[IntPtr]$Layout) {
    if (-not [LayoutFixture]::IsWindow($Window)) { throw 'Target window no longer exists.' }
    $tid=Thread-Of $Window; if (-not $tid) { throw 'Target thread unavailable.' }
    [IntPtr]$result=[IntPtr]::Zero
    if ([LayoutFixture]::SendMessageTimeoutW($Window,0x50,[IntPtr]::Zero,$Layout,2,3000,[ref]$result) -eq [IntPtr]::Zero) { throw 'Layout change request failed or timed out.' }
    Wait-Until { [LayoutFixture]::GetKeyboardLayout($tid) -eq $Layout } 'layout change acknowledgement'
}
function Press-Key([IntPtr]$Window,[uint16]$Key,[bool]$Chord) {
    if ([LayoutFixture]::GetForegroundWindow() -ne $Window) { throw 'Fixture lost foreground; no input sent.' }
    foreach ($code in @(0x11,0x10,[int]$Key)) { if (([int][LayoutFixture]::GetAsyncKeyState($code) -band 0x8000) -ne 0) { throw 'A required key is physically held; no input sent.' } }
    $held=[Collections.Generic.List[uint16]]::new()
    try {
        if ($Chord) { foreach ($code in @(0x11,0x10)) { [LayoutFixture]::Key([uint16]$code,$false); $held.Add([uint16]$code) } }
        [LayoutFixture]::Key($Key,$false); $held.Add($Key)
    } finally {
        # Release any keys we injected even if focus changed during the chord.
        for ($i=$held.Count-1; $i -ge 0; $i--) { [LayoutFixture]::Key($held[$i],$true) }
    }
}
$exe=[IO.Path]::GetFullPath($FixtureExe)
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Fixture executable not found.' }
$n=[LayoutFixture]::GetKeyboardLayoutList(0,$null)
$layouts=New-Object 'IntPtr[]' $n
if ($n -lt 2 -or [LayoutFixture]::GetKeyboardLayoutList($n,$layouts) -ne $n) { throw 'SKIP: US and German layouts must already be loaded.' }
$us=@($layouts | Where-Object { ($_.ToInt64() -band 0xffff) -eq 0x0409 } | Select-Object -First 1)
$de=@($layouts | Where-Object { ($_.ToInt64() -band 0xffff) -eq 0x0407 } | Select-Object -First 1)
if ($us.Count -ne 1 -or $de.Count -ne 1) { throw 'SKIP: US and German layouts must already be loaded.' }
[uint16]$vkUs=0; [uint16]$vkDe=0
$charUs=[LayoutFixture]::ShiftedSemicolon($us[0],[ref]$vkUs)
$charDe=[LayoutFixture]::ShiftedSemicolon($de[0],[ref]$vkDe)
if ($charUs -cne ':' -or $charDe -cne ';') { throw "SKIP: actual layouts did not produce expected shifted semicolon mapping (US=$charUs, DE=$charDe)." }
$previous=[LayoutFixture]::GetForegroundWindow()
if ($previous -eq [IntPtr]::Zero) { throw 'No foreground window; interactive desktop required.' }
$previousThread=Thread-Of $previous; $previousLayout=[LayoutFixture]::GetKeyboardLayout($previousThread)
if ($previousLayout -ne $us[0]) { throw 'SKIP: start with the already-loaded US layout active.' }
$dir=Join-Path ([IO.Path]::GetTempPath()) ('hive-layout-fixture-'+[guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($dir)
$data=Join-Path $dir 'data'; $agent=Join-Path $dir 'pi'; [void][IO.Directory]::CreateDirectory($data); [void][IO.Directory]::CreateDirectory($agent)
$config='{"version":1,"bindings":{"search":"ctrl-shift-;","find":null,"preferences":"ctrl-shift-j","new_session":null,"toggle_sidebar":null,"toggle_terminal":null,"open_git_workspace":null,"search_project_files":null,"choose_model":null,"toggle_mode":null,"previous_session":null,"next_session":null}}'
$configPath=Join-Path $data 'desktop-keybindings.json'; [IO.File]::WriteAllText($configPath,$config,[Text.UTF8Encoding]::new($false))
$before=[IO.File]::ReadAllBytes($configPath)
$app=$null; $window=[IntPtr]::Zero; $failure=$null; $restoreFailure=$null; $result=@{us=$charUs; german=$charDe; usVk=$vkUs; germanVk=$vkDe; stage='launch'}
try {
    $oldData=$env:HIVE_DESKTOP_DATA_DIR; $oldAgent=$env:PI_CODING_AGENT_DIR; $oldOffline=$env:PI_OFFLINE
    $oldNode=$env:HIVE_DESKTOP_NODE; $oldRuntime=$env:HIVE_DESKTOP_RUNTIME; $oldOptions=$env:NODE_OPTIONS
    $guard=(Join-Path $PSScriptRoot 'fixtures/windows-image-network-guard.cjs').Replace('\','/')
    try {
        $env:HIVE_DESKTOP_DATA_DIR=$data; $env:PI_CODING_AGENT_DIR=$agent; $env:PI_OFFLINE='1'
        $env:HIVE_DESKTOP_NODE=$null; $env:HIVE_DESKTOP_RUNTIME=$null; $env:NODE_OPTIONS="--require=`"$guard`""
        $app=Start-Process -FilePath $exe -PassThru
    } finally {
        $env:HIVE_DESKTOP_DATA_DIR=$oldData; $env:PI_CODING_AGENT_DIR=$oldAgent; $env:PI_OFFLINE=$oldOffline
        $env:HIVE_DESKTOP_NODE=$oldNode; $env:HIVE_DESKTOP_RUNTIME=$oldRuntime; $env:NODE_OPTIONS=$oldOptions
    }
    Wait-Until { $app.Refresh(); $window=$app.MainWindowHandle; $window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive' } 'fixture window' 30
    $app.Refresh(); $window=$app.MainWindowHandle; $tid=Thread-Of $window
    if ([LayoutFixture]::GetKeyboardLayout($tid) -ne $us[0]) { throw 'SKIP: fixture did not start with the US layout.' }
    if (-not [LayoutFixture]::SetForegroundWindow($window)) { throw 'Could not focus fixture.' }
    Wait-Until { [LayoutFixture]::GetForegroundWindow() -eq $window } 'fixture foreground'
    $root=[System.Windows.Automation.AutomationElement]::FromHandle($window)
    $scope=[System.Windows.Automation.TreeScope]::Descendants
    function Search-Open {
        foreach ($name in @('搜索会话、消息、项目或 Agent…','Search conversations, messages, projects or agents…')) {
            $condition=[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty,$name)
            if ($root.FindAll($scope,$condition).Count -gt 0) { return $true }
        }; return $false
    }
    function Settings-Open {
        foreach ($name in @('关于与维护','About & Maintenance')) {
            $condition=[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty,$name)
            if ($root.FindAll($scope,$condition).Count -gt 0) { return $true }
        }; return $false
    }
    if (Search-Open) { throw 'Search dialog marker was already present; cannot distinguish result.' }
    # Backend startup must have applied the isolated configuration. Retry only the
    # positive US control, never a failed post-switch test.
    $end=[DateTime]::UtcNow.AddSeconds(30); $opened=$false
    do { Press-Key $window $vkUs $true; Start-Sleep -Milliseconds 400; if (Search-Open) { $opened=$true; break } } while ([DateTime]::UtcNow -lt $end)
    if (-not $opened) { throw 'US positive control failed; configuration/fixture not ready.' }
    $result.usControl=$true; $result.stage='switch-to-german'; Press-Key $window 0x1b $false; Wait-Until { -not (Search-Open) } 'close search'
    Request-Layout $window $de[0]
    if (Settings-Open) { throw 'Settings marker was already present before the German control.' }
    $result.stage='german-independent-control'; Press-Key $window 0x4a $true
    Wait-Until { Settings-Open } 'settings shortcut via German layout' 4
    $result.germanIndependentControl=$true
    Press-Key $window 0x1b $false; Wait-Until { -not (Settings-Open) } 'close settings'
    $result.stage='german-control'; Press-Key $window $vkDe $true
    $germanFailure=$null
    try { Wait-Until { Search-Open } 'search via German layout' 4; $result.germanControl=$true }
    catch { $result.germanControl=$false; $germanFailure=$_.Exception.Message }
    if ($result.germanControl) { Press-Key $window 0x1b $false; Wait-Until { -not (Search-Open) } 'close search' }
    $result.stage='switch-back-to-us'; Request-Layout $window $us[0]
    $result.stage='restored-us-control'; Press-Key $window $vkUs $true; Wait-Until { Search-Open } 'search after US restoration' 4
    $result.usRestoredControl=$true
    if ([Convert]::ToBase64String($before) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($configPath))) { throw 'Fixture shortcut file changed unexpectedly.' }
    $result.configUnchanged=$true
    if ($germanFailure) { $result.stage='german-control'; throw $germanFailure }
    $result.stage='completed'; $result.status='PASS'
} catch {
    $failure=$_.Exception.Message; $result.status='FAIL'; $result.error=$failure
    if ($window -ne [IntPtr]::Zero -and [LayoutFixture]::IsWindow($window)) {
        $result.fixtureLayout=[LayoutFixture]::GetKeyboardLayout((Thread-Of $window)).ToInt64().ToString('X16')
        $result.fixtureForeground=([LayoutFixture]::GetForegroundWindow() -eq $window)
    }
}
finally {
    try {
        if ($window -ne [IntPtr]::Zero -and [LayoutFixture]::IsWindow($window)) { Request-Layout $window $us[0] }
        if ($app -and -not $app.HasExited) { [void]$app.CloseMainWindow(); if (-not $app.WaitForExit(10000)) { throw 'Fixture did not close normally.' } }
        if ([LayoutFixture]::IsWindow($previous)) {
            if ([LayoutFixture]::GetKeyboardLayout($previousThread) -ne $previousLayout) { Request-Layout $previous $previousLayout }
            if ([LayoutFixture]::GetKeyboardLayout($previousThread) -ne $previousLayout) { throw 'Prior window layout was not restored.' }
        } else { throw 'Original foreground window closed; its layout could not be verified.' }
        if ([LayoutFixture]::GetForegroundWindow() -eq $window -and [LayoutFixture]::IsWindow($previous)) { [void][LayoutFixture]::SetForegroundWindow($previous) }
        $result.restored=$true
    } catch { $restoreFailure=$_.Exception.Message; $result.restored=$false; $result.restoreError=$restoreFailure; $result.status='FAIL' }
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($ReceiptPath))) | Out-Null
    [IO.File]::WriteAllText($ReceiptPath,($result | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
    if ($result.restored -and (!$app -or $app.HasExited)) { Remove-Item -LiteralPath $dir -Recurse -Force }
}
if ($failure -or $restoreFailure) { throw "Layout fixture failed; see $ReceiptPath" }
