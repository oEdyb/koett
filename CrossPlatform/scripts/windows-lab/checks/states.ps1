$ErrorActionPreference='Stop'
$targetCode='Add-Type -AssemblyName System.Windows.Forms;$f=New-Object Windows.Forms.Form;$f.Text="Koett Focus Target";$f.Width=700;$f.Height=300;$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock="Fill";$f.Controls.Add($t);$f.Add_Shown({$f.Activate();$t.Focus()});[void]$f.ShowDialog()'
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($targetCode))
$target=[LabDriver]::ByTitle('Koett Focus Target')
$process=$null
if($target -eq [IntPtr]::Zero){$process=Start-Process powershell.exe -ArgumentList @('-NoProfile','-STA','-EncodedCommand',$encoded) -PassThru}
for($i=0;$i -lt 40 -and $target -eq [IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 500;$target=[LabDriver]::ByTitle('Koett Focus Target')}
if($target -eq [IntPtr]::Zero){throw 'Typing target did not open'}
[LabNative]::SetForegroundWindow($target)|Out-Null
[LabDriver]::SetCursorPos(20,20)|Out-Null
Start-Sleep -Milliseconds 300
$before=Get-LabFocus
Set-LabPhase 1
$samples=@()
foreach($letter in 'Typing remains here.'.ToCharArray()){
 [Windows.Forms.SendKeys]::SendWait([string]$letter)
 $samples+=Get-LabFocus
 Start-Sleep -Milliseconds 15
}
if(-not ('LabEdit' -as [type])){Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public class LabEdit { [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,StringBuilder s);public static string Read(IntPtr h){var s=new StringBuilder(4096);SendMessage(h,0xD,(IntPtr)4096,s);return s.ToString();}}
'@
}
$text=[LabEdit]::Read([LabDriver]::GetWindow($target,5))
$record=Get-LabStep 'recording-typing'
$record.before=$before;$record.samples=$samples;$record.typed=$text
$record.focus_preserved=(@($samples|Where-Object {$_.hwnd -ne $before.hwnd -or $_.pid -ne $before.pid}).Count -eq 0)
Set-LabPhase 2
$transcribing=Get-LabStep 'transcribing'
Set-LabPhase 3
$done=Get-LabStep 'done'
Start-Sleep -Milliseconds 2100
$expired=Get-LabStep 'done-expired'
$steps=@($record,$transcribing,$done,$expired)
foreach($case in @(@(4,'not-pasted'),@(5,'rescue-error'),@(6,'transient-error'),@(1,'recording-again'),@(10,'paused'),@(11,'drawing'),@(12,'media-result'))){
 Set-LabPhase $case[0];$steps+=Get-LabStep $case[1]
}
@{steps=$steps;target=$before;artifact='e63e65a'}
