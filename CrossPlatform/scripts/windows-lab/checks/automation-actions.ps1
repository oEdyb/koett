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

$steps=@();$checks=@()
Set-LabPhase 1
$r=Get-PillRect;$dpi=$r.dpi/96
[LabDriver]::SetCursorPos(($r.left+120*$dpi),($r.top+24*$dpi))|Out-Null
Start-Sleep -Milliseconds 300
$steps+=Get-LabStep 'recording-hover-controls-e63'
$overlay=[LabDriver]::ByClass('KoettOverlayWindow')
$null=Invoke-LabButton $overlay 'Pause'
$steps+=Get-LabStep 'pause-button-invoked-e63'
$null=Invoke-LabButton $overlay 'Resume'
$null=Invoke-LabButton $overlay 'Cancel'
$steps+=Get-LabStep 'cancel-button-hides-e63'
[LabDriver]::SetCursorPos(20,20)|Out-Null
Set-LabPhase 3
[LabDriver]::SetCursorPos(($r.left+270*$dpi),($r.top+24*$dpi))|Out-Null
Start-Sleep -Milliseconds 2700
$steps+=Get-LabStep 'done-hover-holds-e63'
[LabDriver]::SetCursorPos(20,20)|Out-Null
Start-Sleep -Milliseconds 1100
$steps+=Get-LabStep 'done-hover-exit-hides-e63'
Set-LabPhase 3
$null=Invoke-LabButton $overlay 'Copy'
$copied=[Windows.Forms.Clipboard]::GetText()
$steps+=Get-LabStep 'done-copied-e63'
Start-Sleep -Milliseconds 1300
$steps+=Get-LabStep 'copied-hides-after-second-e63'
[LabNative]::SetForegroundWindow($target)|Out-Null
Set-LabPhase 1
Set-LabPhase 4
Start-Sleep -Milliseconds 2500
$steps+=Get-LabStep 'not-pasted-persists-e63'
$beforePaste=Get-LabFocus
$null=Invoke-LabButton $overlay 'Paste'
Start-Sleep -Milliseconds 300
if(-not ('LabEdit' -as [type])){Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public class LabEdit { [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,StringBuilder s);public static string Read(IntPtr h){var s=new StringBuilder(4096);SendMessage(h,0xD,(IntPtr)4096,s);return s.ToString();}}
'@
}
$text=[LabEdit]::Read([LabDriver]::GetWindow($target,5))
$steps+=Get-LabStep 'not-pasted-retry-e63'
@{artifact='e63e65a';copied=$copied;pasted_editor_text=$text;before_paste=$beforePaste;steps=$steps}
