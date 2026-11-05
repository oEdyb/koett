$h=[IntPtr]::Zero;for($i=0;$i-lt40-and$h-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 200;$h=[LabDriver]::ByTitle('M2 Elevated Editor')}
if($h-eq[IntPtr]::Zero){throw 'Elevated editor missing'}
$identity=[IO.File]::ReadAllText($env:USERPROFILE+'\M2 Elevated Identity.txt')
if($identity-notmatch'^True PID='){throw "Target not elevated: $identity"}
Activate-LabEditor $h;$focus=Get-LabFocus
Send-DictationCommand 9;Start-Sleep -Seconds 8;Send-DictationCommand 9;Start-Sleep -Milliseconds 800
$overlay=[LabDriver]::ByClass('KoettOverlayWindow');$phase=Get-DictationPhase
$root=[Windows.Automation.AutomationElement]::FromHandle($overlay);$names=@($root.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current.Name})
$text=[IO.File]::ReadAllText($env:USERPROFILE+'\M2 Elevated Contents.txt')
$clipboard=[Windows.Forms.Clipboard]::GetText();$steps=@((Get-LabStep 'elevated-editor-visible-paste-recovery'))
$null=Invoke-LabButton $overlay 'Copy';$copied=[Windows.Forms.Clipboard]::GetText()
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$null=Invoke-WebRequest -UseBasicParsing -Method Post http://10.0.2.2:18768/history -InFile $db
@{passed=($identity-match'^True PID='-and$text-eq''-and$phase-match'Not pasted'-and'Copy'-in$names-and'Paste'-in$names-and$clipboard-match'Phoebe'-and$copied-ceq$clipboard);artifact=Get-LabArtifact;elevated_identity=$identity;target_focus=$focus;editor_text=$text;phase=$phase;actions=$names;clipboard=$clipboard;copied=$copied;steps=$steps}
