function Assert-InstallerText([string]$Text){
 if($Text-match'(?i)"(?:ok|warnings|signature|preserved_files)"\s*:|unsigned.preview|formatter.*bundled'){throw ('Installer exposed machine output: '+$Text)}
}
function Read-InstallerText([Windows.Automation.AutomationElement]$Window){
 $text=@($Window.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})-join' '
 Assert-InstallerText $text
 $text
}
function Click-InstallerButton([Windows.Automation.AutomationElement]$Window,[string]$Name){
 $button=$Window.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,$Name)))
 if(-not$button-or-not$button.Current.IsEnabled){throw ('Installer button unavailable: '+$Name)}
 Activate-LabEditor ([IntPtr]$Window.Current.NativeWindowHandle)
 $r=$button.Current.BoundingRectangle;[LabDriver]::SetCursorPos([int]($r.X+$r.Width/2),[int]($r.Y+$r.Height/2))|Out-Null;[LabActivation]::Click()
}
function Wait-InstallerButton([IntPtr]$Handle,[string]$Name){
 $watch=[Diagnostics.Stopwatch]::StartNew()
 do{
  Start-Sleep -Milliseconds 150
  $root=[Windows.Automation.AutomationElement]::FromHandle($Handle);$text=Read-InstallerText $root
  $button=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,$Name)))
  if($button-and$button.Current.IsEnabled){return $root}
  $popup=[LabDriver]::GetWindow($Handle,6)
  if($popup-ne[IntPtr]::Zero-and$popup-ne$Handle){$dialog=[Windows.Automation.AutomationElement]::FromHandle($popup);$message=Read-InstallerText $dialog;if($message-match'Added or changed app files were kept in:'){Click-InstallerButton $dialog 'OK'}else{throw ('Unexpected installer message: '+$message)}}
 }while($watch.ElapsedMilliseconds-lt120000)
 throw ('Installer timed out waiting for '+$Name)
}
