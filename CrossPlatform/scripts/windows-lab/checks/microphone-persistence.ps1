$ErrorActionPreference='Stop'
if(-not ('MicLab' -as [type])){Add-Type @'
using System;using System.Runtime.InteropServices;
public class MicLab { [DllImport("user32.dll")]public static extern IntPtr GetDlgItem(IntPtr h,int id); }
'@}
function Open-MicSettings {
 Set-LabPhase 8;Start-Sleep -Milliseconds 700
 $h=[LabDriver]::ByTitle('Koett Settings');if($h-eq[IntPtr]::Zero){throw 'Settings missing'};$h
}
function Select-Mic([IntPtr]$h,[int]$index){
 $combo=[MicLab]::GetDlgItem($h,30)
 $null=[LabDriver]::SendMessage($combo,0x14E,[IntPtr]$index,[IntPtr]::Zero)
 $null=[LabDriver]::PostMessage($h,0x111,[IntPtr](30-bor(1-shl16)),[IntPtr]::Zero)
 Start-Sleep -Milliseconds 200
}
function Close-MicSettings([IntPtr]$h){$null=[LabDriver]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 500}
$h=Open-MicSettings
$combo=[MicLab]::GetDlgItem($h,30)
$count=[LabDriver]::SendMessage($combo,0x146,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()
$path=Join-Path $env:APPDATA 'DybergCo\Koett\config\settings.json'
$steps=@();$explicit=$null;$restored=$null
if($count-gt1){
 Select-Mic $h 1
 $null=Invoke-LabButton $h 'Save'
 $explicit=(Get-Content $path -Raw|ConvertFrom-Json).microphone
 if(!$explicit){throw 'Explicit endpoint ID was not saved'}
 $steps+=Get-LabWindowStep 'microphone-explicit-saved' $h
 Close-MicSettings $h;$h=Open-MicSettings
 $restored=[LabDriver]::SendMessage([MicLab]::GetDlgItem($h,30),0x147,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()
 if($restored-ne1){throw 'Endpoint selection did not survive reopening Settings'}
}
Select-Mic $h 0
$null=Invoke-LabButton $h 'Save'
$default=(Get-Content $path -Raw|ConvertFrom-Json).microphone
if($null-ne$default){throw 'System default did not persist as null'}
$null=Invoke-LabButton $h 'Preview'
Start-Sleep -Seconds 1
$root=[Windows.Automation.AutomationElement]::FromHandle($h)
$names=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$active='Stop preview'-in$names
$level=@($names|Where-Object {$_-match'^Input level:'})
$steps+=Get-LabWindowStep 'microphone-default-preview' $h
Close-MicSettings $h;$h=Open-MicSettings
$null=Invoke-LabButton $h 'Preview';Start-Sleep -Seconds 1
$root=[Windows.Automation.AutomationElement]::FromHandle($h)
$reopened=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$reacquired='Stop preview'-in$reopened
$steps+=Get-LabWindowStep 'microphone-preview-reacquired-after-close' $h
if($reacquired){$null=Invoke-LabButton $h 'Stop preview'}
Start-Sleep -Milliseconds 300
$stoppedNames=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$stopped='Stop preview'-notin$stoppedNames
@{passed=($count-gt1-and$restored-eq1-and$null-eq$default-and$active-and$level.Count-gt0-and$reacquired-and$stopped);artifact=Get-LabArtifact;endpoint_count=$count;explicit_endpoint_id=$explicit;reopened_selection=$restored;default_saved=$default;preview_active=$active;preview_levels=$level;reacquired_after_close=$reacquired;stopped=$stopped;names=$names;scope='Emulated HDA endpoint and silent level callbacks; no real physical microphone';steps=$steps}
