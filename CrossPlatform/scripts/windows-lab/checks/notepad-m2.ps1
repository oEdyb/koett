$path=$env:USERPROFILE+'\M2 Notepad.txt';[IO.File]::WriteAllText($path,'')
$null=Start-Process notepad.exe -ArgumentList ('"'+$path+'"')
Start-Sleep -Seconds 3
$np=Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1
$h=$np.MainWindowHandle;Activate-LabEditor $h
$editor=Find-Editor $h;$editor.SetFocus();Start-Sleep -Milliseconds 200
$before=Read-Editor $editor
if($before.Trim().Length-ne0){throw "Notepad is not empty: $before"}
$focus=Get-LabFocus
Send-DictationCommand 9
Start-Sleep -Milliseconds 400
$steps=@((Get-LabStep 'notepad-real-recording'))
Start-Sleep -Milliseconds 7500
Send-DictationCommand 9
$phases=@()
for($i=0;$i-lt80;$i++){
 Start-Sleep -Milliseconds 100;$phase=Get-DictationPhase;$phases+=$phase
 if($phase-match'Koett Done|Not pasted|failed'){break}
}
$text=Read-Editor $editor;$clipboard=[Windows.Forms.Clipboard]::GetText()
$steps+=Get-LabStep 'notepad-real-result'
$steps+=Get-LabWindowStep 'notepad-recognized-text' $h
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3'
$null=Invoke-WebRequest -UseBasicParsing -Method Post http://10.0.2.2:18768/history -InFile $db
@{passed=($text.TrimEnd([char[]]"`r`n")-ceq$clipboard-and$clipboard-match'Phoebe'-and$clipboard-match'old portrait'-and(Get-LabFocus).hwnd-eq$focus.hwnd);artifact=Get-LabArtifact;before=$before;editor_text=$text;clipboard=$clipboard;phase_samples=$phases;original_focus=$focus;steps=$steps}
