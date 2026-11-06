$script=$env:USERPROFILE+'\M2 Elevated Editor.ps1'
$code='Add-Type -AssemblyName System.Windows.Forms;[IO.File]::WriteAllText($env:USERPROFILE+"\M2 Elevated Identity.txt",([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator).ToString()+" PID="+$PID);$f=New-Object Windows.Forms.Form;$f.Text="M2 Elevated Editor";$f.Width=650;$f.Height=300;$t=New-Object Windows.Forms.TextBox;$t.Multiline=$true;$t.Dock="Fill";$t.Add_TextChanged({[IO.File]::WriteAllText($env:USERPROFILE+"\M2 Elevated Contents.txt",$t.Text)});$f.Controls.Add($t);[IO.File]::WriteAllText($env:USERPROFILE+"\M2 Elevated Contents.txt","");[void]$f.ShowDialog()'
[IO.File]::WriteAllText($script,$code)
$null=Start-Process powershell.exe -Verb RunAs -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+$script+'"'))
@{launched=$true}
