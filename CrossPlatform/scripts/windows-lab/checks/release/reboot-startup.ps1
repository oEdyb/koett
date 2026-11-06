if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$path=(Preview-Current)+'\Koett.exe';$run=(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run').Koett
@{passed=($run-ceq('"'+$path+'"'));startup=$run;target=$path;sha256=(Get-FileHash $path).Hash;network=@(Get-NetAdapter|Select-Object Name,Status);config=(Get-FileHash ($env:APPDATA+'\DybergCo\Koett\config\settings.json')).Hash;reboot_requested=$true}
Start-Process shutdown.exe -ArgumentList '/r /t 2' -WindowStyle Hidden
