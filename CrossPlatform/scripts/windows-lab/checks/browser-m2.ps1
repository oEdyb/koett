$edge=${env:ProgramFiles(x86)}+'\Microsoft\Edge\Application\msedge.exe'
$browser=[Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name-like'Koett M2 Browser*'-and$_.Current.ClassName-eq'Chrome_WidgetWin_1'}|Select-Object -First 1
if(!$browser){throw 'Edge window missing'}
$h=[IntPtr]$browser.Current.NativeWindowHandle;Activate-LabEditor $h
$condition=New-Object Windows.Automation.AndCondition ((New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Koett M2 editor'),(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::Edit)))
$editor=$browser.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
if(!$editor){throw 'Browser editor missing'};$editor.SetFocus();$before=Read-Editor $editor
if($before.Length-ne0){throw 'Browser is not empty'}
$focus=Get-LabFocus;Send-DictationCommand 9;Start-Sleep -Seconds 8;Send-DictationCommand 9;Start-Sleep -Milliseconds 800
$text=Read-Editor $editor;$clipboard=[Windows.Forms.Clipboard]::GetText()
$names=@($browser.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$events=@($names|Where-Object {$_-match'^Paste events:'})
$steps=@((Get-LabStep 'browser-result'),(Get-LabWindowStep 'browser-exact-text-and-paste-event-count' $h))
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$null=Invoke-WebRequest -UseBasicParsing -Method Post http://10.0.2.2:18768/history -InFile $db
@{passed=($text-ceq$clipboard-and$clipboard-match'Phoebe'-and'Paste events: 1'-in$events-and(Get-LabFocus).hwnd-eq$focus.hwnd);artifact=Get-LabArtifact;text=$text;clipboard=$clipboard;paste_events=$events;original_focus=$focus;steps=$steps}
