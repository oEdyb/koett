$ErrorActionPreference='Stop';$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$hash=(Get-FileHash $config).Hash
Send-DictationCommand 5;Start-Sleep -Milliseconds 200;$h=[LabDriver]::ByTitle('Koett Settings');Activate-LabEditor $h
$null=Invoke-LabButton $h 'Paste shortcuts…'
$clock=[Diagnostics.Stopwatch]::StartNew()
do {Start-Sleep -Milliseconds 100;$menu=[LabDriver]::ByClass('#32768');$visible=($menu-ne[IntPtr]::Zero-and[LabDriver]::IsWindowVisible($menu))}while($visible-and$clock.ElapsedMilliseconds-lt20000)
@{passed=(-not$visible-and(Get-FileHash $config).Hash-eq$hash);focus=Get-LabFocus;scope='Root menu cancelled using physical guest Escape through QEMU, not injected host input.'}
