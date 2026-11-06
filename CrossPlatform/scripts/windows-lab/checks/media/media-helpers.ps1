$ErrorActionPreference='Stop'
$global:MediaDir=$env:USERPROFILE+'\KoettM5Media'
function Run-MediaCli([string]$action){
 $info=New-Object Diagnostics.ProcessStartInfo;$info.FileName=$global:MediaDir+'\koett-engine.exe';$info.Arguments='app '+$action;$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $p=New-Object Diagnostics.Process;$p.StartInfo=$info;$null=$p.Start();$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();if(-not$p.WaitForExit(5000)){$p.Kill();throw 'App CLI hung'}
 $result=@{action=$action;exit=$p.ExitCode;stdout=[string]$out.Result;stderr=[string]$err.Result};$p.Dispose();$result
}
function Wait-MediaWindow {
 $watch=[Diagnostics.Stopwatch]::StartNew();do{$h=[LabDriver]::ByClass('KoettMediaWindow');if($h-ne[IntPtr]::Zero){return [Windows.Automation.AutomationElement]::FromHandle($h)};Start-Sleep -Milliseconds 100}while($watch.ElapsedMilliseconds-lt10000);throw 'Media window missing'
}
function Media-Item([string]$id){$window=Wait-MediaWindow;$e=$window.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,$id)));if(-not$e){throw ('Media item missing: '+$id)};$e}
function Media-Source([string]$source){([Windows.Automation.ValuePattern](Media-Item '101').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).SetValue($source)}
function Media-Invoke([string]$id){([Windows.Automation.InvokePattern](Media-Item $id).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()}
function Media-Text {([Windows.Automation.TextPattern](Media-Item '105').GetCurrentPattern([Windows.Automation.TextPattern]::Pattern)).DocumentRange.GetText(-1)}
function Wait-MediaDone {
 $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$r=Run-MediaCli 'status';if($r.exit-ne0){throw $r.stderr};$status=$r.stdout|ConvertFrom-Json}while(($status.media_active-or$status.status-ne'ready')-and$watch.ElapsedMilliseconds-lt40000)
 if($status.media_active-or$status.status-ne'ready'){throw ('Media did not finish: '+$r.stdout)};Start-Sleep -Milliseconds 250;$status
}
function Media-Step([string]$name){Get-LabWindowStep $name ([LabDriver]::ByClass('KoettMediaWindow'))}
