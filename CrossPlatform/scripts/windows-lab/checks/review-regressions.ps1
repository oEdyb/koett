$ErrorActionPreference='Stop'
if(-not ('ReviewInput' -as [type])){Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public class ReviewInput {
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,StringBuilder s);
 [DllImport("user32.dll")]public static extern IntPtr GetDlgItem(IntPtr h,int id);
 [DllImport("user32.dll")]static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
 public static void Control(bool down){keybd_event(0x11,0,down?0u:2u,UIntPtr.Zero);}
 public static string Read(IntPtr h){var s=new StringBuilder(4096);SendMessage(h,0xD,(IntPtr)4096,s);return s.ToString();}
 public static void Clear(IntPtr h){SendMessage(h,0xC,IntPtr.Zero,new StringBuilder());}
}
'@}
function New-ReviewEditor([string]$title){
 $h=[LabDriver]::ByTitle($title)
 if($h-eq[IntPtr]::Zero){
  $code='Add-Type -AssemblyName System.Windows.Forms;$f=New-Object Windows.Forms.Form;$f.Text="'+$title+'";$f.Width=700;$f.Height=300;$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock="Fill";$f.Controls.Add($t);$f.Add_Shown({$f.Activate();$t.Focus()});[void]$f.ShowDialog()'
  $encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code))
  $null=Start-Process powershell.exe -ArgumentList @('-NoProfile','-STA','-EncodedCommand',$encoded)
  for($i=0;$i-lt40-and$h-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 200;$h=[LabDriver]::ByTitle($title)}
 }
 if($h-eq[IntPtr]::Zero){throw 'Review editor did not open'}
 $h
}
$a=New-ReviewEditor 'Koett Review Original'
$b=New-ReviewEditor 'Koett Review Second'
$editA=[LabDriver]::GetWindow($a,5);$editB=[LabDriver]::GetWindow($b,5)
[ReviewInput]::Clear($editA);[ReviewInput]::Clear($editB)
Activate-LabEditor $a
[LabDriver]::SetCursorPos(20,20)|Out-Null
Set-LabPhase 1
$overlay=[LabDriver]::ByClass('KoettOverlayWindow')
$root=[Windows.Automation.AutomationElement]::FromHandle($overlay)
$names=@($root.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$unhovered=@('Cancel','Pause','History'|Where-Object {$_-notin$names}).Count-eq0
$pauseWorks=$false;$historyWorks=$false;$cancelWorks=$false
if($unhovered){
 $null=Invoke-LabButton $overlay 'Pause'
 $pauseWorks=([Windows.Automation.AutomationElement]::FromHandle($overlay).Current.Name-match'Paused')
 $null=Invoke-LabButton $overlay 'Resume'
 $null=Invoke-LabButton $overlay 'History'
 for($i=0;$i-lt40-and[LabDriver]::ByTitle('Koett History')-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 100}
 $historyWorks=([LabDriver]::ByTitle('Koett History')-ne[IntPtr]::Zero)
 $null=Invoke-LabButton $overlay 'Cancel'
 $cancelWorks=-not[LabDriver]::IsWindowVisible($overlay)
}
$steps=@((Get-LabStep 'unhovered-actions'))
Activate-LabEditor $a
Set-LabPhase 1
Set-LabPhase 4
$paste=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Paste'))
if(!$paste){throw 'Paste missing'}
[ReviewInput]::Control($true)
try {
 $paste.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
 Start-Sleep -Milliseconds 80
 Activate-LabEditor $b
 $switched=Get-LabFocus
} finally { [ReviewInput]::Control($false) }
Start-Sleep -Milliseconds 600
$textA=[ReviewInput]::Read($editA);$textB=[ReviewInput]::Read($editB)
$raceRefused=($textA-eq''-and$textB-eq''-and(Get-LabFocus).hwnd-eq$b.ToInt64())
$steps+=Get-LabWindowStep 'modifier-wait-second-editor' $b
$steps+=Get-LabStep 'modifier-wait-recovery'
Set-LabPhase 9
Start-Sleep -Milliseconds 700
$history=[LabDriver]::ByTitle('Koett History')
$search=[ReviewInput]::GetDlgItem($history,10)
$searchRoot=[Windows.Automation.AutomationElement]::FromHandle($search)
$searchRoot.SetFocus();Start-Sleep -Milliseconds 150
$searchRoot.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('selftargetmarker')
$ownBefore=Get-LabFocus
if($ownBefore.pid-ne(Get-LabArtifact).pid){throw 'History was not foreground for own-process check'}
Set-LabPhase 1
Set-LabPhase 4
$null=Invoke-LabButton $overlay 'Paste'
Start-Sleep -Milliseconds 300
$selfText=[ReviewInput]::Read($search)
$selfRefused=$selfText-ceq'selftargetmarker'
$steps+=Get-LabWindowStep 'history-search-refuses-self-paste' $history
$searchRoot.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Review pagination')
Start-Sleep -Milliseconds 700
$list=[ReviewInput]::GetDlgItem($history,11)
$countBefore=[LabDriver]::SendMessage($list,0x18B,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()
$next=[ReviewInput]::GetDlgItem($history,14)
$counts=@($countBefore)
if($next-ne[IntPtr]::Zero){
 for($i=0;$i-lt2;$i++){
  $null=Invoke-LabButton $history 'Load more'
  Start-Sleep -Milliseconds 600
  $counts+=[LabDriver]::SendMessage($list,0x18B,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()
 }
}
$pagination=($counts.Count-eq3-and$counts[0]-eq50-and$counts[1]-eq100-and$counts[2]-eq110)
$steps+=Get-LabWindowStep 'history-pagination' $history
@{passed=($unhovered-and$pauseWorks-and$historyWorks-and$cancelWorks-and$raceRefused-and$selfRefused-and$pagination);artifact=Get-LabArtifact;unhovered_names=$names;unhovered_actions=$unhovered;pause=$pauseWorks;history=$historyWorks;cancel=$cancelWorks;modifier_race_refused=$raceRefused;switched=$switched;original_text=$textA;second_text=$textB;self_paste_refused=$selfRefused;own_focus=$ownBefore;history_search_text=$selfText;history_counts=$counts;pagination=$pagination;steps=$steps}
