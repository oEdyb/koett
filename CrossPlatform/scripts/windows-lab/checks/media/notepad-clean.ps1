$root=[Windows.Automation.AutomationElement]::RootElement
$button=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Keep for all')))
if($button){([Windows.Automation.InvokePattern]$button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke();Start-Sleep -Milliseconds 500}
@{dismissed=[bool]$button;focus=Get-LabFocus}
