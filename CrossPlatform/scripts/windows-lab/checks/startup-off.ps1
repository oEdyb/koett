
$ErrorActionPreference='Stop'
Set-LabPhase 8
$settings=[LabDriver]::ByTitle('Koett Settings');[LabNative]::SetForegroundWindow($settings)|Out-Null
$root=[Windows.Automation.AutomationElement]::FromHandle($settings)
$check=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'21'))
$toggle=$check.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
if($toggle.Current.ToggleState-eq[Windows.Automation.ToggleState]::On){$toggle.Toggle()}
$save=Invoke-LabButton $settings 'Save'
$value=(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Koett -ErrorAction SilentlyContinue).Koett
@{checkbox=$toggle.Current.ToggleState.ToString();registry=$value;save=$save;steps=@((Get-LabWindowStep 'startup-off-before-reboot-e63' $settings));processes=@(Get-Process Koett|Select-Object Id,Path)}
