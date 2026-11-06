$ErrorActionPreference='Stop';$cases=@();$steps=@()
function BC([IntPtr]$w,[string]$id){[Windows.Automation.AutomationElement]::FromHandle($w).FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Back([IntPtr]$w,[string]$from,[string]$expected){
 (BC $w $from).SetFocus();[Windows.Forms.SendKeys]::SendWait('+{TAB}');Start-Sleep -Milliseconds 100;$e=[Windows.Automation.AutomationElement]::FocusedElement
 @{case=('shift-tab-'+$from+'-to-'+$expected);passed=($e.Current.AutomationId-ceq$expected);focus_id=$e.Current.AutomationId;focus_name=$e.Current.Name;focus=Get-LabFocus}
}
try{
 Send-DictationCommand 10;Start-Sleep -Milliseconds 400;$words=[LabDriver]::ByTitle('Koett Words');Activate-LabEditor $words
 $cases+=Back $words '43' '42';$cases+=Back $words '42' '41';$steps+=Get-LabWindowStep 'words-backtab-leaves-multiline-field' $words
 Send-DictationCommand 2;Start-Sleep -Milliseconds 400;$history=[LabDriver]::ByTitle('Koett History');(BC $history '10').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('ask Grok about Superbase');Start-Sleep -Milliseconds 500
 $item=(BC $history '11').FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
 $item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 200;$null=Invoke-LabButton $history 'Fix a word…';Start-Sleep -Milliseconds 400
 $fix=[LabDriver]::ByTitle('Koett Fix a word');Activate-LabEditor $fix;$cases+=Back $fix '80' '86';$cases+=Back $fix '86' '85';$steps+=Get-LabWindowStep 'correction-backtab-leaves-multiline-field' $fix
}finally{$fix=[LabDriver]::ByTitle('Koett Fix a word');if($fix-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($fix,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0);cases=$cases;artifact=Get-LabArtifact;steps=$steps}
