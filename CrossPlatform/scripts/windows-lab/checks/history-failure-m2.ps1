$a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle
Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$beforeHash=(Get-FileHash $db).Hash
$recovery=$env:APPDATA+'\DybergCo\Koett\data\Failed Transcripts';$before=@(Get-ChildItem $recovery -File -ErrorAction SilentlyContinue|Select-Object -ExpandProperty FullName)
try {
 $file=Get-Item $db;$file.IsReadOnly=$true
 Send-DictationCommand 9;Start-Sleep -Seconds 8;Send-DictationCommand 9;Start-Sleep -Seconds 1
 $text=Read-Editor $editor;$clip=[Windows.Forms.Clipboard]::GetText();$phase=Get-DictationPhase
 $new=@(Get-ChildItem $recovery -File -ErrorAction SilentlyContinue|Where-Object {$_.FullName-notin$before})
 $saved=@($new|ForEach-Object {[IO.File]::ReadAllText($_.FullName)})
 $steps=@((Get-LabStep 'history-write-failed-after-real-paste'),(Get-LabWindowStep 'history-failure-did-not-block-paste' $a))
 $hash=(Get-FileHash $db).Hash
}finally{(Get-Item $db).IsReadOnly=$false}
@{passed=($text.TrimEnd([char[]]"`r`n")-ceq$clip-and$clip-match'Phoebe'-and$beforeHash-eq$hash-and$new.Count-eq1-and(@($saved|Where-Object {$_-match'Phoebe'}).Count-eq1));artifact=Get-LabArtifact;editor_text=$text;clipboard=$clip;phase=$phase;database_unchanged=($beforeHash-eq$hash);new_recovery_files=$new.Count;recovery_text=$saved;readonly_restored=(-not(Get-Item $db).IsReadOnly);steps=$steps}
