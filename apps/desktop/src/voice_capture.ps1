$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$control = [IO.StreamReader]::new([Console]::OpenStandardInput())
if ($control.ReadLine() -cne 'start') { exit }

$engine = $null
$stage = 'unavailable'
function Reply($result) { [Console]::WriteLine(($result | ConvertTo-Json -Compress)) }
try {
    Add-Type -AssemblyName System.Speech
    $engine = [System.Speech.Recognition.SpeechRecognitionEngine]::new()
    $engine.LoadGrammar([System.Speech.Recognition.DictationGrammar]::new())
    $engine.InitialSilenceTimeout = [TimeSpan]::FromSeconds(8)
    $mode = [System.Speech.Recognition.RecognizeMode]::Multiple
    $stage = 'mic_unavailable'
    if ($env:HIVE_VOICE_AUDIO_FIXTURE) {
        $engine.SetInputToWaveFile($env:HIVE_VOICE_AUDIO_FIXTURE)
        $mode = [System.Speech.Recognition.RecognizeMode]::Single
    } else {
        $engine.SetInputToDefaultAudioDevice()
    }
    $stage = 'recognition_failed'
    Register-ObjectEvent -InputObject $engine -EventName SpeechRecognized -SourceIdentifier HiveVoicePhrase | Out-Null
    Register-ObjectEvent -InputObject $engine -EventName RecognizeCompleted -SourceIdentifier HiveVoiceDone | Out-Null
    $controlTask = $control.ReadLineAsync()
    $parts = [System.Collections.Generic.List[string]]::new()
    $finishing = $false
    $completionDeadline = [DateTime]::MaxValue
    $captureDeadline = [DateTime]::UtcNow.AddSeconds(60)
    $engine.RecognizeAsync($mode)
    do {
        if (-not $finishing -and ($controlTask.IsCompleted -or [DateTime]::UtcNow -ge $captureDeadline)) {
            if ($controlTask.IsCompleted -and $controlTask.GetAwaiter().GetResult() -cne 'finish') {
                $engine.RecognizeAsyncCancel()
                exit
            }
            $engine.RecognizeAsyncStop()
            $finishing = $true
            $completionDeadline = [DateTime]::UtcNow.AddSeconds(3)
        }
        $event = Wait-Event -Timeout 1
        if ($null -eq $event) { continue }
        Remove-Event -EventIdentifier $event.EventIdentifier
        $details = $event.SourceEventArgs
        if ($event.SourceIdentifier -eq 'HiveVoicePhrase') {
            if ($details.Result.Text) { $parts.Add($details.Result.Text) }
        } elseif ($event.SourceIdentifier -eq 'HiveVoiceDone') {
            if ($details.Error -and $parts.Count -eq 0) { throw $details.Error }
            if ($parts.Count -eq 0 -and $details.Result.Text) { $parts.Add($details.Result.Text) }
            break
        }
    } while ([DateTime]::UtcNow -lt $completionDeadline)
    $text = [string]::Join(' ', $parts).Trim()
    if ($text.Length) { Reply @{state='completed'; text=$text} }
    else { Reply @{state='error'; code='no_speech'} }
} catch {
    $code = $stage
    if ($stage -eq 'mic_unavailable' -and $_.Exception.GetBaseException().HResult -eq -2147024891) {
        $code = 'mic_denied'
    }
    Reply @{state='error'; code=$code}
} finally {
    Unregister-Event -SourceIdentifier HiveVoicePhrase -ErrorAction SilentlyContinue
    Unregister-Event -SourceIdentifier HiveVoiceDone -ErrorAction SilentlyContinue
    if ($null -ne $engine) { $engine.Dispose() }
}
