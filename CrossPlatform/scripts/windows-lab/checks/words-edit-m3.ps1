$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$hash=(Get-FileHash $config).Hash
function Word-Control([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Set-Word([string]$id,[string]$text){$e=Word-Control $id;$p=$null;if($e.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$p)){$p.SetValue($text)}else{$e.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a');[Windows.Forms.SendKeys]::SendWait($text)}}
function Click-Word([string]$name){$null=Invoke-LabButton $h $name;Start-Sleep -Milliseconds 350}
function Select-Word([string]$name){$e=(Word-Control '11').FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),$name));$e.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 350}
function Word-Settings{[IO.File]::ReadAllText($config)|ConvertFrom-Json}
$cases=@();$steps=@()
try{
 Send-DictationCommand 10;Start-Sleep -Milliseconds 700;$h=[LabDriver]::ByTitle('Koett Words');Activate-LabEditor $h;$root=[Windows.Automation.AutomationElement]::FromHandle($h)
 Click-Word 'New entry';Set-Word '41' 'M3Alpha';Set-Word '42' 'alpha heard';Click-Word 'Save entry'
 Click-Word 'New entry';Set-Word '41' 'M3Beta';Set-Word '42' 'beta heard';Click-Word 'Save entry'
 Select-Word 'M3Alpha';$term=(Word-Control '41').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value
 $cases+=@{case='uia-list-selection-refreshes-editor';passed=($term-ceq'M3Alpha')}
 Set-Word '41' 'M3Gamma';Click-Word 'Save entry';$s=Word-Settings
 $cases+=@{case='rename-removes-only-original-key';passed=(@($s.dictionary|Where-Object term -eq 'M3Alpha').Count-eq0-and@($s.dictionary|Where-Object term -eq 'M3Gamma').Count-eq1-and@($s.dictionary|Where-Object term -eq 'M3Beta').Count-eq1)}
 $steps+=Get-LabWindowStep 'words-uia-rename' $h
 Select-Word 'M3Gamma';$before=(Get-FileHash $config).Hash;Set-Word '41' 'M3Beta';Click-Word 'Save entry';$status=(Word-Control '47').Current.Name
 $cases+=@{case='rename-collision-visible-no-write';passed=($before-eq(Get-FileHash $config).Hash-and$status-match'already uses');status=$status}
 $steps+=Get-LabWindowStep 'words-rename-collision' $h
 Select-Word 'M3Beta';$s=Word-Settings;@($s.dictionary|Where-Object term -eq 'M3Beta')[0].sounds_like=@('external edit');[IO.File]::WriteAllText($config,($s|ConvertTo-Json -Depth 15),(New-Object Text.UTF8Encoding($false)))
 $before=(Get-FileHash $config).Hash;Set-Word '42' 'stale overwrite';Click-Word 'Save entry';$status=(Word-Control '47').Current.Name
 $cases+=@{case='stale-editor-does-not-overwrite-external-change';passed=($before-eq(Get-FileHash $config).Hash-and$status-match'changed elsewhere');status=$status}
 $steps+=Get-LabWindowStep 'words-stale-editor-preserves-external-edit' $h
 Click-Word 'New entry';$blank=(Word-Control '41').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value
 $cases+=@{case='new-entry-clears-prior-selection';passed=($blank-ceq'')}
}finally{
 $h=[LabDriver]::ByTitle('Koett Words');if($h-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 200}
 [IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))
}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0-and(Get-FileHash $config).Hash-eq$hash);cases=$cases;settings_restored=((Get-FileHash $config).Hash-eq$hash);artifact=Get-LabArtifact;steps=$steps}
