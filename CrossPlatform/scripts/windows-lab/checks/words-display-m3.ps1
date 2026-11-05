$ErrorActionPreference='Stop'
$theme='HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize';$prior=(Get-ItemProperty $theme).AppsUseLightTheme
$steps=[Collections.Generic.List[object]]::new();$cases=[Collections.Generic.List[object]]::new()
function DC([IntPtr]$w,[string]$id){[Windows.Automation.AutomationElement]::FromHandle($w).FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Scale-To([int]$percent){
 $desktop=[Windows.Automation.AutomationElement]::RootElement;$s=$desktop.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Settings'))
 $combo=$s.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'SystemSettings_Display_Scaling_ItemSizeOverride_ComboBox'))
 $expand=$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern);$expand.Expand();Start-Sleep -Milliseconds 150
 $choice=@($desktop.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.ControlType-eq[Windows.Automation.ControlType]::ListItem-and$_.Current.Name-match('^'+$percent+'%')})[0]
 if(!$choice){throw "Missing actual Windows scale $percent"};$choice.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 700
}
function Snapshot([string]$label,[IntPtr]$w,[int]$width,[int]$height,[int]$percent,[int]$light){
 Activate-LabEditor $w;$null=[LabDriver]::PostMessage($w,0x31A,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 250
 $r=[LabDriver]::RectOf($w);$dpi=[LabDriver]::GetDpiForWindow($w)
 $cases.Add(@{case=$label;percent=$percent;light=$light;dpi=$dpi;rect=$r;passed=($dpi-eq96*$percent/100-and$r[2]-$r[0]-eq$width*$percent/100-and$r[3]-$r[1]-eq$height*$percent/100)})
 $steps.Add((Get-LabWindowStep ($label+'-'+$percent+'-'+$(if($light){'light'}else{'dark'})) $w))
}
try{
 $null=Start-Process 'ms-settings:display';Start-Sleep -Seconds 2
 foreach($light in @(1,0)){
  Set-ItemProperty $theme -Name AppsUseLightTheme -Value $light
  foreach($percent in @(100,150,200)){
   Scale-To $percent
   Send-DictationCommand 10;Start-Sleep -Milliseconds 400;$words=[LabDriver]::ByTitle('Koett Words');Snapshot 'words' $words 960 680 $percent $light
   Send-DictationCommand 5;Start-Sleep -Milliseconds 300;$null=Invoke-LabButton ([LabDriver]::ByTitle('Koett Settings')) 'Model and formatting…';Start-Sleep -Milliseconds 300;$processing=[LabDriver]::ByTitle('Koett Model and Formatting');Snapshot 'processing' $processing 560 480 $percent $light
   Send-DictationCommand 2;Start-Sleep -Milliseconds 400;$history=[LabDriver]::ByTitle('Koett History');(DC $history '10').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('ask Grok about Superbase');Start-Sleep -Milliseconds 500
   $item=(DC $history '11').FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
   $item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 200;$null=Invoke-LabButton $history 'Fix a word…';Start-Sleep -Milliseconds 400
   $fix=[LabDriver]::ByTitle('Koett Fix a word');Snapshot 'correction' $fix 960 680 $percent $light
   $null=[LabDriver]::PostMessage($fix,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 200
  }
 }
}catch{throw ($_.Exception.Message+" at "+$_.ScriptStackTrace)}finally{
 Set-ItemProperty $theme -Name AppsUseLightTheme -Value $prior
 Scale-To 200
 foreach($title in @('Koett Words','Koett Model and Formatting','Koett History','Koett Settings')){$w=[LabDriver]::ByTitle($title);if($w-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($w,0x31A,[IntPtr]::Zero,[IntPtr]::Zero)}}
}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0-and$cases.Count-eq18);cases=$cases;theme_restored=$true;scale_restored=200;artifact=Get-LabArtifact;steps=$steps}
