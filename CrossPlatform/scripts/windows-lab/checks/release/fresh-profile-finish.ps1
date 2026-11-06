if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop';$r=Run-MediaCli 'history';$watch=[Diagnostics.Stopwatch]::StartNew();$items=@()
do {
 Start-Sleep -Milliseconds 100
 $h=[LabDriver]::ByTitle('Koett History')
 if($h-ne[IntPtr]::Zero){
  $w=[Windows.Automation.AutomationElement]::FromHandle($h)
  $condition=New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),('11')
  $list=$w.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
  if($list){
   $condition=New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)
   $items=@($list.FindAll([Windows.Automation.TreeScope]::Children,$condition))
  }
 }
}while($items.Count-ne1-and$watch.ElapsedMilliseconds-lt5000)

$names=@($items|ForEach-Object {$_.Current.Name});$step=Get-LabWindowStep 'fresh-history-one-record' $h;$null=Preview-Quit
$state=Get-Content ($global:PreviewDir+'\fresh-state.json') -Raw -Encoding UTF8|ConvertFrom-Json;$fresh=$global:PreviewDir+'\fresh-app-data';if(Test-Path $fresh){throw 'Fresh evidence destination exists'}
Move-Item -LiteralPath $state.base -Destination $fresh;Move-Item -LiteralPath $state.backup -Destination $state.base
if($state.startup){Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Koett -Value $state.startup}
$after=Preview-Data;$mismatch=@($state.before.PSObject.Properties|Where-Object {$after[$_.Name]-cne$_.Value})
@{passed=($names.Count-eq1-and$mismatch.Count-eq0);history_rows=$names;original_profile_restored=($mismatch.Count-eq0);profile_after=$after;fresh_data_preserved=$fresh;startup_restored=$state.startup;step=$step}
