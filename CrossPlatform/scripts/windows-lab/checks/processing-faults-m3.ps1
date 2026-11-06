$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$hash=(Get-FileHash $config).Hash
$h=[LabDriver]::ByTitle('Koett Model and Formatting');$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function MC([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Choose-V2{
 Activate-LabEditor $h;$c=MC '60';$e=$c.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern);$e.Expand();Start-Sleep -Milliseconds 100
 $items=$c.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
 $items[1].GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();$e.Collapse()
}
function Wait-Model{
 for($i=0;$i-lt120;$i++){Start-Sleep -Milliseconds 400;if((MC '61').Current.IsEnabled){return (MC '62').Current.Name}}
 throw 'Model operation did not finish'
}
$cases=@();$steps=@()
try{
 Choose-V2;(Get-Item $config).IsReadOnly=$true;$null=Invoke-LabButton $h 'Switch model';Start-Sleep -Milliseconds 500;$status=Wait-Model
 $cases+=@{case='model-settings-denied-preserves-working-selection';passed=($hash-eq(Get-FileHash $config).Hash-and$status-match'save|denied');status=$status}
 $steps+=Get-LabWindowStep 'model-settings-denied-rollback' $h;(Get-Item $config).IsReadOnly=$false
 Choose-V2;$null=Invoke-LabButton $h 'Switch model';Start-Sleep -Milliseconds 100;$null=Invoke-LabButton $h 'Cancel setup';Start-Sleep -Milliseconds 500;$status=Wait-Model
 $cases+=@{case='model-cancel-preserves-working-selection';passed=($hash-eq(Get-FileHash $config).Hash-and$status-match'cancel');status=$status}
 $steps+=Get-LabWindowStep 'model-cancel-rollback' $h
}finally{(Get-Item $config).IsReadOnly=$false;[IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0);cases=$cases;artifact=Get-LabArtifact;steps=$steps}
