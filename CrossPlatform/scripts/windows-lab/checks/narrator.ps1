
$ErrorActionPreference='Stop'
foreach($title in @('Narrator updates','Narrator Home','Koett Focus Target')){$w=[LabDriver]::ByTitle($title);if($w-ne[IntPtr]::Zero){[LabDriver]::PostMessage($w,0x10,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null}}
Set-LabPhase 8
Start-Sleep -Milliseconds 700
$settings=[LabDriver]::ByTitle('Koett Settings');$root=[Windows.Automation.AutomationElement]::FromHandle($settings)
[LabNative]::SetForegroundWindow($settings)|Out-Null
$checkbox=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'21'))
$checkbox.SetFocus();Start-Sleep -Milliseconds 800
$navigation=@()
for($i=0;$i-lt 4;$i++){
 $e=[Windows.Automation.AutomationElement]::FocusedElement
 $navigation+=@{name=$e.Current.Name;type=$e.Current.ControlType.ProgrammaticName;focus=Get-LabFocus}
 [Windows.Forms.SendKeys]::SendWait('{TAB}');Start-Sleep -Milliseconds 700
}
@{artifact='e63e65a';narrator=@(Get-Process Narrator -ErrorAction SilentlyContinue|Select-Object Id,ProcessName);keyboard=$navigation;steps=@((Get-LabWindowStep 'narrator-settings-200' $settings))}
