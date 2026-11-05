$ErrorActionPreference='Stop'
$already=$null-ne(Get-Process Narrator -ErrorAction SilentlyContinue)
$cases=[Collections.Generic.List[object]]::new();$steps=[Collections.Generic.List[object]]::new()
function KC([IntPtr]$w,[string]$id){[Windows.Automation.AutomationElement]::FromHandle($w).FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Tabs([string]$name,[IntPtr]$w,[string[]]$required){
 Activate-LabEditor $w;$seen=@()
 for($i=0;$i-lt22;$i++){[Windows.Forms.SendKeys]::SendWait('{TAB}');Start-Sleep -Milliseconds 80;$e=[Windows.Automation.AutomationElement]::FocusedElement;$seen+=@{id=$e.Current.AutomationId;name=$e.Current.Name;type=$e.Current.ControlType.ProgrammaticName}}
 $ids=@($seen|ForEach-Object id);$missing=@($required|Where-Object {$_-notin$ids})
 $cases.Add(@{case=$name;passed=($missing.Count-eq0);missing=$missing;navigation=$seen})
 $steps.Add((Get-LabWindowStep $name $w))
}
try{
 if(!$already){$null=Start-Process Narrator.exe;Start-Sleep -Seconds 2}
 foreach($title in @('Narrator updates','Narrator Home')){$w=[LabDriver]::ByTitle($title);if($w-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($w,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}}
 Send-DictationCommand 10;Start-Sleep -Milliseconds 500;Tabs 'narrator-words-keyboard' ([LabDriver]::ByTitle('Koett Words')) @('40','11','41','42','43','44','50','45','46')
 Send-DictationCommand 5;Start-Sleep -Milliseconds 300;$null=Invoke-LabButton ([LabDriver]::ByTitle('Koett Settings')) 'Model and formatting…';Start-Sleep -Milliseconds 400;$p=[LabDriver]::ByTitle('Koett Model and Formatting')
 Tabs 'narrator-processing-keyboard' $p @('60','61','63','64','65','66','67')
 $labels=@();foreach($pair in @(@('60','Speech model'),@('64','Style'),@('65','Structure'),@('66','Context'))){$name=(KC $p $pair[0]).Current.Name;$labels+=@{id=$pair[0];name=$name;expected=$pair[1];passed=($name-ceq$pair[1])}}
 $cases.Add(@{case='native-model-and-formatting-accessible-labels';passed=(@($labels|Where-Object {!$_.passed}).Count-eq0);labels=$labels})
 Send-DictationCommand 2;Start-Sleep -Milliseconds 400;$h=[LabDriver]::ByTitle('Koett History');(KC $h '10').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('ask Grok about Superbase');Start-Sleep -Milliseconds 500
 $item=(KC $h '11').FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
 $item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 200;$null=Invoke-LabButton $h 'Fix a word…';Start-Sleep -Milliseconds 500;$fix=[LabDriver]::ByTitle('Koett Fix a word')
 Tabs 'narrator-correction-keyboard' $fix @('85','86','80','11')
 $running=$null-ne(Get-Process Narrator -ErrorAction SilentlyContinue)
}finally{
 $fix=[LabDriver]::ByTitle('Koett Fix a word');if($fix-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($fix,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}
 if(!$already){Get-Process Narrator -ErrorAction SilentlyContinue|Stop-Process}
}
@{passed=($running-and@($cases|Where-Object {!$_.passed}).Count-eq0);narrator_running_during_navigation=$running;scope='Native keyboard/UIA exposure with Narrator running; spoken pronunciation is not independently transcribed';cases=$cases;artifact=Get-LabArtifact;steps=$steps}
