param(
    [Parameter(Mandatory)][int]$AppProcessId,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][ValidateSet('select','resolve','reopen')][string]$Phase,
    [Parameter(Mandatory)][string]$OriginalText,
    [string]$EditedText,
    [string]$CanceledText,
    [Parameter(Mandatory)][string]$Draft,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [switch]$StopRejection,
    [switch]$DefiniteRejection,
    [switch]$UnknownRecovery,
    [string]$SpoolPath,
    [string]$RejectionPath,
    [string]$SnapshotPath,
    [switch]$LegacyRetry,
    [string]$LegacyAttachmentName,
    [string]$LegacyDeliveryId,
    [string]$LegacyNativePath,
    [string]$LegacyNativeHash,
    [string]$LegacyQueriesPath
)
. (Join-Path $PSScriptRoot 'windows-native-ui-common.ps1')
$result = [ordered]@{status='failed'; phase=$Phase; stages=[ordered]@{}; error=$null; controls=@()}
$app=$null; $root=$null
function Row-Action([string]$Text,[string]$Action) {
    $end=[DateTime]::UtcNow.AddSeconds(30)
    do {
        $rows=@(All-Controls | Where-Object {
            $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Group -and
            ([string]$_.Current.Name).StartsWith($Text + ' ·',[StringComparison]::Ordinal)
        })
        if ($rows.Count -gt 1) { throw "Ambiguous queued row: $Text" }
        if ($rows.Count -eq 1) {
            $buttons=@($rows[0].FindAll([System.Windows.Automation.TreeScope]::Descendants,
                [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -ceq $Action -and $_.Current.IsEnabled })
            if ($buttons.Count -gt 1) { throw "Ambiguous queued row action: $Text / $Action" }
            if ($buttons.Count -eq 1) { return $buttons[0] }
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    throw "Queued row action unavailable: $Text / $Action"
}
function Assert-Draft {
    [void](Wait-Control {
        param($c)
        $pattern=$null
        $c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$pattern) -and
            $pattern.Current.Value -ceq $Draft
    } 'independent composer draft')
    $result.stages.draftPreserved=$true
}
function Assert-LegacyThreadPaused {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $LegacyNativePath).Hash
    $queries = @(Get-Content -LiteralPath $LegacyQueriesPath | Where-Object { $_.Trim() })
    if ($hash -ine $LegacyNativeHash -or $queries.Count -ne 1) {
        throw 'The legacy fixture sent a turn or changed native history before explicit continuation'
    }
}
function Wait-LegacyReceipt {
    return Wait-Control {param($c)
        $name = [string]$c.Current.Name
        $name.StartsWith('Duplicate-risk retry approved at ',[StringComparison]::Ordinal) -and
            $name.EndsWith("native outcome was unknown. Receipt: $LegacyDeliveryId",[StringComparison]::Ordinal)
    } 'durable duplicate-risk approval with the original delivery ID'
}
function Invoke-LegacyRetry {
    Assert-LegacyThreadPaused
    $row = Wait-Control {param($c)
        $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Group -and
        ([string]$c.Current.Name).StartsWith($OriginalText + ' ·',[StringComparison]::Ordinal)
    } 'protected legacy row'
    if (@($row.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition) | Where-Object {
            $_.Current.Name -in @('Edit queued message','Cancel queued message')
        }).Count) { throw 'Legacy input became editable or retractable' }
    Invoke-Control (Wait-Name 'Review uncertain deliveries')
    if ($Phase -eq 'reopen') {
        $result.stages.coldApprovalReceipt=(Wait-LegacyReceipt).Current.Name
        Assert-LegacyThreadPaused
        $result.stages.coldApprovalRetainedWithoutSend=$true
        Invoke-Control (Wait-Name 'Resume processing')
        [void](Wait-Name 'Send message')
        $result.stages.explicitResume=$true
        return
    }
    $source = Wait-Control {param($c)
        $name = [string]$c.Current.Name
        $name.StartsWith('user · The agent may already have received this input.',[StringComparison]::Ordinal) -and
            $name.EndsWith('Attachments: ' + $LegacyAttachmentName,[StringComparison]::Ordinal)
    } 'original review source and attachment'
    $text = Wait-Control {param($c) $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and
        $c.Current.Name -ceq $OriginalText } 'original review text'
    $result.stages.reviewSource=$source.Current.Name
    $result.stages.reviewText=$text.Current.Name
    Invoke-Control (Wait-Name 'Review resend…')
    $warning = 'The agent may already have acted. Resending this input and its attachments may repeat work. After approving, select Resume processing to send it.'
    [void](Wait-Name $warning)
    $approve = Wait-Name 'Approve resend (may duplicate)'
    $result.stages.duplicateWarning=$warning
    if ($Phase -eq 'select') {
        $spoolBefore = (Get-FileHash -Algorithm SHA256 -LiteralPath $SpoolPath).Hash
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-queued-legacy-retry-confirm.png')
        Invoke-Control (Wait-Name 'Cancel resend')
        [void](Wait-Name 'Review resend…')
        if (@(All-Controls | Where-Object { $_.Current.Name -eq 'Approve resend (may duplicate)' }).Count -or
            (Get-FileHash -Algorithm SHA256 -LiteralPath $SpoolPath).Hash -cne $spoolBefore) {
            throw 'Cancel changed the queue or kept the confirmation visible'
        }
        $result.stages.cancelPreservedSpool=$true
        $result.stages.cancelSpoolSha256=$spoolBefore.ToLowerInvariant()
    } else {
        Invoke-Control $approve
        [void](Wait-Name 'Resend approved. The earlier outcome is unknown. Select Resume processing to send the original input again.')
        $result.stages.approvalReceipt=(Wait-LegacyReceipt).Current.Name
        $result.stages.approvedWithoutSending=$true
    }
    Assert-LegacyThreadPaused
}
try {
    if (-not $LegacyRetry -and (-not $EditedText -or -not $CanceledText)) {
        throw 'Queue edit and cancellation text are required'
    }
    $app=Get-Process -Id $AppProcessId
    if ($app.Path -ine [IO.Path]::GetFullPath($Executable)) { throw 'Unexpected desktop executable' }
    $end=[DateTime]::UtcNow.AddSeconds(30)
    do {
        $app.Refresh()
        if ($app.HasExited) { throw 'Desktop exited early' }
        $window=$app.MainWindowHandle
        if ($window -ne [IntPtr]::Zero -and $app.MainWindowTitle -eq 'Hive') { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $end)
    if ($window -eq [IntPtr]::Zero -or $app.MainWindowTitle -ne 'Hive') { throw 'Fixture window unavailable' }
    if (-not [NativeFixtureWindow]::SetForegroundWindow($window)) { throw 'Could not focus fixture window' }
    $root=[System.Windows.Automation.AutomationElement]::FromHandle($window)
    Assert-Draft
    if ($LegacyRetry) {
        Invoke-LegacyRetry
    } elseif ($Phase -eq 'select') {
        if ($StopRejection) {
            Invoke-Control (Wait-Name 'Resume processing')
            [void](Wait-Name 'Stop')
            $composer=Wait-Control {
                param($c)
                $p=$null
                $c.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$p) -and $p.Current.Value -ceq $Draft
            } 'composer before steering'
            $composer.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($OriginalText)
            Invoke-Control (Wait-Name 'Steer')
            $end=[DateTime]::UtcNow.AddSeconds(20)
            while (-not (Test-Path -LiteralPath $RejectionPath)) {
                if ([DateTime]::UtcNow -ge $end) { throw 'Controlled peer did not reject the actual native Steer' }
                Start-Sleep -Milliseconds 100
            }
            # Read-only snapshot before Stop anchors both original message IDs/attachments.
            $snapshot=Get-Content -Raw -LiteralPath $SpoolPath | ConvertFrom-Json
            if (@($snapshot).Count -ne 2) { throw 'Both original deliveries must exist before Stop' }
            [IO.File]::WriteAllText($SnapshotPath,($snapshot | ConvertTo-Json -Depth 24),[Text.UTF8Encoding]::new($false))
            $composer.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Draft)
            $prefix=if($LegacyRetry){'windows-queued-legacy-retry'}elseif($UnknownRecovery){'windows-queued-stop-unknown'}elseif($DefiniteRejection){'windows-queued-stop-declined'}else{'windows-queued-stop-rejection'}
            Save-NativeWindow (Join-Path $EvidenceDirectory "$prefix-before-stop.png")
            Invoke-Control (Wait-Name 'Stop')
            [void](Wait-Name 'Resume processing')
            Assert-Draft
            $result.stages.nativeSteerRejectedBeforeStop=$true
            $result.stages.stopInvoked=$true
        }
        if ($UnknownRecovery) {
            $unknown=Wait-Control {param($c) $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Group -and ([string]$c.Current.Name).StartsWith($OriginalText+' ·',[StringComparison]::Ordinal)} 'protected uncertain original'
            $actions=@($unknown.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -in @('Edit queued message','Cancel queued message') })
            if ($actions.Count) { throw 'Uncertain attempted input became editable or retractable' }
            $result.stages.unknownProtected=$true
        } else {
        Invoke-Control (Row-Action $OriginalText 'Edit queued message')
        $input=Wait-Name 'Queued message text'
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($EditedText)
        Invoke-Control (Wait-Name 'Save changes')
        [void](Row-Action $EditedText 'Edit queued message')
        Assert-Draft
        if (-not $StopRejection) {
        Invoke-Control (Row-Action $CanceledText 'Cancel queued message')
        Invoke-Control (Wait-Name 'Confirm cancellation')
        $end=[DateTime]::UtcNow.AddSeconds(15)
        do {
            $remaining=@(All-Controls | Where-Object { ([string]$_.Current.Name).StartsWith($CanceledText + ' ·',[StringComparison]::Ordinal) })
            if ($remaining.Count -eq 0) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $end)
        if ($remaining.Count) { throw 'Canceled input remained in the executable queue' }
        $result.stages.edited=$true; $result.stages.canceled=$true
        } else { $result.stages.edited=$true }
        }
    } elseif ($Phase -eq 'resolve') {
        [void](Row-Action $CanceledText 'Edit queued message')
        Invoke-Control (Wait-Name 'Review uncertain deliveries')
        $source=Wait-Control {param($c) ([string]$c.Current.Name) -cmatch '^user · (This new appended input|The agent may already have received this input\.)'} 'uncertain original source read through UIA'
        $text=Wait-Control {param($c) $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and $c.Current.Name -ceq $OriginalText} 'exact uncertain original read through UIA'
        $result.stages.reviewSource=$source.Current.Name
        $result.stages.reviewText=$text.Current.Name
        Invoke-Control (Wait-Name 'Discard this queued wait…')
        $confirmation='The agent may already have acted. This only discards the queued wait; native work and Hive audit remain.'
        [void](Wait-Control {param($c) $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and $c.Current.Name -ceq $confirmation} 'second confirmation read through UIA')
        $result.stages.confirmationRead=$confirmation
        Save-NativeWindow (Join-Path $EvidenceDirectory 'windows-queued-stop-unknown-confirm.png')
        Invoke-Control (Wait-Name 'Confirm discard queued wait')
        [void](Wait-Name 'Queued wait discarded; native outcome remains unknown.')
        [void](Wait-Control {param($c) ([string]$c.Current.Name).StartsWith('Local wait discarded at ',[StringComparison]::Ordinal)} 'durable local disposition receipt')
        [void](Row-Action $CanceledText 'Edit queued message')
        $result.stages.explicitLocalDiscard=$true
    } else {
        if ($UnknownRecovery) {
            [void](Row-Action $CanceledText 'Edit queued message')
            Invoke-Control (Wait-Name 'Review uncertain deliveries')
            [void](Wait-Control {param($c) ([string]$c.Current.Name).StartsWith('Local wait discarded at ',[StringComparison]::Ordinal)} 'cold disposition receipt')
            if (@(All-Controls | Where-Object { ([string]$_.Current.Name).StartsWith($OriginalText+' ·',[StringComparison]::Ordinal) }).Count) { throw 'Discarded local wait returned after restart' }
            $result.stages.coldReceiptRetained=$true
        } else {
        [void](Row-Action $EditedText 'Edit queued message')
        if (@(All-Controls | Where-Object { ([string]$_.Current.Name).StartsWith($CanceledText + ' ·',[StringComparison]::Ordinal) }).Count) {
            throw 'Canceled input returned after cold reopen'
        }
        }
        Invoke-Control (Wait-Name 'Resume processing')
        [void](Wait-Control {param($c) $c.Current.Name -match 'Synthetic queued completion'} 'controlled resumed completion')
        $result.stages.explicitResume=$true
    }
    Assert-Draft
    $prefix=if($LegacyRetry){'windows-queued-legacy-retry'}elseif($UnknownRecovery){'windows-queued-stop-unknown'}elseif($DefiniteRejection){'windows-queued-stop-declined'}elseif($StopRejection){'windows-queued-stop-rejection'}else{'windows-queued-inputs'}
    Save-NativeWindow (Join-Path $EvidenceDirectory ("$prefix-$Phase.png"))
    if (-not $app.CloseMainWindow()) { throw 'Could not request normal exit' }
    if (-not $app.WaitForExit(30000)) { throw 'Desktop did not exit' }
    $result.stages.exitObserved=$true; $result.status='passed'
} catch {
    $result.error=$_.Exception.ToString()
    if ($root) {
        try { $result.controls=@(All-Controls | ForEach-Object { $_.Current.Name } | Where-Object { $_ } | Select-Object -First 180) } catch {}
        $prefix=if($LegacyRetry){'windows-queued-legacy-retry'}elseif($UnknownRecovery){'windows-queued-stop-unknown'}elseif($DefiniteRejection){'windows-queued-stop-declined'}elseif($StopRejection){'windows-queued-stop-rejection'}else{'windows-queued-inputs'}
        try { Save-NativeWindow (Join-Path $EvidenceDirectory ("$prefix-$Phase-failure.png")) } catch { $result.captureError=$_.Exception.Message }
    }
} finally {
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))) | Out-Null
    [IO.File]::WriteAllText($OutputPath,($result | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
}
if ($result.status -ne 'passed') { throw "Queued input fixture failed: $($result.error)" }
