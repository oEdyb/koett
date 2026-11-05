if(-not('M2Native'-as[type])){Add-Type 'using System;using System.Runtime.InteropServices;public class M2Native{[DllImport("user32.dll")]public static extern IntPtr GetDlgItem(IntPtr h,int id); }'}
Send-DictationCommand 5;Start-Sleep -Milliseconds 600
$settings=[LabDriver]::ByTitle('Koett Settings');$combo=[M2Native]::GetDlgItem($settings,33)
$null=[LabDriver]::SendMessage($combo,0x14E,[IntPtr]1,[IntPtr]::Zero);$null=Invoke-LabButton $settings 'Save'
$steps=@((Get-LabWindowStep 'hold-mode-saved' $settings));$null=[LabDriver]::PostMessage($settings,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 400
$np=Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1
$h=$np.MainWindowHandle;Activate-LabEditor $h;$editor=Find-Editor $h;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
if((Read-Editor $editor).Trim().Length-ne0){throw 'Notepad did not clear'}
try {
 [DictationLab]::Key(17,$true);[DictationLab]::Key(16,$true);[DictationLab]::Key(32,$true)
 Start-Sleep -Milliseconds 500;$holdStart=Get-DictationPhase;$steps+=Get-LabStep 'hold-recording'
 Start-Sleep -Milliseconds 7500
} finally {[DictationLab]::Key(32,$false);[DictationLab]::Key(16,$false);[DictationLab]::Key(17,$false)}
Start-Sleep -Seconds 1
$holdText=Read-Editor $editor;$holdClip=[Windows.Forms.Clipboard]::GetText();$steps+=Get-LabStep 'hold-result'
$holdPassed=($holdStart-match'Recording'-and$holdText.TrimEnd([char[]]"`r`n")-ceq$holdClip-and$holdClip-match'Phoebe')
$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
Send-DictationCommand 9;Start-Sleep -Milliseconds 500
$overlay=[LabDriver]::ByClass('KoettOverlayWindow');$null=Invoke-LabButton $overlay 'Pause';$paused=Get-DictationPhase;$steps+=Get-LabStep 'real-paused'
Start-Sleep -Seconds 1;$null=Invoke-LabButton $overlay 'Resume';$resumed=Get-DictationPhase
Start-Sleep -Seconds 7;Send-DictationCommand 9;Start-Sleep -Seconds 1
$pauseText=Read-Editor $editor;$pauseClip=[Windows.Forms.Clipboard]::GetText();$steps+=Get-LabStep 'resumed-result'
$pausePassed=($paused-match'Paused'-and$resumed-match'Recording'-and$pauseText.TrimEnd([char[]]"`r`n")-ceq$pauseClip-and$pauseClip.Length-gt0)
$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
Send-DictationCommand 9;Start-Sleep -Milliseconds 700;$null=Invoke-LabButton $overlay 'Cancel';Start-Sleep -Seconds 2
$cancelText=Read-Editor $editor;$cancelPhase=Get-DictationPhase;$steps+=Get-LabStep 'real-cancelled'
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$null=Invoke-WebRequest -UseBasicParsing -Method Post http://10.0.2.2:18768/history -InFile $db
@{passed=($holdPassed-and$pausePassed-and$cancelText.Trim().Length-eq0-and-not[LabDriver]::IsWindowVisible($overlay));artifact=Get-LabArtifact;hold=$holdPassed;hold_start=$holdStart;hold_text=$holdText;pause_resume=$pausePassed;paused=$paused;resumed=$resumed;pause_text=$pauseText;cancel_text=$cancelText;cancel_phase=$cancelPhase;steps=$steps}
