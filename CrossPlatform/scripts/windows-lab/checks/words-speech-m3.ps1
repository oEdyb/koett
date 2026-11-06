$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config)
$cases=@();$steps=@()
function Control([IntPtr]$w,[string]$id){[Windows.Automation.AutomationElement]::FromHandle($w).FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Fill([IntPtr]$w,[string]$id,[string]$text){$e=Control $w $id;$p=$null;if($e.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$p)){$p.SetValue($text)}else{$e.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a');[Windows.Forms.SendKeys]::SendWait($text)}}
try{
 $processing=[LabDriver]::ByTitle('Koett Model and Formatting');$t=(Control $processing '63').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
 if($t.Current.ToggleState-eq[Windows.Automation.ToggleState]::On){$t.Toggle();$null=Invoke-LabButton $processing 'Apply formatting';Start-Sleep -Milliseconds 500}
 Send-DictationCommand 10;Start-Sleep -Milliseconds 600;$words=[LabDriver]::ByTitle('Koett Words');Activate-LabEditor $words
 $null=Invoke-LabButton $words 'New entry';Fill $words '41' 'M3Phoebe';Fill $words '42' 'Phoebe';$null=Invoke-LabButton $words 'Save entry';Start-Sleep -Milliseconds 300
 $combo=Control $words '40';$expand=$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern);$expand.Expand();Start-Sleep -Milliseconds 100
 $item=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Replacements'))
 $item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();$expand.Collapse();Start-Sleep -Milliseconds 300
 $null=Invoke-LabButton $words 'New entry';Fill $words '41' 'old portrait';Fill $words '42' 'M3 replacement';$null=Invoke-LabButton $words 'Save entry';Start-Sleep -Milliseconds 500
 $target=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle;Activate-LabEditor $target;$editor=Find-Editor $target;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');$before=Get-LabFocus
 Send-DictationCommand 9;Start-Sleep -Seconds 10;$clock=[Diagnostics.Stopwatch]::StartNew();Send-DictationCommand 9
 for($i=0;$i-lt150;$i++){Start-Sleep -Milliseconds 100;$text=Read-Editor $editor;if($text-match'M3 replacement'){break}}
 $latency=$clock.Elapsed.TotalMilliseconds;$after=Get-LabFocus;$clip=[Windows.Forms.Clipboard]::GetText()
 $cases+=@{case='dictionary-and-replacement-apply-to-one-live-cpu-wav-result';passed=($text-match'M3Phoebe'-and$text-match'M3 replacement'-and$text-notmatch'old portrait'-and([regex]::Matches($text,'M3 replacement')).Count-eq1-and$text.TrimEnd([char[]]"`r`n")-ceq$clip-and$after.hwnd-eq$before.hwnd);text=$text;stop_to_text_ms=$latency;focus_before=$before;focus_after=$after}
 $steps+=Get-LabWindowStep 'dictionary-and-replacements-live-cpu-wav-paste' $target
 Send-DictationCommand 2;Start-Sleep -Milliseconds 500;$history=[LabDriver]::ByTitle('Koett History');Activate-LabEditor $history
 Fill $history '10' 'M3Phoebe';Start-Sleep -Milliseconds 600
 $item=(Control $history '11').FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
 $item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 300
 $raw=(Control $history '17').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern);if($raw.Current.ToggleState-eq[Windows.Automation.ToggleState]::On){$raw.Toggle();Start-Sleep -Milliseconds 300}
 $null=Invoke-LabButton $history 'Copy';$final=[Windows.Forms.Clipboard]::GetText();$raw.Toggle();Start-Sleep -Milliseconds 300;$null=Invoke-LabButton $history 'Copy';$rawText=[Windows.Forms.Clipboard]::GetText()
 $cases+=@{case='history-retains-original-asr-before-dictionary-and-replacement';passed=($final-ceq$text.TrimEnd([char[]]"`r`n")-and$rawText-match'Phoebe'-and$rawText-match'old portrait'-and$rawText-notmatch'M3Phoebe|M3 replacement');final=$final;raw=$rawText}
 $steps+=Get-LabWindowStep 'history-original-asr-copy-after-word-transforms' $history
}finally{
 foreach($title in @('Koett Words','Koett History')){$h=[LabDriver]::ByTitle($title);if($h-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}}
 # Restore raw mode in the engine, then restore the exact settings file.
 $p=[LabDriver]::ByTitle('Koett Model and Formatting');if($p-ne[IntPtr]::Zero){$t=(Control $p '63').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern);if($t.Current.ToggleState-eq[Windows.Automation.ToggleState]::On){$t.Toggle();$null=Invoke-LabButton $p 'Apply formatting';Start-Sleep -Milliseconds 300}}
 [IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))
}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0);cases=$cases;fixture='same official Compact 0.wav; text spelling/near-miss rule, not CTC acoustic gap closure';artifact=Get-LabArtifact;steps=$steps}
