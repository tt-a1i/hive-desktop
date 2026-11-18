param(
    [Parameter(Mandatory)] [string] $Bundle,
    [Parameter(Mandatory)] [string] $Receipt
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
$executable = Join-Path (Resolve-Path $Bundle) 'hive-desktop.exe'
$receiptPath = [IO.Path]::GetFullPath($Receipt)
$directory = Join-Path ([IO.Path]::GetTempPath()) ('hive voice ' + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $directory | Out-Null
$wave = Join-Path $directory 'speech sample.wav'
$evidence = @{ pass=$false; offline=$true; microphoneUsed=$false; recognizedAudio=$false }
try {
    $recognizers = [System.Speech.Recognition.SpeechRecognitionEngine]::InstalledRecognizers()
    $evidence.installedLanguages = @($recognizers | ForEach-Object { $_.Culture.Name })
    if ($recognizers.Count -eq 0) {
        $reply = & $executable --voice-check-file $wave | ConvertFrom-Json
        if ($LASTEXITCODE -ne 1 -or $reply.state -ne 'error' -or $reply.code -ne 'unavailable') {
            throw 'The packaged application did not diagnose the missing installed recognizer.'
        }
        $evidence.diagnostic = $reply.code
        $evidence.pass = $true
        Write-Host 'No installed offline speech recognizer. Verified the packaged diagnostic only.'
        return
    }

    $engine = [System.Speech.Recognition.SpeechRecognitionEngine]::new()
    try { $language = $engine.RecognizerInfo.Culture.Name }
    finally { $engine.Dispose() }
    if ($language -ne 'en-US') { throw "Windows fixture needs the runner's en-US recognition engine (found $language)." }
    $synth = [System.Speech.Synthesis.SpeechSynthesizer]::new()
    try {
        $voice = @($synth.GetInstalledVoices() | Where-Object { $_.Enabled -and $_.VoiceInfo.Culture.Name -eq $language })[0]
        if ($null -eq $voice) { throw "No $language voice for the audio fixture." }
        $synth.SelectVoice($voice.VoiceInfo.Name)
        $synth.SetOutputToWaveFile($wave)
        $synth.Speak('The garden has seven bright red flowers.')
    } finally { $synth.Dispose() }
    $reply = & $executable --voice-check-file $wave | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $reply.state -ne 'completed' -or $reply.text -notmatch '\bgarden\b' -or $reply.text -notmatch '\bflowers\b') {
        throw "The packaged recognizer did not transcribe the audio fixture: $($reply | ConvertTo-Json -Compress)"
    }
    $evidence.language = $language
    $evidence.transcript = $reply.text
    $evidence.recognizedAudio = $true
    $evidence.pass = $true
    Write-Host 'The packaged Windows recognizer transcribed actual offline audio.'
} finally {
    $evidence | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 -Path $receiptPath
    Remove-Item -LiteralPath $directory -Recurse -Force
}
