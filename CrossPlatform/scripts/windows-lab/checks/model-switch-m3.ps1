if($null-eq$ModelIndex){$ModelIndex=0}
$ErrorActionPreference='Stop'
$ids=@('compact','parakeet_v2','parakeet_v3','nemotron');$expected=$ids[$ModelIndex]
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json'
function Read-Settings { $f=[IO.File]::Open($config,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite-bor[IO.FileShare]::Delete));$r=New-Object IO.StreamReader($f);try{$r.ReadToEnd()|ConvertFrom-Json}finally{$r.Dispose()} }
$h=[LabDriver]::ByTitle('Koett Model and Formatting');if($h-eq[IntPtr]::Zero){throw 'Open Model and Formatting on the live WAV engine'}
$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function Model-Control([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
Activate-LabEditor $h;$combo=Model-Control '60';$expand=$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern);$expand.Expand();Start-Sleep -Milliseconds 100
$items=$combo.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
if($items.Count-ne4){throw ('Expected four models, got '+$items.Count)}
$choice=$items[$ModelIndex].Current.Name;$items[$ModelIndex].GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();$expand.Collapse();Start-Sleep -Milliseconds 150
$before=(Read-Settings).selected_model;$memoryBefore=(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory
$clock=[Diagnostics.Stopwatch]::StartNew();$null=Invoke-LabButton $h 'Switch model'
$ready=$false;$status='';for($i=0;$i-lt120-and!$ready;$i++){Start-Sleep -Milliseconds 500;$saved=Read-Settings;$status=(Model-Control '62').Current.Name;$ready=($saved.selected_model-eq$expected-and$status-match'^Ready:')}
$loadMs=$clock.Elapsed.TotalMilliseconds
if(!$ready){throw ('Model did not become ready: '+$expected+'; '+$status)}
$steps=@((Get-LabWindowStep ('model-'+$expected+'-ready') $h))
$target=$env:USERPROFILE+'\M3 speech '+$expected+'.txt';[IO.File]::WriteAllText($target,'');$null=Start-Process notepad.exe -ArgumentList ('"'+$target+'"');Start-Sleep -Seconds 2
$a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle;Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');[Windows.Forms.Clipboard]::SetText('M3 speech sentinel')
Send-DictationCommand 9;Start-Sleep -Seconds 10
$clock.Restart();Send-DictationCommand 9;$text='';for($i=0;$i-lt120;$i++){Start-Sleep -Milliseconds 100;$text=Read-Editor $editor;if($text-match'portrait'){break}}
$deliveryMs=$clock.Elapsed.TotalMilliseconds;$clipboard=[Windows.Forms.Clipboard]::GetText();$phase=Get-DictationPhase
$steps+=Get-LabWindowStep ('model-'+$expected+'-wav-paste') $a
@{passed=($text-match'Ph(?:oe|e)be'-and$text-match'old portrait'-and$text.TrimEnd([char[]]"`r`n")-ceq$clipboard-and([regex]::Matches($text,'old portrait')).Count-eq1);model=$expected;choice=$choice;model_before=$before;model_after=(Read-Settings).selected_model;load_ms=$loadMs;stop_to_text_ms=$deliveryMs;guest_free_kib_before=$memoryBefore;guest_free_kib_after=(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory;text=$text;phase=$phase;fixture='official Compact test_wavs/0.wav, same prerecorded input for each model';artifact=Get-LabArtifact;steps=$steps}
