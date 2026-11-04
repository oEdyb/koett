$ErrorActionPreference='Stop'
$target=[LabDriver]::ByTitle('Koett Focus Target')
if($target-eq[IntPtr]::Zero){throw 'Run the states check before the original-target guard check'}
$code='Add-Type -AssemblyName System.Windows.Forms;$f=New-Object Windows.Forms.Form;$f.Text="Koett Second Target";$f.Width=700;$f.Height=300;$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock="Fill";$f.Controls.Add($t);$f.Add_Shown({$f.Activate();$t.Focus()});[void]$f.ShowDialog()'
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code))
$second=[LabDriver]::ByTitle('Koett Second Target')
if($second-eq[IntPtr]::Zero){$null=Start-Process powershell.exe -ArgumentList @('-NoProfile','-STA','-EncodedCommand',$encoded)}
for($i=0;$i-lt40-and$second-eq[IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 200;$second=[LabDriver]::ByTitle('Koett Second Target')}
if($second-eq[IntPtr]::Zero){throw 'Second editor did not open'}
Activate-LabEditor $target
Start-Sleep -Milliseconds 200
Set-LabPhase 1
Set-LabPhase 4
$original=Get-LabFocus
Activate-LabEditor $second
Start-Sleep -Milliseconds 200
$switched=Get-LabFocus
$overlay=[LabDriver]::ByClass('KoettOverlayWindow')
$null=Invoke-LabButton $overlay 'Paste'
Start-Sleep -Milliseconds 250
$text=[LabEdit]::Read([LabDriver]::GetWindow($second,5))
$after=Get-LabFocus
@{passed=($text-eq''-and$after.hwnd-eq$switched.hwnd);artifact=(Get-LabArtifact);original=$original;switched=$switched;after=$after;second_editor_text=$text;refused_new_target=($text-eq''-and$after.hwnd-eq$switched.hwnd);steps=@((Get-LabStep 'uia-changed-target-refused'),(Get-LabWindowStep 'second-editor-untouched' $second))}
