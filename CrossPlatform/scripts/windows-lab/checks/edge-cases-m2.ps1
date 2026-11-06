$ErrorActionPreference='Stop'
$exe=$env:USERPROFILE+'\KoettM2d\Koett-Windows-x86_64\Koett.exe'
$sample=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav'
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3'
$failed=$env:APPDATA+'\DybergCo\Koett\data\Failed Recordings'
function Restart-Wav([string]$path){
 if([LabDriver]::ByClass('KoettOwnerWindow')-ne[IntPtr]::Zero){Send-DictationCommand 4;Start-Sleep -Seconds 1}
 $null=Start-Process $exe -ArgumentList @('--dictation-lab-wav',$path);Start-Sleep -Seconds 8
}
function Write-Silence([string]$path,[int]$samples){
 $stream=[IO.File]::Create($path);$w=New-Object IO.BinaryWriter($stream)
 try{$w.Write([Text.Encoding]::ASCII.GetBytes('RIFF'));$w.Write([int](36+$samples*2));$w.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '));$w.Write([int]16);$w.Write([int16]1);$w.Write([int16]1);$w.Write([int]16000);$w.Write([int]32000);$w.Write([int16]2);$w.Write([int16]16);$w.Write([Text.Encoding]::ASCII.GetBytes('data'));$w.Write([int]($samples*2));for($i=0;$i-lt$samples;$i++){$w.Write([int16]0)}}finally{$w.Dispose()}
}
$path=$env:USERPROFILE+'\M2 edge target.txt';[IO.File]::WriteAllText($path,'');$null=Start-Process notepad.exe -ArgumentList ('"'+$path+'"');Start-Sleep -Seconds 2
$a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle
$results=@();$steps=@()
foreach($case in @(@{name='silence';samples=48000;wait=3500},@{name='short';samples=1600;wait=300},@{name='no-samples';samples=0;wait=300})){
 $wav=$env:USERPROFILE+'\m2-'+$case.name+'.wav';Write-Silence $wav $case.samples;Restart-Wav $wav
 Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}')
 [Windows.Forms.Clipboard]::SetText('edge clipboard sentinel');$hash=(Get-FileHash $db).Hash;$audio=@(Get-ChildItem $failed -File).Count
 Send-DictationCommand 9;Start-Sleep -Milliseconds $case.wait
 $during=Get-DictationPhase
 if($during-match'Recording'){Send-DictationCommand 9};Start-Sleep -Seconds 2
 $phase=Get-DictationPhase;$text=Read-Editor $editor;$clip=[Windows.Forms.Clipboard]::GetText()
 $results+=@{case=$case.name;passed=($text.Trim().Length-eq0-and$clip-ceq'edge clipboard sentinel'-and(Get-FileHash $db).Hash-eq$hash-and@(Get-ChildItem $failed -File).Count-eq$audio);phase=$phase;during=$during;editor=$text;clipboard=$clip;history_unchanged=((Get-FileHash $db).Hash-eq$hash)}
 $steps+=Get-LabStep $case.name
}
Restart-Wav $sample
# A private WinForms target can be destroyed without touching any user's editor.
$code='Add-Type -AssemblyName System.Windows.Forms;$f=New-Object Windows.Forms.Form;$f.Text="M2 Destroy Target";$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock="Fill";$f.Controls.Add($t);[void]$f.ShowDialog()'
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code));$target=Start-Process powershell.exe -ArgumentList @('-NoProfile','-STA','-EncodedCommand',$encoded) -PassThru
try{
 $h=[IntPtr]::Zero;for($i=0;$i-lt40-and$h-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 100;$h=[LabDriver]::ByTitle('M2 Destroy Target')}
 Activate-LabEditor $h;Send-DictationCommand 9;Start-Sleep -Seconds 8
 Stop-Process -Id $target.Id;Start-Sleep -Milliseconds 200
 Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();Send-DictationCommand 9;Start-Sleep -Seconds 2
 $phase=Get-DictationPhase;$overlay=[LabDriver]::ByClass('KoettOverlayWindow');$null=Invoke-LabButton $overlay 'Copy';$clip=[Windows.Forms.Clipboard]::GetText()
 $results+=@{case='destroyed-target';passed=($phase-match'Not pasted'-and(Read-Editor $editor).Trim().Length-eq0-and$clip-match'Phoebe');phase=$phase;recovered_copy=$clip}
 $steps+=Get-LabStep 'destroyed-target-refuses-paste-and-keeps-copy'
}finally{if(!$target.HasExited){Stop-Process -Id $target.Id}}
# A separate guest process holds the clipboard beyond Koett's bounded retries.
$ready=$env:USERPROFILE+'\m2-clipboard.ready';$release=$env:USERPROFILE+'\m2-clipboard.release'
foreach($p in @($ready,$release)){if(Test-Path $p){Remove-Item $p}}
$code=@'
Add-Type @"
using System;using System.Runtime.InteropServices;
public class ClipboardLease{[DllImport("user32.dll")]public static extern bool OpenClipboard(IntPtr h);[DllImport("user32.dll")]public static extern bool CloseClipboard();}
"@
if(![ClipboardLease]::OpenClipboard([IntPtr]::Zero)){throw 'clipboard lease failed'}
try{[IO.File]::WriteAllText($env:USERPROFILE+'\m2-clipboard.ready','held');$end=[DateTime]::UtcNow.AddSeconds(20);while(!(Test-Path ($env:USERPROFILE+'\m2-clipboard.release'))-and[DateTime]::UtcNow-lt$end){Start-Sleep -Milliseconds 100}}finally{$null=[ClipboardLease]::CloseClipboard()}
'@
Restart-Wav $sample;Activate-LabEditor $a;$editor.SetFocus();Send-DictationCommand 9;Start-Sleep -Seconds 8
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code));$lease=Start-Process powershell.exe -WindowStyle Hidden -ArgumentList @('-NoProfile','-EncodedCommand',$encoded) -PassThru
try{
 for($i=0;$i-lt50-and!(Test-Path $ready);$i++){Start-Sleep -Milliseconds 100};if(!(Test-Path $ready)){throw 'clipboard lease not ready'}
 Activate-LabEditor $a;$editor.SetFocus();Send-DictationCommand 9;Start-Sleep -Seconds 3
 $phase=Get-DictationPhase;$before=Read-Editor $editor;$steps+=Get-LabStep 'clipboard-contention-shows-not-pasted'
}finally{[IO.File]::WriteAllText($release,'release');$lease.WaitForExit(5000)|Out-Null}
Activate-LabEditor $a;$editor.SetFocus();$overlay=[LabDriver]::ByClass('KoettOverlayWindow');$null=Invoke-LabButton $overlay 'Paste';Start-Sleep -Milliseconds 500
$text=Read-Editor $editor;$clip=[Windows.Forms.Clipboard]::GetText()
$results+=@{case='clipboard-contention';passed=($phase-match'Not pasted'-and$before.Trim().Length-eq0-and$text.TrimEnd([char[]]"`r`n")-ceq$clip-and$clip-match'Phoebe');phase=$phase;recovered_text=$text;lease_exited=$lease.HasExited}
$steps+=Get-LabStep 'clipboard-release-manual-paste-once'
@{passed=(@($results|Where-Object {!$_.passed}).Count-eq0);cases=$results;artifact=Get-LabArtifact;steps=$steps}
