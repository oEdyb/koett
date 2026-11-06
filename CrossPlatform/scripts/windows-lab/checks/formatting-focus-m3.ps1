$ErrorActionPreference='Stop'
$h=[LabDriver]::ByTitle('Koett Model and Formatting');$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function Formatting-Control([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json'
$toggle=(Formatting-Control '63').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
if($toggle.Current.ToggleState-ne[Windows.Automation.ToggleState]::On){$toggle.Toggle()}
$null=Invoke-LabButton $h 'Apply formatting'
# Invoke of a native settings button activates its window. The user then resumes typing.
$target=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle;Activate-LabEditor $target;$editor=Find-Editor $target;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}')
$before=Get-LabFocus;$samples=@();$status=''
for($i=0;$i-lt100;$i++){
 [Windows.Forms.SendKeys]::SendWait('setup typing ');$samples+=Get-LabFocus;Start-Sleep -Milliseconds 300;$status=(Formatting-Control '68').Current.Name;if($status-match'ready|Raw active:'){break}
}
$s=[IO.File]::ReadAllText($config)|ConvertFrom-Json;$text=Read-Editor $editor
$servers=@(Get-Process llama-server -ErrorAction SilentlyContinue|ForEach-Object {@{pid=$_.Id;window=$_.MainWindowHandle.ToInt64()}})
@{passed=($status-match'S1-mini ready'-and$s.formatting.enabled-and@($samples|Where-Object {$_.hwnd-ne$before.hwnd}).Count-eq0-and$text-match'setup typing'-and$servers.Count-eq1-and$servers[0].window-eq0);status=$status;focus_before=$before;focus_samples=$samples;typed=$text;servers=$servers;artifact=Get-LabArtifact;steps=@((Get-LabWindowStep 'formatter-setup-keeps-notepad-typing-focus' $target))}
