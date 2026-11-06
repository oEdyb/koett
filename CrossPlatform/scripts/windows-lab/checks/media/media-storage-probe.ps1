$ErrorActionPreference='Stop'
$window=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Transcribe media')))
function Item([string]$id){$window.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,$id)))}
$sample=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav'
$mediaDir=$env:APPDATA+'\DybergCo\Koett\data\Media Transcripts';$backup=$mediaDir+'.m5-storage-probe';$recovery=$env:APPDATA+'\DybergCo\Koett\data\Failed Transcripts'
if(Test-Path $backup){throw 'Storage probe backup already exists'}
$before=@(Get-ChildItem $recovery -File|Select-Object -ExpandProperty Name);$hadFolder=Test-Path $mediaDir
try{
 if($hadFolder){Move-Item $mediaDir $backup}
 [IO.File]::WriteAllText($mediaDir,'isolated M5 storage probe')
 ([Windows.Automation.ValuePattern](Item '101').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)).SetValue($sample)
 ([Windows.Automation.InvokePattern](Item '1').GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke()
 $watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 200;$new=@(Get-ChildItem $recovery -File|Where-Object {$_.Name-notin$before});$status=(Item '108').Current.Name}while($watch.ElapsedMilliseconds-lt30000-and($new.Count-ne1-or$status-match'Preparing|Transcribing|Converting|Inspecting'))
 $value=([Windows.Automation.TextPattern](Item '105').GetCurrentPattern([Windows.Automation.TextPattern]::Pattern)).DocumentRange.GetText(-1)
 @{recovery_created=($new.Count-eq1);recovery=@($new|Select-Object Name,Length);status=$status;text=$value;reproduced_warning_loss=($new.Count-eq1-and$status-eq'Saved');focus=Get-LabFocus}
}finally{
 if((Test-Path $mediaDir -PathType Leaf)-and[IO.File]::ReadAllText($mediaDir)-eq'isolated M5 storage probe'){Remove-Item $mediaDir}
 if($hadFolder){Move-Item $backup $mediaDir}
}
