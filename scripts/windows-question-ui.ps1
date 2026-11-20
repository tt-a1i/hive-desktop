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
        # tree on Windows; locate the Open dialog's ComboBoxEx32/ComboBox/Edit
        # filename field via UIA or its Win32 child HWND, then click the OK button.
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OpenDialogHelper {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, string lParam);
}
'@
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        $openDialog = $null
        $okButton = $null
        do {
            $openDialog = @(All-Controls | Where-Object {
                $_.Current.ProcessId -eq $AppProcessId -and
                $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Window -and
                $_.Current.Name -ceq 'Open'
            }) | Select-Object -First 1
            $okButton = @(All-Controls | Where-Object {
                $_.Current.ProcessId -eq $AppProcessId -and
                $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
                $_.Current.Name -ceq 'Select images, documents or other files'
            }) | Select-Object -First 1
            if ($null -ne $openDialog -and $null -ne $okButton) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($null -eq $openDialog -or $null -eq $okButton) { throw 'Native file chooser Open dialog unavailable' }
        $dialogHwnd = [IntPtr]$openDialog.Current.NativeWindowHandle
        $comboEx = [OpenDialogHelper]::FindWindowEx($dialogHwnd, [IntPtr]::Zero, 'ComboBoxEx32', $null)
        $combo = if ($comboEx -ne [IntPtr]::Zero) { [OpenDialogHelper]::FindWindowEx($comboEx, [IntPtr]::Zero, 'ComboBox', $null) } else { [IntPtr]::Zero }
        $editHwnd = if ($combo -ne [IntPtr]::Zero) { [OpenDialogHelper]::FindWindowEx($combo, [IntPtr]::Zero, 'Edit', $null) } else { [IntPtr]::Zero }
        if ($editHwnd -ne [IntPtr]::Zero) {
            [void][OpenDialogHelper]::SendMessage($editHwnd, 0x000C, [IntPtr]::Zero, $Source)
        } else {
            $field = @($openDialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | Where-Object {
                $vp = $null
                ($_.Current.AutomationId -eq '1148' -or $_.Current.Name -ceq 'File name:') -and
                $_.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$vp)
            }) | Select-Object -First 1
            if ($null -eq $field) { throw 'Native file chooser filename Edit HWND and ValuePattern unavailable' }
            $field.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Source)
        }
        $okButton.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
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
