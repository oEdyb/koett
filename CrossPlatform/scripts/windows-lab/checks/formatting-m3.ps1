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
$app=(Get-LabArtifact);$owned=@(Get-CimInstance Win32_Process -Filter "name='llama-server.exe'"|Where-Object {$_.ParentProcessId-eq$app.pid})
if($owned.Count-ne1){throw 'Expected one formatter child owned by this guest Koett process'}
Stop-Process -Id $owned[0].ProcessId -Force
$second=Speech-Once 'formatter-crash-raw-wav-pasted-once'
$status=(Format-Control '68').Current.Name
$steps=@($first.step,$second.step);$first.Remove('step');$second.Remove('step')
@{passed=($first.passed-and$second.passed-and$status-match'Raw active:');formatted=$first;raw_fallback=$second;status=$status;artifact=$app;steps=$steps}
