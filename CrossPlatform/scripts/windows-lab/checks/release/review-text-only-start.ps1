if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop';$null=Run-MediaCli 'settings';Start-Sleep -Milliseconds 200;$settings=[Windows.Automation.AutomationElement]::FromHandle([LabDriver]::ByTitle('Koett Settings'))
$check=$settings.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,'40')));$toggle=[Windows.Automation.TogglePattern]$check.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern);if($toggle.Current.ToggleState-ne[Windows.Automation.ToggleState]::On){$toggle.Toggle()};$null=[LabDriver]::PostMessage([IntPtr]$settings.Current.NativeWindowHandle,0x111,[IntPtr]22,[IntPtr]::Zero);Start-Sleep -Milliseconds 200
$path=$global:MediaDir+'\pin-private-fixture.ps1'
@'
Add-Type -AssemblyName System.Windows.Forms
$f=New-Object Windows.Forms.Form;$f.Text='M6 text-only target';$f.Width=700;$f.Height=400
$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock='Fill';$t.Text='';$t.AccessibleName='M6 sensitive editor';$f.Controls.Add($t);$f.Add_Shown({$t.Focus();$t.SelectAll()});[void]$f.ShowDialog()
'@|Set-Content $path -Encoding UTF8
$p=Start-Process powershell.exe -ArgumentList ('-NoProfile -STA -ExecutionPolicy Bypass -File "'+$path+'"') -PassThru
$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$h=[LabDriver]::ByTitle('M6 text-only target')}while($h-eq[IntPtr]::Zero-and$watch.ElapsedMilliseconds-lt5000);if($h-eq[IntPtr]::Zero){throw 'Pin fixture missing'};Activate-LabEditor $h
$r=Run-MediaCli 'start';$watch=[Diagnostics.Stopwatch]::StartNew();do{Start-Sleep -Milliseconds 100;$s=(Run-MediaCli 'status').stdout|ConvertFrom-Json}while($s.status-ne'recording'-and$watch.ElapsedMilliseconds-lt10000)
$rect=[LabDriver]::RectOf($h);@{passed=($s.status-eq'recording');start=$r;target=$h.ToInt64();fixture_pid=$p.Id;rect=$rect;point=@(($rect[0]+200),($rect[1]+180));focus=Get-LabFocus;artifact=Get-LabArtifact}
