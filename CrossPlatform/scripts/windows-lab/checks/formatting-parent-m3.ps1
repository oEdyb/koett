$ErrorActionPreference='Stop'
$app=Get-LabArtifact;$owned=@(Get-CimInstance Win32_Process -Filter "name='llama-server.exe'"|Where-Object {$_.ParentProcessId-eq$app.pid})
if($owned.Count-ne1){throw 'Expected one Koett-owned formatter child before crash test'}
$pidChild=$owned[0].ProcessId;Stop-Process -Id $app.pid -Force
for($i=0;$i-lt30;$i++){Start-Sleep -Milliseconds 100;if(!(Get-Process -Id $pidChild -ErrorAction SilentlyContinue)){break}}
$alive=$null-ne(Get-Process -Id $pidChild -ErrorAction SilentlyContinue)
@{passed=(!$alive);parent_pid=$app.pid;child_pid=$pidChild;child_alive_after_three_seconds=$alive;artifact=$app;steps=@((Get-LabStep 'parent-crash-reaps-formatter-child'))}
