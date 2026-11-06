param([Parameter(Mandatory)][string]$Setup)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'helpers.ps1')
. (Join-Path $PSScriptRoot 'installer-messages.ps1')
try{Assert-InstallerText '{"ok":true}';throw 'JSON detector did not reject machine output'}catch{if($_.Exception.Message-notmatch'Installer exposed machine output'){throw}}
$p=Start-Process $Setup -PassThru
$watch=[Diagnostics.Stopwatch]::StartNew()
do{Start-Sleep -Milliseconds 100;$h=[LabDriver]::ByTitle('Koett Setup')}while($h-eq[IntPtr]::Zero-and$watch.ElapsedMilliseconds-lt10000)
$root=Wait-InstallerButton $h 'Install'
Click-InstallerButton $root 'Install'
$root=Wait-InstallerButton $h 'Finish'
$text=Read-InstallerText $root
if($text-notmatch'Launch Koett'){throw 'Launch checkbox missing'}
$check=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Launch Koett')))
$toggle=[Windows.Automation.TogglePattern]$check.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
if($toggle.Current.ToggleState-eq[Windows.Automation.ToggleState]::On){$toggle.Toggle()}
Start-Sleep -Milliseconds 400
$step=Get-LabWindowStep 'visible-installer-json-guard' $h
Click-InstallerButton $root 'Finish'
if(-not$p.WaitForExit(10000)-or$p.ExitCode-ne0){throw 'Setup did not finish successfully'}
@{passed=$true;visible_text=$text;step=$step;setup_sha256=(Get-FileHash $Setup).Hash}
