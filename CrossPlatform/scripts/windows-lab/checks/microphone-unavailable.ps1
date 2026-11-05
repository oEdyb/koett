$ErrorActionPreference='Stop'
$path=Join-Path $env:APPDATA 'DybergCo\Koett\config\settings.json'
$original=[IO.File]::ReadAllText($path)
$steps=@();$names=@();$rejected=$false
try {
 $settings=$original|ConvertFrom-Json
 $settings.microphone='wasapi:{0.0.1.00000000}.{00000000-0000-0000-0000-000000000000}'
 $h=[LabDriver]::ByTitle('Koett Settings')
 if($h-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 500}
 [IO.File]::WriteAllText($path,($settings|ConvertTo-Json -Depth 20),(New-Object Text.UTF8Encoding($false)))
 Set-LabPhase 8;Start-Sleep -Milliseconds 700
 $h=[LabDriver]::ByTitle('Koett Settings')
 $null=Invoke-LabButton $h 'Preview';Start-Sleep -Seconds 1
 $root=[Windows.Automation.AutomationElement]::FromHandle($h)
 $names=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
 $rejected=(@($names|Where-Object {$_-match'Selected microphone unavailable'}).Count-gt0-and'Stop preview'-notin$names)
 $steps+=Get-LabWindowStep 'unavailable-selected-microphone-does-not-preview-another-input' $h
} finally {
 $h=[LabDriver]::ByTitle('Koett Settings')
 if($h-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 300}
 [IO.File]::WriteAllText($path,$original,(New-Object Text.UTF8Encoding($false)))
}
@{passed=$rejected;artifact=Get-LabArtifact;names=$names;original_settings_restored=([IO.File]::ReadAllText($path)-ceq$original);scope='Unavailable endpoint fixture; not physical unplug';steps=$steps}
