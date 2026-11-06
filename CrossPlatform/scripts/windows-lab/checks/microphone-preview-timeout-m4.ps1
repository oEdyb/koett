$ErrorActionPreference='Stop'
$artifact=Get-LabArtifact
$command=(Get-CimInstance Win32_Process -Filter ('ProcessId='+$artifact.pid)).CommandLine
if($command-notmatch'--microphone-open-timeout-lab'){throw 'Launch the forced microphone timeout lab first'}
Send-DictationCommand 5;Start-Sleep -Milliseconds 200
$window=[LabDriver]::ByTitle('Koett Settings');Activate-LabEditor $window
$root=[Windows.Automation.AutomationElement]::FromHandle($window)
function Status-Text {
 @($root.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::Text)))|ForEach-Object {[string]$_.Current.Name})
}
function Workers {
 @(Get-CimInstance Win32_Process -Filter "name='Koett.exe'"|Where-Object {$_.CommandLine-match'windows-microphone-worker'}|ForEach-Object {[int]$_.ProcessId})
}
$enumeration=[Diagnostics.Stopwatch]::StartNew()
do{Start-Sleep -Milliseconds 100;$text=Status-Text}while($enumeration.ElapsedMilliseconds-lt8500-and!($text-match'timed out'))
$enumerationResult=@{elapsed_ms=$enumeration.ElapsedMilliseconds;text=$text;workers=@(Workers);focus=Get-LabFocus}
$rounds=@()
foreach($round in 1..2){
 $clock=[Diagnostics.Stopwatch]::StartNew();$null=Invoke-LabButton $window 'Preview'
 if($round-eq1){foreach($extra in 1..3){$null=[LabDriver]::PostMessage($window,0x111,[IntPtr]32,[IntPtr]::Zero)}}
 $seen=@();$pending=$false;$disabled=$false
 do{
  Start-Sleep -Milliseconds 100;$workers=@(Workers);foreach($worker in $workers){if($worker-notin$seen){$seen+=$worker}}
  $text=Status-Text;$pending=$pending-or[bool]($text-match'Opening microphone')
  $button=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'32'))
  $disabled=$disabled-or(-not$button.Current.IsEnabled)
 }while($clock.ElapsedMilliseconds-lt8500-and($seen.Count-eq0-or-not$button.Current.IsEnabled-or$workers.Count-gt0-or!($text-match'timed out')))
 $rounds+=@{round=$round;elapsed_ms=$clock.ElapsedMilliseconds;pending_visible=$pending;pending_disabled=$disabled;worker_pids=$seen;remaining_workers=$workers;text=$text;focus=Get-LabFocus;passed=($pending-and$disabled-and$seen.Count-eq1-and$workers.Count-eq0-and[bool]($text-match'timed out')-and$clock.ElapsedMilliseconds-lt7500)}
}
$null=[LabDriver]::PostMessage($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
@{passed=([bool]($enumerationResult.text-match'timed out')-and$enumerationResult.workers.Count-eq0-and@($rounds|Where-Object {-not$_.passed}).Count-eq0);artifact=$artifact;enumeration=$enumerationResult;preview=$rounds;scope='Forced worker stalls; no physical microphone.'}
