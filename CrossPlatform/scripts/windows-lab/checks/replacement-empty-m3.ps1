$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$hash=(Get-FileHash $config).Hash
$steps=@()
try{
 # Only the isolated guest settings are changed. This is the exact verified Compact result.
 $s=$original|ConvertFrom-Json;$s.dictionary=@();$s.formatting.enabled=$false
 $s.replacements=@(@{phrase="Well, I don't wish to see it any more, observed Phoebe, turning away her eyes. It is certainly very like the old portrait.";replacement=''})
 [IO.File]::WriteAllText($config,($s|ConvertTo-Json -Depth 15),(New-Object Text.UTF8Encoding($false)))
 $target=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle;Activate-LabEditor $target;$editor=Find-Editor $target;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}')
 [Windows.Forms.Clipboard]::SetText('M3 preserve clipboard after empty replacement');$before=Get-LabFocus
 Send-DictationCommand 9;Start-Sleep -Seconds 10;Send-DictationCommand 9
 Start-Sleep -Seconds 2
 $text=Read-Editor $editor;$clip=[Windows.Forms.Clipboard]::GetText();$after=Get-LabFocus;$phase=Get-DictationPhase
 $steps+=Get-LabWindowStep 'empty-replacement-keeps-clipboard-and-target' $target
 $ok=($text.TrimEnd([char[]]"`r`n")-ceq''-and$clip-ceq'M3 preserve clipboard after empty replacement'-and$before.hwnd-eq$after.hwnd)
}finally{[IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))}
@{passed=$ok;editor=$text;clipboard=$clip;phase=$phase;focus_before=$before;focus_after=$after;settings_restored=((Get-FileHash $config).Hash-eq$hash);fixture='official Compact 0.wav; a literal rule removes the whole result';artifact=Get-LabArtifact;steps=$steps}
