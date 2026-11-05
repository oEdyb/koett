$exe=Join-Path (Split-Path (Get-LabArtifact).path) 'koett-engine.exe'
$model=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8'
$sample=$model+'\test_wavs\0.wav';$silence=$env:USERPROFILE+'\m2-silence.wav'
$header=[Convert]::FromBase64String('UklGRiR3AQBXQVZFZm10IBAAAAABAAEAgD4AAAB9AAACABAAZGF0YQB3AQA=')
$wav=New-Object byte[] 96044;[Array]::Copy($header,$wav,44);[IO.File]::WriteAllBytes($silence,$wav)
$out=$env:USERPROFILE+'\koett-offline.stdout.txt';$err=$env:USERPROFILE+'\koett-offline.stderr.txt'
$p=Start-Process $exe -ArgumentList @($model,$sample,$silence) -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
$null=$p.Handle;$p.PriorityClass='Idle';$p.WaitForExit()
$text=[IO.File]::ReadAllText($out);$errors=[IO.File]::ReadAllText($err)
$lines=$text-split"`n";$silenceLine=@($lines|Where-Object {$_-like'*m2-silence.wav*'})
@{passed=($p.ExitCode-eq0-and$text-match'Phoebe'-and$text-match'old portrait'-and$silenceLine.Count-eq1-and$silenceLine[0].TrimEnd("`r").EndsWith("`t"));exit_code=$p.ExitCode;stdout=$text;stderr=$errors;exe_sha256=(Get-FileHash $exe).Hash;sample_sha256=(Get-FileHash $sample).Hash;silence_sha256=(Get-FileHash $silence).Hash;network='Caller must disconnect QEMU NIC before this check';focus=Get-LabFocus}
