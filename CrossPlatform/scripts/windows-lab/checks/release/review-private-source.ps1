$ErrorActionPreference='Stop'
$root=[Windows.Automation.AutomationElement]::RootElement
$browser=@($root.FindAll([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ClassNameProperty,'Chrome_WidgetWin_1')))|Where-Object {$_.Current.Name-match'InPrivate'}|Select-Object -First 1)[0]
if(-not$browser){throw 'Edge window unavailable'};Activate-LabEditor ([IntPtr]$browser.Current.NativeWindowHandle);Start-Sleep -Milliseconds 300
$field=$browser.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Media focus editor')))
if(-not$field){throw 'Browser editor unavailable'}
$field.SetFocus();$before=Get-LabFocus
$p=Start-Process ($env:USERPROFILE+'\KoettM5Media\Koett.exe') -ArgumentList 'media' -PassThru
Start-Sleep -Seconds 2
$window=$root.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Transcribe media')))
if(-not$window){throw 'Media window unavailable'}
$inputBox=$window.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,'101')))
$value=([Windows.Automation.ValuePattern]$inputBox.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).Current.Value
@{step=Get-LabWindowStep 'private-edge-source-omitted' ([IntPtr]$window.Current.NativeWindowHandle);artifact=Get-LabArtifact;passed=($value-eq'');browser_name=$browser.Current.Name;private_indicators=@($browser.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {if($_.Current.Name-match'InPrivate'){$_.Current.Name}});source=$value;before=$before;after=Get-LabFocus;focus_in_editor=$field.Current.HasKeyboardFocus}
