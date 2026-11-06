if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop';$global:PreviewRoot=$env:LOCALAPPDATA+'\Programs\Koett M6 åä';$global:PreviewDir=$env:USERPROFILE+'\KoettM6Packages'
function Preview-Data {
 $base=$env:APPDATA+'\DybergCo\Koett';$hashes=@{}
 foreach($path in @(($base+'\config\settings.json'),($base+'\data\History.sqlite3'),($base+'\data\Last Transcript.txt'),($base+'\data\Dictionary.json'),($base+'\data\Replacements.json'))){if(Test-Path $path -PathType Leaf){$hashes[$path]=(Get-FileHash $path).Hash}}
 $hashes
}
function Preview-Run([string]$Action,[string]$Payload){
 $request=@{script=$global:PreviewDir+'\current\Koett-Windows-x86_64\manage-preview.ps1';action=$Action;payload=$Payload;root=$global:PreviewRoot}|ConvertTo-Json -Compress
 $base64=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($request));$i=New-Object Diagnostics.ProcessStartInfo;$i.FileName=$env:SystemRoot+'\System32\WindowsPowerShell\v1.0\powershell.exe';$i.Arguments='-NoProfile -ExecutionPolicy Bypass -File "'+$global:PreviewDir+'\manager-wrapper.ps1" -RequestBase64 '+$base64;$i.UseShellExecute=$false;$i.CreateNoWindow=$true;$i.RedirectStandardOutput=$true;$i.RedirectStandardError=$true;$i.StandardOutputEncoding=[Text.Encoding]::UTF8;$i.StandardErrorEncoding=[Text.Encoding]::UTF8
 $p=New-Object Diagnostics.Process;$p.StartInfo=$i;$null=$p.Start();$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();if(-not$p.WaitForExit(60000)){$p.Kill();throw 'Preview manager hung'};$text=[string]$out.Result;$result=@{action=$Action;exit=$p.ExitCode;stdout=$text;stderr=[string]$err.Result;json=($text|ConvertFrom-Json)};$p.Dispose();$result
}
function Preview-Current { $s=Get-Content ($global:PreviewRoot+'\current.json') -Raw|ConvertFrom-Json;Join-Path $global:PreviewRoot $s.current.Replace('/','\') }
function Preview-Launch {
 $path=(Preview-Current)+'\Koett.exe';$sample=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav';$i=New-Object Diagnostics.ProcessStartInfo;$i.FileName=$path;$i.Arguments='--dictation-lab-wav "'+$sample+'"';$i.UseShellExecute=$false;$i.CreateNoWindow=$true;$i.EnvironmentVariables['WEBVIEW2_BROWSER_EXECUTABLE_FOLDER']=$global:MediaDir+'\missing-browser-runtime';foreach($key in @('GROQ_API_KEY','OPENAI_API_KEY','CARTESIA_API_KEY')){$i.EnvironmentVariables[$key]=''};$p=[Diagnostics.Process]::Start($i)
 $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$r=Run-MediaCli 'status';if($r.exit-eq0){$state=$r.stdout|ConvertFrom-Json}}while(($r.exit-ne0-or$state.status-ne'ready')-and$watch.ElapsedMilliseconds-lt40000)
 if($r.exit-ne0-or$state.status-ne'ready'){throw 'Installed preview did not become ready'}
 @{pid=$p.Id;path=$path;hash=(Get-FileHash $path).Hash;status=$state;focus=Get-LabFocus}
}
function Preview-Quit { if(@(Get-Process Koett -ErrorAction SilentlyContinue).Count-eq0){return @{already_closed=$true}};$r=Run-MediaCli 'quit';if($r.exit-ne0){throw $r.stderr};$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$running=@(Get-Process Koett -ErrorAction SilentlyContinue)}while($running.Count-and$watch.ElapsedMilliseconds-lt10000);if($running.Count){throw 'Installed app did not quit'};$r }
