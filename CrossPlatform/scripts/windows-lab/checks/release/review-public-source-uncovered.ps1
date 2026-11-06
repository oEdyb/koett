foreach($name in @('Koett Settings','Transcript recovery','Transcribe media')){$h=[LabDriver]::ByTitle($name);if($h-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}};Start-Sleep -Milliseconds 300
$ErrorActionPreference='Stop'
$root=[Windows.Automation.AutomationElement]::RootElement
$browser=$root.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ClassNameProperty,'Chrome_WidgetWin_1')))
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
@{step=Get-LabWindowStep 'public-edge-source' ([IntPtr]$window.Current.NativeWindowHandle);artifact=Get-LabArtifact;passed=($value-eq'http://10.0.2.2:18767/m5-media-page.html');source=$value;before=$before;after=Get-LabFocus;focus_in_editor=$field.Current.HasKeyboardFocus}
