$a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle
$b=[LabDriver]::ByTitle('M2 Other Target');if($b-eq[IntPtr]::Zero){throw 'Other target missing'}
Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3'
try {
 (Get-Item $db).IsReadOnly=$true
 Send-DictationCommand 9;Start-Sleep -Seconds 8
 try{[DictationLab]::Key(17,$true);Send-DictationCommand 9;Start-Sleep -Milliseconds 50;Activate-LabEditor $b}finally{[DictationLab]::Key(17,$false)}
 Start-Sleep -Seconds 1
 $phase=Get-DictationPhase;$overlay=[LabDriver]::ByClass('KoettOverlayWindow');$root=[Windows.Automation.AutomationElement]::FromHandle($overlay)
 $actions=@($root.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
 $steps=@((Get-LabStep 'combined-delivery-and-history-failure'))
}finally{(Get-Item $db).IsReadOnly=$false}
@{passed=($phase-match'Not pasted'-and'Paste'-in$actions-and'Copy'-in$actions);artifact=Get-LabArtifact;phase=$phase;actions=$actions;readonly_restored=(-not(Get-Item $db).IsReadOnly);steps=$steps}
