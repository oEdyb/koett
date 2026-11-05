$reader=$env:USERPROFILE+'\M2 Terminal Reader.ps1';$receipt=$env:USERPROFILE+'\M2 Terminal Receipt.txt'
if(Test-Path $receipt){Remove-Item $receipt}
$code='[Console]::Title="Koett M2 Terminal";[Console]::WriteLine("Dictation data reader. Input is saved as text, never evaluated.");$line=[Console]::ReadLine();[IO.File]::WriteAllText($env:USERPROFILE+"\M2 Terminal Receipt.txt",$line);[Console]::Clear();[Console]::WriteLine("Receipt saved.");$null=[Console]::ReadLine()'
[IO.File]::WriteAllText($reader,$code)
$null=Start-Process wt.exe -ArgumentList @('-w','new','powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+$reader+'"'))
Start-Sleep -Seconds 3
$terminal=$null
for($i=0;$i-lt30-and!$terminal;$i++){
 $terminal=[Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Children,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name-like'*Koett M2 Terminal*'}|Select-Object -First 1
 if(!$terminal){Start-Sleep -Milliseconds 200}
}
if(!$terminal){throw 'Terminal reader window missing'}
$h=[IntPtr]$terminal.Current.NativeWindowHandle;Activate-LabEditor $h
$focus=Get-LabFocus;Send-DictationCommand 9;Start-Sleep -Seconds 8;Send-DictationCommand 9;Start-Sleep -Milliseconds 800
$clipboard=[Windows.Forms.Clipboard]::GetText();$steps=@((Get-LabStep 'terminal-dictation-result'),(Get-LabWindowStep 'terminal-input-before-submit' $h))
[Windows.Forms.SendKeys]::SendWait('{ENTER}')
for($i=0;$i-lt30-and!(Test-Path $receipt);$i++){Start-Sleep -Milliseconds 100}
$text=$(if(Test-Path $receipt){[IO.File]::ReadAllText($receipt)}else{'NO RECEIPT'})
$steps+=Get-LabWindowStep 'terminal-receipt-saved-without-execution' $h
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$null=Invoke-WebRequest -UseBasicParsing -Method Post http://10.0.2.2:18768/history -InFile $db
@{passed=($text-ceq$clipboard-and$clipboard-match'Phoebe'-and(Get-LabFocus).hwnd-eq$focus.hwnd);artifact=Get-LabArtifact;receipt=$text;clipboard=$clipboard;scope='Windows Terminal with a Console.ReadLine data sink; transcript never evaluated as commands';original_focus=$focus;steps=$steps}
