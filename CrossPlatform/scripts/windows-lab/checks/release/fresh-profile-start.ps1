if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop';$null=Preview-Quit;$base=$env:APPDATA+'\DybergCo\Koett';$backup=$base+'.m6-before-fresh';if(Test-Path $backup){throw 'Previous profile backup exists'}
$before=Preview-Data;$run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run';$startup=(Get-ItemProperty $run -Name Koett -ErrorAction SilentlyContinue).Koett
@{base=$base;backup=$backup;before=$before;startup=$startup}|ConvertTo-Json -Depth 6|Set-Content ($global:PreviewDir+'\fresh-state.json') -Encoding UTF8
Move-Item -LiteralPath $base -Destination $backup;if($startup){Remove-ItemProperty $run -Name Koett}
$noProfile=(-not(Test-Path $base));$launch=Preview-Launch
[Windows.Forms.Clipboard]::SetText('fresh preview sentinel');$copy=Run-MediaCli 'copy-last';$unsaved=Run-MediaCli 'copy-unsaved';$paste=Run-MediaCli 'paste-last';$clip=[Windows.Forms.Clipboard]::GetText()
@{passed=($noProfile-and$launch.status.status-eq'ready'-and$copy.exit-ne0-and$unsaved.exit-ne0-and$paste.exit-ne0-and$clip-eq'fresh preview sentinel');fresh_roaming_data=$noProfile;cache_preseeded=$true;same_windows_account=$true;launch=$launch;copy_empty=$copy;copy_unsaved_empty=$unsaved;paste_empty=$paste;clipboard=$clip;backup=$backup;before=$before;network=@(Get-NetAdapter|Select-Object Name,Status);artifact=Get-LabArtifact}
