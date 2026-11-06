$ErrorActionPreference='Stop'
function Dialog-Item([string]$id){$r=[Windows.Automation.AutomationElement]::FromHandle([IntPtr](Get-LabFocus).hwnd);$r.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,$id)))}
$c=Dialog-Item '2';([Windows.Automation.InvokePattern]$c.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke();Start-Sleep -Milliseconds 200
$preserved=([Windows.Automation.ValuePattern](Media-Item '101').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).Current.Value-eq'cancel must preserve this'
$path=$global:MediaDir+'\tail åä.wav';Copy-Item ($global:MediaDir+'\tail.wav') $path -Force
$window=Wait-MediaWindow;$h=[IntPtr]$window.Current.NativeWindowHandle;$null=[LabDriver]::PostMessage($h,0x111,[IntPtr]109,[IntPtr]::Zero);Start-Sleep -Milliseconds 500
$root=[Windows.Automation.AutomationElement]::FromHandle([IntPtr](Get-LabFocus).hwnd)
$e=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.AutomationId-eq'1148'-and$_.Current.ControlType-eq[Windows.Automation.ControlType]::Edit})[0]
([Windows.Automation.ValuePattern]$e.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).SetValue($path);$e.SetFocus();[Windows.Forms.SendKeys]::SendWait('{ENTER}');Start-Sleep -Milliseconds 400
$chosen=([Windows.Automation.ValuePattern](Media-Item '101').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).Current.Value
$lastPath=$env:APPDATA+'\DybergCo\Koett\data\Last Transcript.txt';$lastBefore=(Get-FileHash $lastPath).Hash
Media-Invoke '1';$null=Wait-MediaDone;$text=Media-Text;$before=Get-LabFocus;Media-Invoke '104';$after=Get-LabFocus;$clip=[Windows.Forms.Clipboard]::GetText();$lastAfter=(Get-FileHash $lastPath).Hash
@{passed=($preserved-and$chosen-eq$path-and$text.Trim()-ceq$clip.Trim()-and$clip.Trim().Length-gt0-and$clip-ne[IO.File]::ReadAllText($lastPath)-and$lastBefore-eq$lastAfter-and$before.hwnd-eq$after.hwnd);cancel_preserved=$preserved;chosen=$chosen;result=$text;clipboard=$clip;last_before=$lastBefore;last_after=$lastAfter;before=$before;after=$after;step=Media-Step 'browse-unicode-and-copy-distinct';artifact=Get-LabArtifact}
