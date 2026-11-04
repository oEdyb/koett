$ErrorActionPreference='Stop'
$desktop=[Windows.Automation.AutomationElement]::RootElement
$steps=@();$checks=@()
foreach($percent in @(100,150,200)){
 $settings=$desktop.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Settings'))
 $combo=$settings.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'SystemSettings_Display_Scaling_ItemSizeOverride_ComboBox'))
 $expand=$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern)
 if($expand.Current.ExpandCollapseState-ne[Windows.Automation.ExpandCollapseState]::Expanded){$expand.Expand()}
 Start-Sleep -Milliseconds 200
 $choice=$null
 foreach($e in $desktop.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)){
  if($e.Current.ControlType-eq[Windows.Automation.ControlType]::ListItem-and$e.Current.Name-match('^'+$percent+'%')){$choice=$e;break}
 }
 if(!$choice){throw "Scale $percent not available"}
 $choice.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select()
 Start-Sleep -Milliseconds 800
 [LabDriver]::SetCursorPos(20,20)|Out-Null
 Set-LabPhase 1
 $r=Get-PillRect;$checks+=@{percent=$percent;rect=$r;expected_width=300*$percent/100;expected_height=48*$percent/100;matches=(($r.right-$r.left)-eq300*$percent/100-and($r.bottom-$r.top)-eq48*$percent/100)}
 $steps+=Get-LabStep ('scale-'+$percent+'-visible-e63')
}
@{checks=$checks;steps=$steps}
