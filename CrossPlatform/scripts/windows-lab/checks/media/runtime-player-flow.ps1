$ErrorActionPreference='Stop'
function Player-State {
 $browser=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ClassNameProperty,'Chrome_WidgetWin_1')))
 @($browser.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Text)))|Where-Object {$_.Current.Name-match'^(Playing|Paused) [0-9]'}|ForEach-Object {$_.Current.Name}) -join ';'
}
$before=Get-LabFocus;$playing=Player-State;$start=Run-MediaCli 'start'
$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$state=(Run-MediaCli 'status').stdout|ConvertFrom-Json}while($state.status-ne'recording'-and$watch.ElapsedMilliseconds-lt15000)
$paused=Player-State;$during=Get-LabFocus
$pause=Run-MediaCli 'pause';$pausedCapture=(Run-MediaCli 'status').stdout|ConvertFrom-Json
$resume=Run-MediaCli 'resume';$resumedCapture=(Run-MediaCli 'status').stdout|ConvertFrom-Json
$cancel=Run-MediaCli 'cancel';$ready=Wait-MediaDone;Start-Sleep -Milliseconds 300;$resumed=Player-State;$after=Get-LabFocus
@{passed=($start.exit-eq0-and$state.status-eq'recording'-and$playing-match'^Playing'-and$paused-match'^Paused'-and$pausedCapture.paused-and-not$resumedCapture.paused-and$resumed-match'^Playing'-and$before.hwnd-eq$during.hwnd-and$before.pid-eq$during.pid-and$before.hwnd-eq$after.hwnd-and$before.pid-eq$after.pid);start=$start;capture=$state;player_before=$playing;player_during=$paused;player_after=$resumed;paused_capture=$pausedCapture;resumed_capture=$resumedCapture;cancel=$cancel;before=$before;during=$during;after=$after;artifact=Get-LabArtifact}
