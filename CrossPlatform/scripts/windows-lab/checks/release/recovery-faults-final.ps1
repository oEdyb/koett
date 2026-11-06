if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop';$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$fallback=$env:APPDATA+'\DybergCo\Koett\data\Failed Transcripts';$backup=$fallback+'.m6-recovery-backup';$steps=@()
if(Test-Path $backup){throw 'Previous fault backup exists'}
$h=[LabDriver]::ByClass('KoettRecoveryWindow');if($h-eq[IntPtr]::Zero){$null=Run-MediaCli 'recovery';Start-Sleep -Milliseconds 400;$h=[LabDriver]::ByClass('KoettRecoveryWindow')}
function Recovery-Item([string]$id){$w=[Windows.Automation.AutomationElement]::FromHandle([LabDriver]::ByClass('KoettRecoveryWindow'));$w.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,$id)))}
function Recovery-Invoke([string]$id){([Windows.Automation.InvokePattern](Recovery-Item $id).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()}
function Wait-Recovery { $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$s=(Run-MediaCli 'status').stdout|ConvertFrom-Json}while($s.recovery_busy-and$watch.ElapsedMilliseconds-lt15000);if($s.recovery_busy){throw 'Recovery did not finish'};$s }
$moved=Test-Path $fallback;$readOnly=(Get-Item $db).IsReadOnly
try {
 if($moved){Move-Item $fallback $backup};[IO.File]::WriteAllText($fallback,'M6 guest-only recovery fault');(Get-Item $db).IsReadOnly=$true
 Activate-LabEditor $h
 $start=Run-MediaCli 'start';$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$s=(Run-MediaCli 'status').stdout|ConvertFrom-Json}while($s.status-ne'recording'-and$watch.ElapsedMilliseconds-lt10000);if($s.status-ne'recording'){throw 'Capture did not start'}
 Start-Sleep -Milliseconds 7550;$stop=Run-MediaCli 'stop';$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$s=(Run-MediaCli 'status').stdout|ConvertFrom-Json}while(($s.status-ne'ready'-or$s.unsaved_transcripts-ne1)-and$watch.ElapsedMilliseconds-lt20000);if($s.unsaved_transcripts-ne1){throw ('Pending transcript missing: '+($s|ConvertTo-Json -Compress))}
 $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$label=(Recovery-Item '101').Current.Name;$text=([Windows.Automation.TextPattern](Recovery-Item '102').GetCurrentPattern([Windows.Automation.TextPattern]::Pattern)).DocumentRange.GetText(-1)}while(($label-notmatch'1 unsaved'-or$text-notmatch'Phoebe')-and$watch.ElapsedMilliseconds-lt5000)
 $live=($label-match'1 unsaved'-and$text-match'Phoebe');$steps+=Get-LabWindowStep 'unsaved-window-updates-live' $h
 $quit=Run-MediaCli 'quit';if($quit.exit-eq0){throw 'CLI Quit dropped unsaved text'}
 $owner=[LabDriver]::ByClass('KoettOwnerWindow');$null=[LabDriver]::PostMessage($owner,0x111,[IntPtr]4,[IntPtr]::Zero);Start-Sleep -Milliseconds 250
 $dialog=[LabDriver]::ByClass('#32770');if($dialog-eq[IntPtr]::Zero){throw 'Native Quit warning missing'};$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$default=[Windows.Automation.AutomationElement]::FocusedElement.Current.Name}while($default-ne'No'-and$watch.ElapsedMilliseconds-lt2500)
 $defaultId=([LabDriver]::SendMessage($dialog,0x400,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()-band0xffff);$steps+=Get-LabWindowStep 'quit-warning-default-no-keyboard' $dialog
 if($default-eq'No'-and$defaultId-eq7){[Windows.Forms.SendKeys]::SendWait('{ENTER}')}else{$null=[LabDriver]::PostMessage($dialog,0x111,[IntPtr]7,[IntPtr]::Zero)};Start-Sleep -Milliseconds 150
 [Windows.Forms.Clipboard]::SetText('M6 clipboard sentinel');$before=Get-LabFocus;$copy=Run-MediaCli 'copy-unsaved';$null=Wait-Recovery;$copied=[Windows.Forms.Clipboard]::GetText();$after=Get-LabFocus
 $copyOk=($copy.exit-eq0-and$copied.Trim()-ceq$text.Trim()-and$before.hwnd-eq$after.hwnd-and$before.pid-eq$after.pid)
 Recovery-Invoke '3';$failed=Wait-Recovery;$failedMessage=(Recovery-Item '101').Current.Name;$steps+=Get-LabWindowStep 'retry-failed-raw-text-retained' $h
 $still=([Windows.Automation.TextPattern](Recovery-Item '102').GetCurrentPattern([Windows.Automation.TextPattern]::Pattern)).DocumentRange.GetText(-1)
 $failureKept=($failed.unsaved_transcripts-eq1-and$still-ceq$text-and$failedMessage-match'fail|denied|saved|access|read.only')
}finally{
 (Get-Item $db).IsReadOnly=$readOnly
 if(Test-Path $fallback -PathType Leaf){Remove-Item $fallback};if($moved){Move-Item $backup $fallback}
}
$beforeSave=(Get-FileHash $db).Hash;$retry=Run-MediaCli 'retry-save';$saved=Wait-Recovery;$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$savedMessage=(Recovery-Item '101').Current.Name}while($savedMessage-ne'All transcripts saved'-and$watch.ElapsedMilliseconds-lt5000);$afterSave=(Get-FileHash $db).Hash
$again=Run-MediaCli 'retry-save';$null=Wait-Recovery;$afterAgain=(Get-FileHash $db).Hash;$steps+=Get-LabWindowStep 'retry-saved-once' $h
@{passed=($live-and$copyOk-and$default-eq'No'-and$defaultId-eq7-and$failedMessage.Length-le80-and$failureKept-and$saved.unsaved_transcripts-eq0-and$beforeSave-ne$afterSave-and$afterSave-eq$afterAgain-and$savedMessage-eq'All transcripts saved');live_update=$live;native_quit_default=$default;native_quit_default_id=$defaultId;cli_quit=$quit;copy=$copy;copy_without_focus_change=$copyOk;before=$before;after=$after;text=$text;copied=$copied;failed_message=$failedMessage;failed_text_retained=$failureKept;retry=$retry;saved=$saved;hash_before=$beforeSave;hash_after=$afterSave;hash_repeat=$afterAgain;restored_readonly=(Get-Item $db).IsReadOnly;fallback_restored=(Test-Path $fallback -PathType Container);steps=$steps;artifact=Get-LabArtifact}
