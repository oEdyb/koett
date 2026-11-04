$ErrorActionPreference='Stop'
if(-not ('LabEdit' -as [type])){Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public class LabEdit { [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,StringBuilder s);public static string Read(IntPtr h){var s=new StringBuilder(4096);SendMessage(h,0xD,(IntPtr)4096,s);return s.ToString();}}
'@
}
$targetCode='Add-Type -AssemblyName System.Windows.Forms;$f=New-Object Windows.Forms.Form;$f.Text="Koett Focus Target";$f.Width=700;$f.Height=300;$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock="Fill";$f.Controls.Add($t);$f.Add_Shown({$f.Activate();$t.Focus()});[void]$f.ShowDialog()'
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($targetCode))
$target=[LabDriver]::ByTitle('Koett Focus Target')
$process=$null
if($target -eq [IntPtr]::Zero){$process=Start-Process powershell.exe -ArgumentList @('-NoProfile','-STA','-EncodedCommand',$encoded) -PassThru}
for($i=0;$i -lt 40 -and $target -eq [IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 500;$target=[LabDriver]::ByTitle('Koett Focus Target')}
if($target -eq [IntPtr]::Zero){throw 'Typing target did not open'}
Activate-LabEditor $target
[LabDriver]::SetCursorPos(20,20)|Out-Null
Start-Sleep -Milliseconds 300
$before=Get-LabFocus

if(-not ('LabMouse' -as [type])){Add-Type @'
using System.Runtime.InteropServices;
public class LabMouse{[DllImport("user32.dll")]public static extern void mouse_event(uint f,uint x,uint y,uint d,System.UIntPtr e);public static void Click(){mouse_event(2,0,0,0,System.UIntPtr.Zero);mouse_event(4,0,0,0,System.UIntPtr.Zero);}}
'@}
function Click-Pill([int]$x){$r=Get-PillRect;$dpi=$r.dpi/96;[LabDriver]::SetCursorPos(($r.left+$x*$dpi),($r.top+24*$dpi))|Out-Null;Start-Sleep -Milliseconds 150;[LabMouse]::Click();Start-Sleep -Milliseconds 150}
$steps=@();$checks=@()
Set-LabPhase 1
$r=Get-PillRect;$dpi=$r.dpi/96
[LabDriver]::SetCursorPos(($r.left+120*$dpi),($r.top+24*$dpi))|Out-Null
Start-Sleep -Milliseconds 300
$steps+=Get-LabStep 'recording-hover-controls'
$overlay=[LabDriver]::ByClass('KoettOverlayWindow')
Click-Pill 135
$steps+=Get-LabStep 'pause-button-invoked'
Click-Pill 135
Click-Pill 65
$steps+=Get-LabStep 'cancel-button-hides'
[LabDriver]::SetCursorPos(20,20)|Out-Null
Set-LabPhase 3
[LabDriver]::SetCursorPos(($r.left+270*$dpi),($r.top+24*$dpi))|Out-Null
Start-Sleep -Milliseconds 2700
$steps+=Get-LabStep 'done-hover-holds'
[LabDriver]::SetCursorPos(20,20)|Out-Null
Start-Sleep -Milliseconds 1400
$steps+=Get-LabStep 'done-hover-exit-hides'
Set-LabPhase 3
Click-Pill 265
$copied=[Windows.Forms.Clipboard]::GetText()
$steps+=Get-LabStep 'done-copied'
Start-Sleep -Milliseconds 1300
$steps+=Get-LabStep 'copied-hides-after-second'
Activate-LabEditor $target
# Clear only the isolated Edit control; SendKeys can miss its child focus.
$null=[LabEdit]::SendMessage([LabDriver]::GetWindow($target,5),0xC,[IntPtr]::Zero,(New-Object Text.StringBuilder))
Start-Sleep -Milliseconds 100
$beforePasteText=[LabEdit]::Read([LabDriver]::GetWindow($target,5))
if($beforePasteText-ne''){throw 'The isolated editor did not clear before the Paste check'}
Set-LabPhase 1
Set-LabPhase 4
Start-Sleep -Milliseconds 2500
$steps+=Get-LabStep 'not-pasted-persists'
$beforePaste=Get-LabFocus
Click-Pill 210
Start-Sleep -Milliseconds 300
$text=[LabEdit]::Read([LabDriver]::GetWindow($target,5))
$steps+=Get-LabStep 'not-pasted-retry'
@{passed=($copied-eq'Desktop lab result'-and($text -ceq 'Desktop lab recovery')-and@($steps|Where-Object {$_.focus.hwnd-ne$before.hwnd-or$_.focus.pid-ne$before.pid}).Count-eq0);artifact=(Get-LabArtifact);interaction='real guest mouse_event';copied=$copied;editor_text_before_paste=$beforePasteText;exact_insert=($text -ceq 'Desktop lab recovery');pasted_editor_text=$text;before_paste=$beforePaste;steps=$steps}
