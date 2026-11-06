$ErrorActionPreference='Stop'
$dir=$env:USERPROFILE+'\KoettM5Local';$exe=$dir+'\koett-transcribe-engine.exe'
$input=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav'
$labProfile=$env:APPDATA+'\DybergCo\Koett';$paths=@('config\settings.json','data\History.sqlite3','data\Last Transcript.txt' | ForEach-Object {Join-Path $labProfile $_})
function Hash-Profile { $r=@{};foreach($path in $paths){if(Test-Path $path){$r[$path]=(Get-FileHash $path).Hash}};$r }
$before=Hash-Profile;if($before.Count-ne3){throw 'Expected all three original profile hashes'};$inputBefore=(Get-FileHash $input).Hash
function Run-CLI([string]$name,[string]$arguments,[string]$modelDir){
 $i=New-Object Diagnostics.ProcessStartInfo;$i.FileName=$exe;$i.Arguments=$arguments;$i.UseShellExecute=$false;$i.CreateNoWindow=$true;$i.RedirectStandardOutput=$true;$i.RedirectStandardError=$true
 if($modelDir){$i.EnvironmentVariables['KOETT_MODEL_DIR']=$modelDir}
 $i.EnvironmentVariables['GROQ_API_KEY']='';$i.EnvironmentVariables['OPENAI_API_KEY']='';$i.EnvironmentVariables['CARTESIA_API_KEY']=''
 $p=New-Object Diagnostics.Process;$p.StartInfo=$i;$focus=Get-LabFocus
 if(-not$p.Start()){throw 'Could not start CLI'}
 $stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
 if(-not$p.WaitForExit(40000)){$p.Kill();throw ('Timed out: '+$name)}
 $r=@{name=$name;exit=$p.ExitCode;stdout=[string]$stdout.Result;stderr=[string]$stderr.Result;before=$focus;after=Get-LabFocus};$p.Dispose();$r
}
$output=$dir+'\transcript å.json'
$success=Run-CLI 'offline-compact' ('--offline --model-version compact --results-json "'+$output+'" "'+$input+'"') ''
$result=[IO.File]::ReadAllText($output)|ConvertFrom-Json
if($success.exit-ne0-or$result.Count-ne1-or-not$result[0].text){throw ('Offline decode failed: '+($success|ConvertTo-Json -Compress))}
$missing=$dir+'\absent-model';$failedOutput=$dir+'\failure.json'
$missingRun=Run-CLI 'offline-missing-model' ('--offline --model-version compact --results-json "'+$failedOutput+'" "'+$input+'"') $missing
$overwrite=Run-CLI 'refuse-input-overwrite' ('--offline --model-version compact --results-json "'+$input+'" "'+$input+'"') ''
$invalid=Run-CLI 'reject-malformed-call' '--offline' ''
$after=Hash-Profile;$unchanged=($before|ConvertTo-Json -Compress)-eq($after|ConvertTo-Json -Compress)
@{passed=($unchanged-and$missingRun.exit-ne0-and$overwrite.exit-ne0-and$invalid.exit-ne0-and-not(Test-Path $missing)-and-not(Test-Path $failedOutput)-and$inputBefore-eq(Get-FileHash $input).Hash);results=@($success,$missingRun,$overwrite,$invalid);transcript=$result;profile_before=$before;profile_after=$after;input_hash=$inputBefore;exe_hash=(Get-FileHash $exe).Hash;network=@(Get-NetAdapter|Select-Object Name,Status,LinkSpeed);focus=Get-LabFocus}
