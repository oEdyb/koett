$ErrorActionPreference='Stop'
$h=[LabDriver]::ByTitle('Koett Model and Formatting')
$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function Format-Control([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Speech-Once([string]$name){
 Activate-LabEditor $target;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');[Windows.Forms.Clipboard]::SetText('M3 formatting sentinel')
 $before=Get-LabFocus;Send-DictationCommand 9;Start-Sleep -Seconds 10;Send-DictationCommand 9
 $text='';for($i=0;$i-lt300;$i++){Start-Sleep -Milliseconds 100;$text=Read-Editor $editor;if($text-match'portrait'){break}}
 $after=Get-LabFocus;$phase=Get-DictationPhase;$clip=[Windows.Forms.Clipboard]::GetText()
 @{passed=($text-match'portrait'-and([regex]::Matches($text,'portrait')).Count-eq1-and$text.TrimEnd([char[]]"`r`n")-ceq$clip-and$before.hwnd-eq$after.hwnd);text=$text;phase=$phase;focus_before=$before;focus_after=$after;step=(Get-LabWindowStep $name $target)}
}
$target=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle;$editor=Find-Editor $target
$first=Speech-Once 'formatted-wav-pasted-once'
Send-DictationCommand 2;Start-Sleep -Milliseconds 400;$history=[LabDriver]::ByTitle('Koett History');$hr=[Windows.Automation.AutomationElement]::FromHandle($history)
function History-Control([string]$id){$hr.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
(History-Control '10').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('portrait');Start-Sleep -Milliseconds 500
$item=(History-Control '11').FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
$item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 250
$null=Invoke-LabButton $history 'Copy';$historyFinal=[Windows.Forms.Clipboard]::GetText()
(History-Control '17').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Toggle();Start-Sleep -Milliseconds 300;$null=Invoke-LabButton $history 'Copy';$historyRaw=[Windows.Forms.Clipboard]::GetText()
$historyStep=Get-LabWindowStep 'formatting-keeps-original-asr-in-history' $history
$null=[LabDriver]::PostMessage($history,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 200
$historyPreserved=($historyFinal-ceq$first.text.TrimEnd([char[]]"`r`n")-and$historyRaw-ceq"Well, I don't wish to see it any more, observed Phoebe, turning away her eyes. It is certainly very like the old portrait.")
$app=(Get-LabArtifact);$owned=@(Get-CimInstance Win32_Process -Filter "name='llama-server.exe'"|Where-Object {$_.ParentProcessId-eq$app.pid})
if($owned.Count-ne1){throw 'Expected one formatter child owned by this guest Koett process'}
Stop-Process -Id $owned[0].ProcessId -Force
$second=Speech-Once 'formatter-crash-raw-wav-pasted-once'
$status=(Format-Control '68').Current.Name
$steps=@($first.step,$historyStep,$second.step);$first.Remove('step');$second.Remove('step')
@{passed=($first.passed-and$second.passed-and$historyPreserved-and$status-match'Raw active:');formatted=$first;raw_fallback=$second;history_raw_preserved=$historyPreserved;history_final=$historyFinal;history_raw=$historyRaw;status=$status;artifact=$app;steps=$steps}
