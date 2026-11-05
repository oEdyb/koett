$ErrorActionPreference='Stop'
function Word-Control([string]$id){$wordRoot.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Set-Word([string]$id,[string]$text){$e=Word-Control $id;$pattern=$null;if($e.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$pattern)){$pattern.SetValue($text)}else{$e.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a');[Windows.Forms.SendKeys]::SendWait($text)}}
function Click-Word([string]$name){$null=Invoke-LabButton $words $name;Start-Sleep -Milliseconds 250}
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$originalHash=(Get-FileHash $config).Hash
$steps=@();$cases=@()
try{
 Send-DictationCommand 10;Start-Sleep -Seconds 1
 $words=[LabDriver]::ByTitle('Koett Words');if($words-eq[IntPtr]::Zero){throw 'Words did not open'}
 Activate-LabEditor $words;$wordRoot=[Windows.Automation.AutomationElement]::FromHandle($words)
 Set-Word '41' 'M3Koett';Set-Word '42' 'm three cot, m three coat';Click-Word 'Save entry'
 $saved=[IO.File]::ReadAllText($config)|ConvertFrom-Json;$entry=@($saved.dictionary|Where-Object {$_.term-ceq'M3Koett'})
 $cases+=@{case='dictionary-explicit-save';passed=($entry.Count-eq1-and$entry[0].sounds_like.Count-eq2)}
 $steps+=Get-LabWindowStep 'dictionary-entry-saved' $words
 # A size-limit failure must not commit the partially edited dictionary.
 $hash=(Get-FileHash $config).Hash;Set-Word '41' ('x'*81);Set-Word '42' 'heard';Click-Word 'Save entry'
 $status=(Word-Control '47').Current.Name
 $cases+=@{case='invalid-term-preserves-settings';passed=((Get-FileHash $config).Hash-eq$hash-and$status.Length-gt0);status=$status}
 $steps+=Get-LabWindowStep 'invalid-term-visible-error-no-write' $words
 # Native category selector routes to literal replacements; save preserves dictionary.
 $kind=Word-Control '40';$kind.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern).Expand();Start-Sleep -Milliseconds 100
 $option=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Replacements'))
 $option.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 250
 Set-Word '41' 'M3 phrase';Set-Word '42' 'M3 replacement ÅÄÖ';Click-Word 'Save entry'
 $saved=[IO.File]::ReadAllText($config)|ConvertFrom-Json;$rule=@($saved.replacements|Where-Object {$_.phrase-ceq'M3 phrase'})
 $cases+=@{case='replacement-explicit-save';passed=($rule.Count-eq1-and$rule[0].replacement-ceq'M3 replacement ÅÄÖ'-and@($saved.dictionary|Where-Object {$_.term-ceq'M3Koett'}).Count-eq1)}
 $steps+=Get-LabWindowStep 'replacement-entry-saved' $words
 # Select the saved row through UIA, remove it, and confirm unrelated settings survive.
 $list=Word-Control '11';$item=$list.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'M3 phrase'))
 $item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Click-Word 'Remove entry'
 $saved=[IO.File]::ReadAllText($config)|ConvertFrom-Json
 $cases+=@{case='replacement-remove-preserves-dictionary';passed=(@($saved.replacements|Where-Object {$_.phrase-ceq'M3 phrase'}).Count-eq0-and@($saved.dictionary|Where-Object {$_.term-ceq'M3Koett'}).Count-eq1)}
 $steps+=Get-LabWindowStep 'replacement-removed' $words
}catch{throw ($_.Exception.Message+" at "+$_.ScriptStackTrace)}finally{
 if([LabDriver]::ByTitle('Koett Words')-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage([LabDriver]::ByTitle('Koett Words'),0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 200}
 [IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))
}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0-and(Get-FileHash $config).Hash-eq$originalHash);cases=$cases;settings_restored=((Get-FileHash $config).Hash-eq$originalHash);artifact=Get-LabArtifact;steps=$steps}
