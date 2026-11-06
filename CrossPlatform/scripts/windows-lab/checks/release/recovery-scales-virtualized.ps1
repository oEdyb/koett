if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
function Recovery-Item([string]$id){$w=[Windows.Automation.AutomationElement]::FromHandle([LabDriver]::ByClass('KoettRecoveryWindow'));$w.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,$id)))}
function Recovery-Step([string]$name){Get-LabWindowStep $name ([LabDriver]::ByClass('KoettRecoveryWindow'))}
$r=Run-MediaCli 'recovery';Start-Sleep -Milliseconds 300
$ErrorActionPreference='Stop'
$desktop=[Windows.Automation.AutomationElement]::RootElement
$settings=$desktop.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Settings')));if(-not$settings){throw 'Windows Display Settings missing'}
$display=$settings.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,'Display1')));if($display){([Windows.Automation.SelectionItemPattern]$display.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()}
function Set-MediaScale([int]$percent){
 $settings=$desktop.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Settings')))
 Activate-LabEditor ([IntPtr]$settings.Current.NativeWindowHandle)
 $combo=$settings.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,'SystemSettings_Display_Scaling_ItemSizeOverride_ComboBox')))
 $expand=[Windows.Automation.ExpandCollapsePattern]$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern);if($expand.Current.ExpandCollapseState-ne[Windows.Automation.ExpandCollapseState]::Expanded){$expand.Expand()};Start-Sleep -Milliseconds 200
 $container=[Windows.Automation.ItemContainerPattern]$combo.GetCurrentPattern([Windows.Automation.ItemContainerPattern]::Pattern)
 $choice=$container.FindItemByProperty($null,[Windows.Automation.AutomationElement]::NameProperty,($percent.ToString()+'%'))
 if(-not$choice){$choice=$container.FindItemByProperty($null,[Windows.Automation.AutomationElement]::NameProperty,($percent.ToString()+'% (Recommended)'))}
 if(-not$choice){throw ('Scale does not exist: '+$percent)}
 $virtual=$null;if($choice.TryGetCurrentPattern([Windows.Automation.VirtualizedItemPattern]::Pattern,[ref]$virtual)){([Windows.Automation.VirtualizedItemPattern]$virtual).Realize()}
 ([Windows.Automation.SelectionItemPattern]$choice.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)).Select()
 $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$selected=([Windows.Automation.SelectionPattern]$combo.GetCurrentPattern([Windows.Automation.SelectionPattern]::Pattern)).Current.GetSelection()[0].Current.Name}while($selected-notmatch('^'+$percent+'%')-and$watch.ElapsedMilliseconds-lt5000)
 if($selected-notmatch('^'+$percent+'%')){throw ('Scale selection differs: '+$selected+' expected '+$percent)}
 $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$dpi=[LabDriver]::GetDpiForWindow([LabDriver]::ByClass('KoettRecoveryWindow'))}while($dpi-ne96*$percent/100-and$watch.ElapsedMilliseconds-lt5000)
 if($dpi-ne96*$percent/100){throw ('DPI did not change to '+$percent+': '+$dpi)}
}
$themePath='HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize';$original=(Get-ItemProperty $themePath).AppsUseLightTheme;$steps=@();$results=@()
try{
 foreach($percent in @(100,150,200)){
  Set-MediaScale $percent
  foreach($light in @(0,1)){
   Set-ItemProperty $themePath -Name AppsUseLightTheme -Value $light
   [LabDriver]::PostMessage([LabDriver]::ByClass('KoettRecoveryWindow'),0x31A,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
   (Recovery-Item '102').SetFocus();Start-Sleep -Milliseconds 200
   $source=(Recovery-Item '102').Current.BoundingRectangle;$button=(Recovery-Item '3').Current.BoundingRectangle;$copy=(Recovery-Item '1').Current.BoundingRectangle
   $outer=[LabDriver]::RectOf([LabDriver]::ByClass('KoettRecoveryWindow'));$ok=[Math]::Abs($button.Height-32*$percent/100)-le1-and[Math]::Abs($button.Height-32*$percent/100)-le1-and$copy.Right-lt$outer[2]-and[Math]::Abs(($outer[2]-$outer[0])-600*$percent/100)-le1-and[Math]::Abs(($outer[3]-$outer[1])-420*$percent/100)-le1
   $results+=@{scale=$percent;light=$light;dpi=[LabDriver]::GetDpiForWindow([LabDriver]::ByClass('KoettRecoveryWindow'));passed=[bool]$ok;source=$source.ToString();transcribe=$button.ToString();focus=Get-LabFocus}
   $steps+=Recovery-Step ('media-'+$percent+'-theme-'+$light)
  }
 }
}finally{
 Set-ItemProperty $themePath -Name AppsUseLightTheme -Value $original
 Set-MediaScale 175
 [LabDriver]::PostMessage([LabDriver]::ByClass('KoettRecoveryWindow'),0x31A,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
}
@{passed=(@($results|Where-Object {-not$_.passed}).Count-eq0);checks=$results;steps=$steps;restored_scale=175;restored_theme=$original;artifact=Get-LabArtifact}
