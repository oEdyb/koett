$ErrorActionPreference='Stop'
function DB-Hash {
 $stream=[IO.File]::Open($db,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite-bor[IO.FileShare]::Delete));$sha=[Security.Cryptography.SHA256]::Create()
 try{[BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','')}finally{$sha.Dispose();$stream.Dispose()}
}
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config);$settingsHash=(Get-FileHash $config).Hash
$file=$env:APPDATA+'\DybergCo\Koett\data\Corrections.json';$existed=Test-Path $file;$prior=if($existed){[IO.File]::ReadAllText($file)}else{$null}
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$dbHash=(DB-Hash)
$steps=@();$cases=@()
function Correction-Control([string]$id){$fixRoot.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
try{
 if($existed){Remove-Item $file}
 Send-DictationCommand 2;Start-Sleep -Milliseconds 700;$history=[LabDriver]::ByTitle('Koett History');Activate-LabEditor $history
 $root=[Windows.Automation.AutomationElement]::FromHandle($history)
 $search=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'10'))
 $search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('ask Grok about Superbase')
 Start-Sleep -Milliseconds 700
 $list=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'11'))
 $row=$list.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
 $row.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 300
 $null=Invoke-LabButton $history 'Fix a word…'
 $fix=[IntPtr]::Zero;for($i=0;$i-lt30-and$fix-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 100;$fix=[LabDriver]::ByTitle('Koett Fix a word')}
 if($fix-eq[IntPtr]::Zero){throw 'Correction editor did not open; use its rebuilt artifact'}
 $fixRoot=[Windows.Automation.AutomationElement]::FromHandle($fix);$edited=Correction-Control '86';$edited.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a');[Windows.Forms.SendKeys]::SendWait('ask Groq about Supabase and Olle will review.')
 $null=Invoke-LabButton $fix 'Save correction';Start-Sleep -Milliseconds 700
 $saved=[IO.File]::ReadAllText($file)|ConvertFrom-Json
 $cases+=@{case='correction-save-does-not-change-history-or-dictionary';passed=($saved.entries[0].corrected-ceq'ask Groq about Supabase and Olle will review.'-and(DB-Hash)-eq$dbHash-and(Get-FileHash $config).Hash-eq$settingsHash)}
 $steps+=Get-LabWindowStep 'correction-saved-without-implicit-dictionary-adoption' $fix
 $proposals=Correction-Control '11';$all=$proposals.FindAll([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
 $everyday=@($all|Where-Object {$_.Current.Name-match'^all → Olle'})[0]
 $everyday.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 200
 $warning=(Correction-Control '84').Current.Name;$add=(Correction-Control '81').Current.Name
 $stillUnchanged=(Get-FileHash $config).Hash-eq$settingsHash
 $steps+=Get-LabWindowStep 'everyday-warning-before-explicit-acceptance' $fix
 $null=Invoke-LabButton $fix 'Add anyway';Start-Sleep -Milliseconds 300
 $s=[IO.File]::ReadAllText($config)|ConvertFrom-Json
 $cases+=@{case='everyday-warning-and-explicit-add-anyway';passed=($warning-match'everyday'-and$add-ceq'Add anyway'-and$stillUnchanged-and@($s.dictionary|Where-Object {$_.term-ceq'Olle'-and'all'-in$_.sounds_like}).Count-eq1);warning=$warning}
 $steps+=Get-LabWindowStep 'everyday-form-requires-add-anyway' $fix
 $all=$proposals.FindAll([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)))
 $grok=@($all|Where-Object {$_.Current.Name-match'^Grok → Groq'})[0];$grok.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();$hash=(Get-FileHash $config).Hash
 $null=Invoke-LabButton $fix 'Dismiss';Start-Sleep -Milliseconds 300
 $dismissed=[IO.File]::ReadAllText($file)|ConvertFrom-Json
 $cases+=@{case='dismiss-suggestion-does-not-write-dictionary';passed=(@($dismissed.dismissed|Where-Object {$_.heard-ceq'Grok'-and$_.term-ceq'Groq'}).Count-eq1-and(Get-FileHash $config).Hash-eq$hash)}
 $steps+=Get-LabWindowStep 'suggestion-dismissed-without-dictionary-change' $fix
 # Failure in the separate correction store must preserve both history and settings.
 $hash=(Get-FileHash $file).Hash;(Get-Item $file).IsReadOnly=$true
 $edited.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a');[Windows.Forms.SendKeys]::SendWait('This must not replace the saved correction.')
 $null=Invoke-LabButton $fix 'Save correction';Start-Sleep -Milliseconds 300
 $notice=(Correction-Control '83').Current.Name
 $cases+=@{case='correction-write-failure-preserves-all-stores';passed=((Get-FileHash $file).Hash-eq$hash-and(DB-Hash)-eq$dbHash-and$notice-match'denied');notice=$notice}
 $steps+=Get-LabWindowStep 'correction-write-failure-visible' $fix
}finally{
 if(Test-Path $file){(Get-Item $file).IsReadOnly=$false}
 $h=[LabDriver]::ByTitle('Koett Fix a word');if($h-ne[IntPtr]::Zero){$null=[LabDriver]::PostMessage($h,0x10,[IntPtr]::Zero,[IntPtr]::Zero);Start-Sleep -Milliseconds 200}
 [IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))
 if($existed){[IO.File]::WriteAllText($file,$prior,(New-Object Text.UTF8Encoding($false)))}elseif(Test-Path $file){Remove-Item $file}
}
@{passed=(@($cases|Where-Object {!$_.passed}).Count-eq0-and(Get-FileHash $config).Hash-eq$settingsHash-and(DB-Hash)-eq$dbHash);cases=$cases;settings_restored=((Get-FileHash $config).Hash-eq$settingsHash);history_unchanged=((DB-Hash)-eq$dbHash);artifact=Get-LabArtifact;steps=$steps}
