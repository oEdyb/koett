$ErrorActionPreference='Stop'
$desktop=[Windows.Automation.AutomationElement]::RootElement
$settings=$desktop.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Settings')));if(-not$settings){throw 'Windows Display Settings missing'}
$display=$settings.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,'Display1')));if($display){([Windows.Automation.SelectionItemPattern]$display.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()}
function Set-MediaScale([int]$percent){
 $settings=$desktop.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Settings')))
 Activate-LabEditor ([IntPtr]$settings.Current.NativeWindowHandle)
 $combo=$settings.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,'SystemSettings_Display_Scaling_ItemSizeOverride_ComboBox')))
 $expand=[Windows.Automation.ExpandCollapsePattern]$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern);if($expand.Current.ExpandCollapseState-ne[Windows.Automation.ExpandCollapseState]::Expanded){$expand.Expand()};Start-Sleep -Milliseconds 100
 $choice=@($settings.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::ListItem)))|Where-Object {$_.Current.Name-match('^'+$percent+'%')}|Select-Object -First 1)
 if($choice.Count-ne1){throw ('Scale missing: '+$percent)}
 ([Windows.Automation.SelectionItemPattern]$choice[0].GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()
 $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$dpi=[LabDriver]::GetDpiForWindow([LabDriver]::ByClass('KoettMediaWindow'))}while($dpi-ne96*$percent/100-and$watch.ElapsedMilliseconds-lt5000)
 if($dpi-ne96*$percent/100){throw ('DPI did not change to '+$percent+': '+$dpi)}
}
$themePath='HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize';$original=(Get-ItemProperty $themePath).AppsUseLightTheme;$steps=@();$results=@()
try{
 foreach($percent in @(100,150,200)){
  Set-MediaScale $percent
  foreach($light in @(0,1)){
   Set-ItemProperty $themePath -Name AppsUseLightTheme -Value $light
   [LabDriver]::PostMessage([LabDriver]::ByClass('KoettMediaWindow'),0x31A,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
   (Media-Item '101').SetFocus();Start-Sleep -Milliseconds 200
   $source=(Media-Item '101').Current.BoundingRectangle;$button=(Media-Item '1').Current.BoundingRectangle;$copy=(Media-Item '104').Current.BoundingRectangle
   $outer=[LabDriver]::RectOf([LabDriver]::ByClass('KoettMediaWindow'));$ok=[Math]::Abs($source.Height-32*$percent/100)-le1-and[Math]::Abs($button.Height-32*$percent/100)-le1-and$copy.Right-lt$outer[2]-and[Math]::Abs(($outer[2]-$outer[0])-680*$percent/100)-le1-and[Math]::Abs(($outer[3]-$outer[1])-500*$percent/100)-le1
   $results+=@{scale=$percent;light=$light;dpi=[LabDriver]::GetDpiForWindow([LabDriver]::ByClass('KoettMediaWindow'));passed=[bool]$ok;source=$source.ToString();transcribe=$button.ToString();focus=Get-LabFocus}
   $steps+=Media-Step ('media-'+$percent+'-theme-'+$light)
  }
 }
}finally{
 Set-ItemProperty $themePath -Name AppsUseLightTheme -Value $original
 Set-MediaScale 175
 [LabDriver]::PostMessage([LabDriver]::ByClass('KoettMediaWindow'),0x31A,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
}
@{passed=(@($results|Where-Object {-not$_.passed}).Count-eq0);checks=$results;steps=$steps;restored_scale=175;restored_theme=$original;artifact=Get-LabArtifact}
