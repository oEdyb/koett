$path=(Get-LabArtifact).path
$owner=[LabDriver]::ByClass('KoettOwnerWindow')
if($owner-ne[IntPtr]::Zero){Send-DictationCommand 4;Start-Sleep -Seconds 2}
$sample=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav'
$p=Start-Process $path -ArgumentList @('--dictation-lab-wav',$sample) -PassThru
Start-Sleep -Seconds 8
@{artifact=Get-LabArtifact;phase=Get-DictationPhase;sample=$sample;sample_sha256=(Get-FileHash $sample).Hash;focus=Get-LabFocus}
