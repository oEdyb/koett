
$ErrorActionPreference='Stop'
$path=Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Koett-export-e63.txt'
$expected="Fixture: Swedish ÅÄÖ — Unicode stays intact.`nSecond line for history Copy and Export."
$exported=[IO.File]::ReadAllText($path)
$steps=@((Get-LabWindowStep 'history-export-file-verified' ([LabDriver]::ByTitle('Koett History'))))
$results=@()
foreach($light in @(0,1)){
 Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -Name AppsUseLightTheme -Value $light
 foreach($title in @('Koett Settings','Koett History')){[LabDriver]::PostMessage([LabDriver]::ByTitle($title),0x31A,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null}
 Start-Sleep -Milliseconds 400
 Set-LabPhase 1
 $steps+=Get-LabStep $(if($light){'light-pill-e63'}else{'dark-pill-e63'})
 Set-LabPhase 8
 [LabNative]::SetForegroundWindow([LabDriver]::ByTitle('Koett Settings'))|Out-Null
 Start-Sleep -Milliseconds 200
 $steps+=Get-LabWindowStep $(if($light){'light-settings-e63'}else{'dark-settings-e63'}) ([LabDriver]::ByTitle('Koett Settings'))
 $results+=@{light=$light;focus=Get-LabFocus}
}
@{artifact='e63e65a';export=@{path=$path;text=$exported;exact=($exported-ceq$expected);sha256=(Get-FileHash $path).Hash};themes=$results;steps=$steps}
