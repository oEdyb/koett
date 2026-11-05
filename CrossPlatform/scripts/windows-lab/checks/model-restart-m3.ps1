$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$before=[IO.File]::ReadAllText($config)|ConvertFrom-Json;$hash=(Get-FileHash $config).Hash
$app=Get-LabArtifact;Send-DictationCommand 4;Start-Sleep -Seconds 1
$sample=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav'
$path=$env:PATH;try{$env:PATH=[IO.File]::ReadAllText($env:USERPROFILE+'\m3-llama-path.txt')+';'+$path;$null=Start-Process $app.path -ArgumentList @('--dictation-lab-wav',$sample) -RedirectStandardError ($env:USERPROFILE+'\m3-koett-error.log')}finally{$env:PATH=$path}
Start-Sleep -Seconds 8;Send-DictationCommand 5;Start-Sleep -Milliseconds 400;$null=Invoke-LabButton ([LabDriver]::ByTitle('Koett Settings')) 'Model and formatting…';Start-Sleep -Milliseconds 400
$h=[LabDriver]::ByTitle('Koett Model and Formatting');$root=[Windows.Automation.AutomationElement]::FromHandle($h)
function Model-Control([string]$id){$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::AutomationIdProperty),$id))}
$index=[LabDriver]::SendMessage([IntPtr](Model-Control '60').Current.NativeWindowHandle,0x147,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()
$status=(Model-Control '62').Current.Name;$labels=@();foreach($pair in @(@('64','Style'),@('65','Structure'),@('66','Context'))){$n=(Model-Control $pair[0]).Current.Name;$labels+=@{id=$pair[0];name=$n;passed=($n-ceq$pair[1])}}
$expected=@('compact','parakeet_v2','parakeet_v3','nemotron').IndexOf($before.selected_model)
@{passed=($index-eq$expected-and$status-match'^Ready:'-and(Get-FileHash $config).Hash-eq$hash-and@($labels|Where-Object {!$_.passed}).Count-eq0);model=$before.selected_model;selected_index=$index;expected_index=$expected;status=$status;settings_hash_unchanged=((Get-FileHash $config).Hash-eq$hash);accessible_labels=$labels;artifact=Get-LabArtifact;steps=@((Get-LabWindowStep 'model-persists-across-restart-and-native-labels' $h))}
