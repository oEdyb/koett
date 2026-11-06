if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
function Set-LabOption([string]$label,[string]$property,[bool]$wanted,[bool]$default){
 $config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$value=(Get-Content $config -Raw -Encoding UTF8|ConvertFrom-Json).$property;if($null-eq$value){$value=$default}
 if([bool]$value-ne$wanted){
  $null=Run-MediaCli 'settings';Start-Sleep -Milliseconds 300;$h=[LabDriver]::ByTitle('Koett Settings');Activate-LabEditor $h;$null=[LabDriver]::PostMessage($h,0x111,[IntPtr]43,[IntPtr]::Zero);Start-Sleep -Milliseconds 200
  $root=[Windows.Automation.AutomationElement]::RootElement;$menu=$root.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ClassNameProperty,'#32768')));$item=$menu.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,$label)));([Windows.Automation.InvokePattern]$item.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke();Start-Sleep -Milliseconds 200
  $null=[LabDriver]::PostMessage($h,0x111,[IntPtr]22,[IntPtr]::Zero);Start-Sleep -Milliseconds 200
 }
 $actual=(Get-Content $config -Raw -Encoding UTF8|ConvertFrom-Json).$property;if($null-eq$actual){$actual=$default};if([bool]$actual-ne$wanted){throw ('Option not saved: '+$property)}
 @{name=$label;property=$property;actual=$actual;focus=Get-LabFocus}
}
