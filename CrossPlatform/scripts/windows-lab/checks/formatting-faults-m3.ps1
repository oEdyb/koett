$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$hash=(Get-FileHash $config).Hash
$h=[LabDriver]::ByTitle('Koett Model and Formatting');$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function Format-Control([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Enable-Format{$t=(Format-Control '63').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern);if($t.Current.ToggleState-ne[Windows.Automation.ToggleState]::On){$t.Toggle()};$null=Invoke-LabButton $h 'Apply formatting'}
function Wait-Format{for($i=0;$i-lt100;$i++){Start-Sleep -Milliseconds 200;if((Format-Control '67').Current.IsEnabled){return (Format-Control '68').Current.Name}};throw 'Formatting operation did not finish'}
$cases=@();$steps=@()
try{
 (Get-Item $config).IsReadOnly=$true;Enable-Format;Start-Sleep -Milliseconds 300;$status=Wait-Format
 $children=@(Get-CimInstance Win32_Process -Filter "name='llama-server.exe'"|Where-Object {$_.ParentProcessId-eq(Get-LabArtifact).pid})
 $cases+=@{case='formatter-settings-denial-preserves-raw-and-reaps-candidate';passed=($hash-eq(Get-FileHash $config).Hash-and$status-match'denied'-and$children.Count-eq0);status=$status;children=$children.Count}
 $steps+=Get-LabWindowStep 'formatter-settings-denial-keeps-raw' $h;(Get-Item $config).IsReadOnly=$false
 Enable-Format;$pre='';for($i=0;$i-lt40;$i++){Start-Sleep -Milliseconds 30;$pre=(Format-Control '68').Current.Name;if($pre-match'Checking|Loading|ready'){break}}
 $null=Invoke-LabButton $h 'Cancel setup';Start-Sleep -Milliseconds 300;$status=Wait-Format
 $children=@(Get-CimInstance Win32_Process -Filter "name='llama-server.exe'"|Where-Object {$_.ParentProcessId-eq(Get-LabArtifact).pid})
 $cases+=@{case='formatter-cancel-preserves-raw-and-reaps-candidate';passed=($pre-notmatch'ready'-and$hash-eq(Get-FileHash $config).Hash-and$status-match'cancel'-and$children.Count-eq0);before_cancel=$pre;status=$status;children=$children.Count}
 $steps+=Get-LabWindowStep 'formatter-cancel-keeps-raw' $h
}finally{(Get-Item $config).IsReadOnly=$false;[IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0);cases=$cases;artifact=Get-LabArtifact;steps=$steps}
