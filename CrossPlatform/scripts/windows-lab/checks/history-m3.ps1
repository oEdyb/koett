$ErrorActionPreference='Stop'
function Cancel-Export {
 for($i=0;$i-lt50;$i++){
  $dialog=[LabDriver]::ByTitle('Save As')
  if($dialog-ne[IntPtr]::Zero){$r=[Windows.Automation.AutomationElement]::FromHandle($dialog);$button=$r.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),'Cancel'));if($button){$button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke();return}}
  Start-Sleep -Milliseconds 100
 }
 throw 'Save As Cancel did not become accessible'
}
$h=[LabDriver]::ByTitle('Koett History');if($h-eq[IntPtr]::Zero){throw 'Open seeded M3 History first'}
if([LabDriver]::ByTitle('Save As')-ne[IntPtr]::Zero){Cancel-Export}
Activate-LabEditor $h
$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function ById([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
$search=ById '10';$search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('')
$raw=ById '17';$toggle=$raw.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern);if($toggle.Current.ToggleState-eq[Windows.Automation.ToggleState]::On){$toggle.Toggle()}
Start-Sleep -Milliseconds 500
function Export-To([int]$id,[string]$path){
 $null=[LabDriver]::PostMessage($h,0x111,[IntPtr]$id,[IntPtr]::Zero)
 $dialog=[IntPtr]::Zero;for($i=0;$i-lt30-and$dialog-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 100;$dialog=[LabDriver]::ByTitle('Save As')}
 if($dialog-eq[IntPtr]::Zero){throw 'Save As did not open'}
 $saveRoot=[Windows.Automation.AutomationElement]::FromHandle($dialog)
 $edit=$null;for($j=0;$j-lt30-and!$edit;$j++){Start-Sleep -Milliseconds 100;$edit=$saveRoot.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),'1001'))}
 if(!$edit){throw 'Save As filename field unavailable'}
 $edit.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a');[Windows.Forms.SendKeys]::SendWait($path);[Windows.Forms.SendKeys]::SendWait('{TAB}')
 $null=Invoke-LabButton $dialog 'Save'
 for($i=0;$i-lt50-and!(Test-Path $path);$i++){Start-Sleep -Milliseconds 100}
 Start-Sleep -Milliseconds 300
 if(!(Test-Path $path)){throw "Export missing: $path"}
 [IO.File]::ReadAllText($path)
}
$list=ById '11';$items=$list.FindAll([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)));$visibleCount=$items.Count
$items[1].GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 300
$null=Invoke-LabButton $h 'Copy';$final=[Windows.Forms.Clipboard]::GetText();$steps=@((Get-LabWindowStep 'final-transcript-copy' $h))
$raw=ById '17';$raw.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern).Toggle();Start-Sleep -Milliseconds 300
$null=Invoke-LabButton $h 'Copy';$rawText=[Windows.Forms.Clipboard]::GetText();$steps+=Get-LabWindowStep 'raw-transcript-view-and-copy' $h
$single=$env:USERPROFILE+'\m3-single.txt';$all=$env:USERPROFILE+'\m3-all.txt';$filtered=$env:USERPROFILE+'\m3-filtered.txt'
foreach($p in @($single,$all,$filtered)){if(Test-Path $p){Remove-Item $p}}
$singleText=Export-To 13 $single
$allText=Export-To 15 $all;$status=(ById '16').Current.Name
$steps+=Get-LabWindowStep 'raw-export-all-110-from-first-50' $h
$search=ById '10';$search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('108');Start-Sleep -Milliseconds 500
$filteredText=Export-To 15 $filtered;$steps+=Get-LabWindowStep 'filtered-raw-export-one' $h
# Cancel must not write or truncate the existing exports.
$hash=(Get-FileHash $all).Hash;$null=[LabDriver]::PostMessage($h,0x111,[IntPtr]15,[IntPtr]::Zero);Start-Sleep -Milliseconds 400
$dialog=[LabDriver]::ByTitle('Save As');Cancel-Export;Start-Sleep -Milliseconds 300
$cancelled=((Get-FileHash $all).Hash-eq$hash-and[LabDriver]::ByTitle('Save As')-eq[IntPtr]::Zero)
$search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('109');Start-Sleep -Milliseconds 400
$list=ById '11';$item=$list.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ListItem)));$item.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select();Start-Sleep -Milliseconds 300
$null=Invoke-LabButton $h 'Copy';$legacy=[Windows.Forms.Clipboard]::GetText();$steps+=Get-LabWindowStep 'legacy-raw-fallback-to-final' $h
@{passed=($visibleCount-eq50-and$final-ceq'M3 final 108 ÅÄÖ'-and$rawText-ceq'M3 raw 108 åäö'-and$singleText-ceq$rawText-and([regex]::Matches($allText,'M3 (raw|final)')).Count-eq110-and$allText.Contains('M3 raw 000 åäö')-and$allText.Contains('M3 final 109 ÅÄÖ')-and([regex]::Matches($filteredText,'M3 raw')).Count-eq1-and$cancelled-and$legacy-ceq'M3 final 109 ÅÄÖ');visible_rows=$visibleCount;final_copy=$final;raw_copy=$rawText;single_export=$singleText;all_export_count=([regex]::Matches($allText,'M3 (raw|final)')).Count;all_export_status=$status;all_export_sha256=(Get-FileHash $all).Hash;filtered_export=$filteredText;cancel_preserved_file=$cancelled;legacy_raw_fallback=$legacy;artifact=Get-LabArtifact;steps=$steps}
