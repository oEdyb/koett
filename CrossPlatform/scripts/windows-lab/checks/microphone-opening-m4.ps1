$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config)
$rounds=@();$artifact=Get-LabArtifact
try{
 foreach($inputName in @('default','named')){
  $settings=$original|ConvertFrom-Json
  $settings.microphone=if($inputName-eq'default'){$null}else{'wasapi:{0.0.1.00000000}.{00000000-0000-0000-0000-000000000000}'}
  [IO.File]::WriteAllText($config,($settings|ConvertTo-Json -Depth 20),(New-Object Text.UTF8Encoding($false)))
  foreach($round in 1..2){
   $clock=[Diagnostics.Stopwatch]::StartNew();$seen=@()
   [DictationLab]::Key(0x11,$true);[DictationLab]::Key(0x10,$true);[DictationLab]::Key(0x20,$true)
   Start-Sleep -Milliseconds 80
   [DictationLab]::Key(0x20,$false);[DictationLab]::Key(0x10,$false);[DictationLab]::Key(0x11,$false)
   do{
    Start-Sleep -Milliseconds 100
    $workers=@(Get-CimInstance Win32_Process -Filter "name='Koett.exe'"|Where-Object {$_.CommandLine-match'windows-microphone-worker'}|ForEach-Object {[int]$_.ProcessId})
    foreach($worker in $workers){if($worker-notin$seen){$seen+=$worker}}
    $phase=Get-DictationPhase
   }while($clock.ElapsedMilliseconds-lt9000-and($seen.Count-eq0-or$workers.Count-gt0-or$clock.ElapsedMilliseconds-lt4500))
   $rounds+=@{input=$inputName;round=$round;elapsed_ms=$clock.ElapsedMilliseconds;observed_worker_pids=$seen;remaining_worker_pids=$workers;phase=$phase;focus=Get-LabFocus;visible=[LabDriver]::IsWindowVisible([LabDriver]::ByClass('KoettOverlayWindow'));passed=($seen.Count-gt0-and$workers.Count-eq0-and$phase-match'timed out'-and$clock.ElapsedMilliseconds-lt7500)}
  }
 }
}finally{[IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))}
@{passed=(@($rounds|Where-Object {-not$_.passed}).Count-eq0);artifact=$artifact;rounds=$rounds;scope='Start Koett with --microphone-open-timeout-lab and Ctrl+Shift+Space toggle settings first. Injected child stalls, no physical microphone.'}
