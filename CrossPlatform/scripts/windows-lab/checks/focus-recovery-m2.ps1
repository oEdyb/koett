$code='Add-Type -AssemblyName System.Windows.Forms;$f=New-Object Windows.Forms.Form;$f.Text="M2 Other Target";$f.Width=600;$f.Height=300;$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock="Fill";$f.Controls.Add($t);[void]$f.ShowDialog()'
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code));$null=Start-Process powershell.exe -ArgumentList @('-NoProfile','-STA','-EncodedCommand',$encoded)
$other=[IntPtr]::Zero;for($i=0;$i-lt40-and$other-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 200;$other=[LabDriver]::ByTitle('M2 Other Target')}
if($other-eq[IntPtr]::Zero){throw 'Other target missing'}
$a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle
Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
if((Read-Editor $editor).Trim().Length-ne0){throw 'Original editor did not clear'}
Send-DictationCommand 9;Start-Sleep -Seconds 8
try {
 [DictationLab]::Key(17,$true);Send-DictationCommand 9
 $during='';for($i=0;$i-lt30;$i++){Start-Sleep -Milliseconds 10;$during=Get-DictationPhase;if($during-match'Transcribing'){break}}
 Activate-LabEditor $other
}finally{[DictationLab]::Key(17,$false)}
Start-Sleep -Milliseconds 700
$overlay=[LabDriver]::ByClass('KoettOverlayWindow');$root=[Windows.Automation.AutomationElement]::FromHandle($overlay)
$names=@($root.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$phase=Get-DictationPhase;$steps=@((Get-LabStep 'foreground-switch-during-asr-visible-recovery'))
$beforeA=Read-Editor $editor;$secondEditor=Find-Editor $other;$beforeB=Read-Editor $secondEditor
$null=Invoke-LabButton $overlay 'Paste';Start-Sleep -Milliseconds 200
$wrongRefused=((Read-Editor $secondEditor)-ceq$beforeB-and(Read-Editor $editor)-ceq$beforeA)
Start-Sleep -Seconds 2;$persistent=Get-DictationPhase;$steps+=Get-LabStep 'not-pasted-persists-with-copy-and-paste'
Activate-LabEditor $a;$editor.SetFocus();$null=Invoke-LabButton $overlay 'Paste';Start-Sleep -Milliseconds 300
$after=Read-Editor $editor;$clip=[Windows.Forms.Clipboard]::GetText();$steps+=Get-LabStep 'manual-paste-recovered-original-target'
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$null=Invoke-WebRequest -UseBasicParsing -Method Post http://10.0.2.2:18768/history -InFile $db
@{passed=($during-match'Transcribing'-and$phase-match'Not pasted'-and$persistent-match'Not pasted'-and'Paste'-in$names-and'Copy'-in$names-and$beforeA.Trim().Length-eq0-and$beforeB.Trim().Length-eq0-and$wrongRefused-and$after.TrimEnd([char[]]"`r`n")-ceq$clip);artifact=Get-LabArtifact;during_asr=$during;failure_phase=$phase;persistent_phase=$persistent;actions=$names;wrong_target_refused=$wrongRefused;before_original=$beforeA;before_other=$beforeB;recovered=$after;clipboard=$clip;steps=$steps}
