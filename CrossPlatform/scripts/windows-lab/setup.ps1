$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force C:\KoettLab | Out-Null
Copy-Item "$PSScriptRoot\bridge.ps1" C:\KoettLab\bridge.ps1
Expand-Archive "$PSScriptRoot\Koett.zip" C:\KoettLab -Force
$action=New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -WindowStyle Minimized -ExecutionPolicy Bypass -File C:\KoettLab\bridge.ps1'
$principal=New-ScheduledTaskPrincipal -UserId "$env:COMPUTERNAME\KoettLab" -LogonType Interactive -RunLevel Limited
$trigger=New-ScheduledTaskTrigger -AtLogOn -User "$env:COMPUTERNAME\KoettLab"
Register-ScheduledTask -TaskName 'KoettLabBridge' -Action $action -Principal $principal -Trigger $trigger -Force | Out-Null
Start-ScheduledTask -TaskName 'KoettLabBridge'
