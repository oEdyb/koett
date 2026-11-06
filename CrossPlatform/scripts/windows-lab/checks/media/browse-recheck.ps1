$ErrorActionPreference='Stop';$path=$global:MediaDir+'\tail åä.wav'
$root=[Windows.Automation.AutomationElement]::FromHandle([IntPtr](Get-LabFocus).hwnd);$all=$root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
$e=@($all|Where-Object {$_.Current.AutomationId-eq'1148'-and$_.Current.ControlType-eq[Windows.Automation.ControlType]::Edit})[0]
([Windows.Automation.ValuePattern]$e.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).SetValue($path)
$b=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,'1')));([Windows.Automation.InvokePattern]$b.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()
$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$w=Wait-MediaWindow}while(-not$w.Current.IsEnabled-and$watch.ElapsedMilliseconds-lt5000);if(-not$w.Current.IsEnabled){throw 'File picker did not finish'}
$chosen=([Windows.Automation.ValuePattern](Media-Item '101').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).Current.Value
$b=Media-Item '1';if($b.Current.Name-ne'Transcribe'){throw ('Wrong control: '+$b.Current.Name)}
$lastPath=$env:APPDATA+'\DybergCo\Koett\data\Last Transcript.txt';$lastBefore=(Get-FileHash $lastPath).Hash
Media-Invoke '1';Start-Sleep -Milliseconds 400;$null=Wait-MediaDone;$text=Media-Text;$before=Get-LabFocus;Media-Invoke '104';$after=Get-LabFocus;$clip=[Windows.Forms.Clipboard]::GetText();$lastAfter=(Get-FileHash $lastPath).Hash
@{passed=($chosen-eq$path-and$text.Trim()-ceq$clip.Trim()-and$clip.Trim().Length-gt0-and$clip-ne[IO.File]::ReadAllText($lastPath)-and$lastBefore-eq$lastAfter-and$before.hwnd-eq$after.hwnd);cancel_preserved=$true;chosen=$chosen;result=$text;clipboard=$clip;last_before=$lastBefore;last_after=$lastAfter;before=$before;after=$after;step=Media-Step 'browse-unicode-and-copy-distinct';artifact=Get-LabArtifact}
