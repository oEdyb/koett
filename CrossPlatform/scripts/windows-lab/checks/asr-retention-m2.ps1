$ErrorActionPreference='Stop'
$exe=(Get-LabArtifact).path
$sample=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav'
$failed=$env:APPDATA+'\DybergCo\Koett\data\Failed Recordings'
$before=@(Get-ChildItem $failed -File -ErrorAction SilentlyContinue|Select-Object -ExpandProperty FullName)
Send-DictationCommand 4;Start-Sleep -Seconds 1
$null=Start-Process $exe -ArgumentList @('--dictation-lab-asr-failure',$sample)
Start-Sleep -Seconds 8
$a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle
if($a-eq0){throw 'Open the isolated Notepad target before this check'}
Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
[Windows.Forms.Clipboard]::SetText('M2 clipboard sentinel')
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$beforeDb=(Get-FileHash $db).Hash
Send-DictationCommand 9;Start-Sleep -Seconds 2;Send-DictationCommand 9;Start-Sleep -Seconds 1
$phase=Get-DictationPhase;$text=Read-Editor $editor;$clipboard=[Windows.Forms.Clipboard]::GetText()
$new=@(Get-ChildItem $failed -File -ErrorAction SilentlyContinue|Where-Object {$_.FullName-notin$before})
$audio=@($new|ForEach-Object {$bytes=[IO.File]::ReadAllBytes($_.FullName);@{name=$_.Name;bytes=$bytes.Length;riff=([Text.Encoding]::ASCII.GetString($bytes,0,4));sample_rate=[BitConverter]::ToInt32($bytes,24);sha256=(Get-FileHash $_.FullName).Hash}})
$steps=@((Get-LabStep 'asr-worker-failure-retains-audio-without-paste'))
$dbUnchanged=(Get-FileHash $db).Hash-eq$beforeDb
$faultPassed=($new.Count-eq1-and$audio[0].riff-eq'RIFF'-and$audio[0].sample_rate-eq16000-and$audio[0].bytes-gt44-and$phase-match'Recording saved:'-and$text.Trim().Length-eq0-and$clipboard-ceq'M2 clipboard sentinel'-and$dbUnchanged)
Send-DictationCommand 4;Start-Sleep -Seconds 1
$null=Start-Process $exe -ArgumentList @('--dictation-lab-wav',$sample);Start-Sleep -Seconds 8
Activate-LabEditor $a;$editor.SetFocus();Send-DictationCommand 9;Start-Sleep -Seconds 8;Send-DictationCommand 9;Start-Sleep -Seconds 1
$successful=Read-Editor $editor;$successClip=[Windows.Forms.Clipboard]::GetText()
$afterCount=@(Get-ChildItem $failed -File -ErrorAction SilentlyContinue).Count
$steps+=Get-LabStep 'successful-asr-does-not-retain-additional-audio'
@{passed=($faultPassed-and$successful.TrimEnd([char[]]"`r`n")-ceq$successClip-and$successClip-match'Phoebe'-and$afterCount-eq($before.Count+1));artifact=Get-LabArtifact;fault_passed=$faultPassed;phase=$phase;retained_audio=$audio;clipboard_after_failure=$clipboard;failure_editor=$text;failure_history_unchanged=$dbUnchanged;successful_text=$successful;additional_success_audio=($afterCount-$before.Count-1);steps=$steps}
