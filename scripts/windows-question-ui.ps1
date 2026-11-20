param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select', 'reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$Source,
    [Parameter(Mandatory)][string]$SessionId,
    [Parameter(Mandatory)][string]$WorkspaceId,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result = [ordered]@{ status = 'failed'; phase = $Phase; checks = @(); error = $null; controls = @() }
$root = $null
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
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus fixture window' }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    $fixtureRoot = [IO.Path]::GetDirectoryName([IO.Path]::GetDirectoryName($Source))
    if ($Phase -eq 'select') {
        $editor = Wait-Name 'Conversation messages'
        $editor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('Start the controlled question')
        Invoke-Control (Wait-Name 'Send message')
        [void](Wait-Name 'Verify fixture')
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            $buttons = @(All-Controls | Where-Object { $_.Current.Name -ceq 'Add files or images' -and $_.Current.IsEnabled } | Sort-Object { $_.Current.BoundingRectangle.Top })
            if ($buttons.Count -ge 2) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($buttons.Count -lt 2) { throw 'Question attachment button unavailable' }
        Invoke-Control $buttons[0]
        # The owned common dialog is included under the application's UIA
        # tree on Windows; wait for the modal dialog to take foreground, find
        # its ComboBox->Edit child HWND recursively via Win32, set the path, and submit.
        Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class OpenDialogHelper {
    public delegate bool EnumChildProc(IntPtr hwnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumChildProc cb, IntPtr lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder sb, int max);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, string lParam);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr hWnd);
    public static IntPtr FindComboEdit(IntPtr dialog) {
        IntPtr found = IntPtr.Zero;
        EnumChildWindows(dialog, (hwnd, _) => {
            var cls = new StringBuilder(64);
            GetClassName(hwnd, cls, cls.Capacity);
            if (cls.ToString() == "Edit") {
                var parent = GetParent(hwnd);
                var pcls = new StringBuilder(64);
                if (parent != IntPtr.Zero) GetClassName(parent, pcls, pcls.Capacity);
                if (pcls.ToString() == "ComboBox") { found = hwnd; return false; }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        $dialogHwnd = [IntPtr]::Zero
        $editHwnd = [IntPtr]::Zero
        do {
            $fg = [NativeFixtureWindow]::GetForegroundWindow()
            [uint32]$fgPid = 0
            if ($fg -ne [IntPtr]::Zero -and $fg -ne $window) {
                [void][OpenDialogHelper]::GetWindowThreadProcessId($fg, [ref]$fgPid)
                if ($fgPid -eq $AppProcessId) {
                    $candidateEdit = [OpenDialogHelper]::FindComboEdit($fg)
                    if ($candidateEdit -ne [IntPtr]::Zero) {
                        $dialogHwnd = $fg
                        $editHwnd = $candidateEdit
                        break
                    }
                }
            }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($editHwnd -eq [IntPtr]::Zero) { throw 'Native file chooser ComboBox Edit HWND unavailable' }
        $editElem = [System.Windows.Automation.AutomationElement]::FromHandle($editHwnd)
        $vp = $null
        if ($editElem.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)) {
            $vp.SetValue($Source)
        } else {
            [void][OpenDialogHelper]::SendMessage($editHwnd, 0x000C, [IntPtr]::Zero, $Source)
        }
        Start-Sleep -Milliseconds 200
        $dialogElem = [System.Windows.Automation.AutomationElement]::FromHandle($dialogHwnd)
        $okCond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty,
            'Select images, documents or other files'
        )
        $okElem = $dialogElem.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $okCond)
        if ($null -ne $okElem) {
            $okElem.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        } else {
            [void][OpenDialogHelper]::PostMessage($dialogHwnd, 0x0111, [IntPtr]1, [IntPtr]::Zero)
        }
        [void](Wait-Name 'Remove question-proof.txt')
        # Wait for the original question's saved draft and compare the exact
        # private bytes before moving the source and submitting the answer.
        $receiptDir = Join-Path $fixtureRoot 'state\chats\interaction-attachments'
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        $draft = $null
        do {
            if (Test-Path -LiteralPath $receiptDir) {
                foreach ($file in @(Get-ChildItem -LiteralPath $receiptDir -Filter '*.json')) {
                    try {
                        $candidate = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
                        if ($candidate.status -eq 'draft' -and $candidate.sessionId -ceq $SessionId -and $candidate.workspaceId -ceq $WorkspaceId -and $candidate.nativeRequestId -eq 7 -and $candidate.attachments.Count -eq 1 -and $candidate.attachments[0].questionId -eq 'public') { $draft = $candidate }
                    } catch {}
                }
            }
            if ($null -ne $draft) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($null -eq $draft) { throw 'Saved question attachment draft unavailable' }
        $private = [string]$draft.attachments[0].filePath
        if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($Source)) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($private))) { throw 'Private bytes differ from source' }
        Move-Item -LiteralPath $Source -Destination ($Source + '.held')
        if (Test-Path -LiteralPath $Source) { throw 'Source still present' }
        $result.checks += @('native chooser selected file', 'private bytes equal original', 'original moved')
        Invoke-Control (Wait-Name 'Verify fixture')
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-question-before-submit.png')
        Invoke-Control (Wait-Name 'Submit answers')
        $response = Join-Path $fixtureRoot 'question-responses.jsonl'
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            if ((Test-Path -LiteralPath $response) -and (Get-Item -LiteralPath $response).Length -gt 0) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        if (-not (Test-Path -LiteralPath $response)) { throw 'Controlled peer did not receive the answer' }
        $result.checks += 'original question delivered'
    } else {
        [void](Wait-Control { param($c)
            $name = [string]$c.Current.Name
            if ($name -like '*Question attachments*Callback handed off*') { return $true }
            $pattern = $null
            if ($c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
                return $pattern.Current.Value -like '*Question attachments*Callback handed off*'
            }
            return $false
        } 'delivered attachment receipt')
        $result.checks += 'delivered receipt visible after cold launch'
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-question-cold.png')
    }
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit' }
    $result.checks += 'normal exit observed'; $result.status = 'passed'
} catch {
    $result.error = $_.Exception.ToString()
    if ($root) {
        try { $result.controls = @(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -First 200) } catch {}
        try { $result.editControls = @(All-Controls | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit } | ForEach-Object { @{ name=$_.Current.Name; id=$_.Current.AutomationId; processId=$_.Current.ProcessId; enabled=$_.Current.IsEnabled; offscreen=$_.Current.IsOffscreen } }) } catch {}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory "windows-question-$Phase-failure.png") } catch { $result.captureError = $_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Question fixture failed: $($result.error)" }
