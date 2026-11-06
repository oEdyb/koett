$ErrorActionPreference='Stop'
Set-LabPhase 8
Start-Sleep -Milliseconds 700
$settings=[LabDriver]::ByTitle('Koett Settings')
if($settings-eq[IntPtr]::Zero){throw 'Settings did not open'}
$root=[Windows.Automation.AutomationElement]::FromHandle($settings)
$nodes=@()
foreach($id in @('30','31','32','22')){
 $node=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))
 if(!$node){throw "Microphone Settings control missing: $id"}
 $nodes+=@{id=$id;name=$node.Current.Name;type=$node.Current.ControlType.ProgrammaticName;enabled=$node.Current.IsEnabled}
}
$null=Invoke-LabButton $settings 'Refresh'
Start-Sleep -Milliseconds 700
$steps=@((Get-LabWindowStep 'microphone-settings-refreshed' $settings))
$null=Invoke-LabButton $settings 'Preview'
Start-Sleep -Seconds 1
$names=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$steps+=Get-LabWindowStep 'microphone-preview-or-visible-input-error' $settings
$stop=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Stop preview'))
if($stop){$null=Invoke-LabButton $settings 'Stop preview'}
$steps+=Get-LabWindowStep 'microphone-preview-stopped' $settings
@{passed=($nodes.Count-eq4-and$nodes[0].type-eq'ControlType.ComboBox');artifact=Get-LabArtifact;controls=$nodes;preview_names=$names;scope='Native control/UI/error smoke only; no proof of real microphone samples, persistence, or recording contention';steps=$steps}
