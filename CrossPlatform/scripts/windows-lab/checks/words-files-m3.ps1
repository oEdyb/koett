$ErrorActionPreference='Stop'
function Word-Control([string]$id){$wordRoot.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
function Finish-File([string]$title,[string]$path){
 $edit=$null;for($i=0;$i-lt50-and!$edit;$i++){
  $d=[LabDriver]::ByTitle($title)
  if($d-ne[IntPtr]::Zero){$r=[Windows.Automation.AutomationElement]::FromHandle($d);$edit=$r.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'1001'))}
  if(!$edit){Start-Sleep -Milliseconds 100}
 }
 if(!$edit){throw "File dialog unavailable: $title"}
 $edit.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a');[Windows.Forms.SendKeys]::SendWait($path);[Windows.Forms.SendKeys]::SendWait('{TAB}')
 $null=[LabDriver]::PostMessage($d,0x111,[IntPtr]1,[IntPtr]::Zero);Start-Sleep -Milliseconds 300
}
function Confirm-Import([string]$name){
 $button=$null;for($i=0;$i-lt50-and!$button;$i++){
  $d=[LabDriver]::ByTitle('Import dictionary');if($d-ne[IntPtr]::Zero){$r=[Windows.Automation.AutomationElement]::FromHandle($d);$button=$r.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),$name))}
  if(!$button){Start-Sleep -Milliseconds 100}
 }
 if(!$button){throw 'Import confirmation unavailable'}
 $button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke();Start-Sleep -Milliseconds 300
}
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$originalHash=(Get-FileHash $config).Hash;$readOnly=(Get-Item $config).IsReadOnly
$source=$env:USERPROFILE+'\m3-import.txt';$export=$env:USERPROFILE+'\m3-dictionary-export.txt';$invalid=$env:USERPROFILE+'\m3-invalid.json'
[IO.File]::WriteAllText($source,"KoettCase = coat case`nÅTerm = err term",(New-Object Text.UTF8Encoding($false)));[IO.File]::WriteAllText($invalid,'{');if(Test-Path $export){Remove-Item $export}
$cases=@();$steps=@()
try{
 Send-DictationCommand 10;Start-Sleep -Seconds 1;$words=[LabDriver]::ByTitle('Koett Words');$wordRoot=[Windows.Automation.AutomationElement]::FromHandle($words);Activate-LabEditor $words
 $null=[LabDriver]::PostMessage($words,0x111,[IntPtr]45,[IntPtr]::Zero);Finish-File 'Open' $source;Confirm-Import 'Cancel'
 $cases+=@{case='import-cancel-no-write';passed=((Get-FileHash $config).Hash-eq$originalHash)}
 $null=[LabDriver]::PostMessage($words,0x111,[IntPtr]45,[IntPtr]::Zero);Finish-File 'Open' $source;Confirm-Import 'OK'
 $saved=[IO.File]::ReadAllText($config)|ConvertFrom-Json
 $cases+=@{case='import-explicit-accept';passed=(@($saved.dictionary|Where-Object {$_.term-ceq'KoettCase'-and'coat case'-in$_.sounds_like}).Count-eq1-and@($saved.dictionary|Where-Object {$_.term-ceq'ÅTerm'}).Count-eq1)}
 $steps+=Get-LabWindowStep 'dictionary-import-explicit-acceptance' $words
 $hash=(Get-FileHash $config).Hash;$null=[LabDriver]::PostMessage($words,0x111,[IntPtr]45,[IntPtr]::Zero);Finish-File 'Open' $invalid;Start-Sleep -Milliseconds 300
 $status=(Word-Control '47').Current.Name;$cases+=@{case='invalid-import-no-write';passed=((Get-FileHash $config).Hash-eq$hash-and$status.Length-gt0);status=$status}
 $null=[LabDriver]::PostMessage($words,0x111,[IntPtr]46,[IntPtr]::Zero);Finish-File 'Save As' $export
 for($i=0;$i-lt30-and!(Test-Path $export);$i++){Start-Sleep -Milliseconds 100}
 $text=[IO.File]::ReadAllText($export);$cases+=@{case='canonical-dictionary-export';passed=($text.Contains('KoettCase = coat case')-and$text.Contains('ÅTerm = err term'));text=$text}
 $steps+=Get-LabWindowStep 'dictionary-export-canonical-text' $words
 # Denied primary settings writes must remain visible and preserve saved entries.
 (Get-Item $config).IsReadOnly=$true
 (Word-Control '41').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('MustNotSave')
 $null=Invoke-LabButton $words 'Save entry';Start-Sleep -Milliseconds 300
 $status=(Word-Control '47').Current.Name;$cases+=@{case='settings-write-failure-preserves-file';passed=((Get-FileHash $config).Hash-eq$hash-and$status-match'could not save');status=$status}
 $steps+=Get-LabWindowStep 'word-save-failure-visible-and-preserves-settings' $words
}finally{
 (Get-Item $config).IsReadOnly=$false
 if([LabDriver]::ByTitle('Koett Words')-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage([LabDriver]::ByTitle('Koett Words'),0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 200}
 [IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)));(Get-Item $config).IsReadOnly=$readOnly
}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0-and(Get-FileHash $config).Hash-eq$originalHash);cases=$cases;settings_restored=((Get-FileHash $config).Hash-eq$originalHash);artifact=Get-LabArtifact;steps=$steps}
