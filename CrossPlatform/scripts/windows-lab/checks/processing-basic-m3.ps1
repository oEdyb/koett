$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$hash=(Get-FileHash $config).Hash
$h=[LabDriver]::ByTitle('Koett Model and Formatting');if($h-eq[IntPtr]::Zero){throw 'Open the live engine Model and Formatting window'}
$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function Processing-Control([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Choose-Processing([string]$id,[string]$name){
 $combo=Processing-Control $id;$expand=$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern);$expand.Expand();Start-Sleep -Milliseconds 100
 $condition=New-Object Windows.Automation.AndCondition (New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),$name),(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem))
 $item=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition);if(!$item){throw "Choice missing: $name"};$item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();$expand.Collapse();Start-Sleep -Milliseconds 150
}
$cases=@();$steps=@()
try{
 Activate-LabEditor $h
 $enabled=(Processing-Control '63').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
 if($enabled.Current.ToggleState-eq[Windows.Automation.ToggleState]::On){$enabled.Toggle()}
 Choose-Processing '64' 'Formal';Choose-Processing '65' 'Lists';Choose-Processing '66' 'Email'
 $null=Invoke-LabButton $h 'Apply formatting';Start-Sleep -Milliseconds 700
 $saved=[IO.File]::ReadAllText($config)|ConvertFrom-Json
 $cases+=@{case='raw-mode-can-save-all-formatting-options-without-preparation';passed=(!$saved.formatting.enabled-and$saved.formatting.style-eq'formal'-and$saved.formatting.structure-eq'lists'-and$saved.formatting.context-eq'email');formatting=$saved.formatting}
 $steps+=Get-LabWindowStep 'raw-mode-formatting-controls-persist' $h
 $model=$saved.selected_model
 Send-DictationCommand 9;Start-Sleep -Milliseconds 600
 $during=Get-DictationPhase;$null=Invoke-LabButton $h 'Switch model';Start-Sleep -Milliseconds 400
 $status=(Processing-Control '62').Current.Name;$after=[IO.File]::ReadAllText($config)|ConvertFrom-Json
 Send-DictationCommand 8;Start-Sleep -Milliseconds 400
 $cases+=@{case='model-change-during-recording-refused-with-visible-status';passed=($during-match'Recording'-and$status-match'Stop recording'-and$after.selected_model-eq$model);phase=$during;status=$status;model_before=$model;model_after=$after.selected_model}
 $steps+=Get-LabWindowStep 'recording-model-change-visible-refusal' $h
}finally{[IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0-and(Get-FileHash $config).Hash-eq$hash);cases=$cases;settings_restored=((Get-FileHash $config).Hash-eq$hash);artifact=Get-LabArtifact;steps=$steps}
