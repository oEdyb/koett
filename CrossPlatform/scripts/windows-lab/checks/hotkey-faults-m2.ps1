$ErrorActionPreference='Stop'
if(-not('HotkeyLab'-as[type])){Add-Type @'
using System;using System.Runtime.InteropServices;
public class HotkeyLab {
 [StructLayout(LayoutKind.Sequential)]struct ID{public uint Size;public IntPtr Window;public uint Id;public Guid Guid;}
 [StructLayout(LayoutKind.Sequential)]struct R{public int Left,Top,Right,Bottom;}
 [DllImport("shell32.dll")]static extern int Shell_NotifyIconGetRect(ref ID id,out R r);
 [DllImport("user32.dll")]public static extern IntPtr GetDlgItem(IntPtr h,int id);
 public static int[] Rect(IntPtr owner){var id=new ID{Size=(uint)Marshal.SizeOf(typeof(ID)),Window=owner,Id=1,Guid=new Guid("c65ddae4-119c-486e-8bed-e30a24f2fb1f")};R r;int hr=Shell_NotifyIconGetRect(ref id,out r);return new[]{hr,r.Left,r.Top,r.Right,r.Bottom};}
}
'@}
$exe=(Get-LabArtifact).path;$sample=$env:LOCALAPPDATA+'\DybergCo\Koett\data\models\sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8\test_wavs\0.wav'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$original=[IO.File]::ReadAllText($config)
$ready=$env:USERPROFILE+'\m2-hotkey.ready';$release=$env:USERPROFILE+'\m2-hotkey.release';$script=$env:USERPROFILE+'\m2-hotkey.ps1'
foreach($p in @($ready,$release)){if(Test-Path $p){Remove-Item $p}}
$code=@'
Add-Type @"
using System;using System.Runtime.InteropServices;
public class HotkeyLease {
 [DllImport("user32.dll",SetLastError=true)]public static extern bool RegisterHotKey(IntPtr h,int id,uint mods,uint key);
 [DllImport("user32.dll")]public static extern bool UnregisterHotKey(IntPtr h,int id);
}
"@
if(![HotkeyLease]::RegisterHotKey([IntPtr]::Zero,501,6,32)){throw 'Could not reserve Ctrl+Shift+Space'}
try{
 [IO.File]::WriteAllText($env:USERPROFILE+'\m2-hotkey.ready','reserved')
 $deadline=[DateTime]::UtcNow.AddSeconds(60)
 while(!(Test-Path ($env:USERPROFILE+'\m2-hotkey.release'))-and[DateTime]::UtcNow-lt$deadline){Start-Sleep -Milliseconds 100}
}finally{$null=[HotkeyLease]::UnregisterHotKey([IntPtr]::Zero,501)}
'@
[IO.File]::WriteAllText($script,$code)
Send-DictationCommand 4;Start-Sleep -Seconds 1
$settings=$original|ConvertFrom-Json;$settings.shortcut='Ctrl+Shift+Space';$settings.recording_mode='hold'
[IO.File]::WriteAllText($config,($settings|ConvertTo-Json -Depth 20),(New-Object Text.UTF8Encoding($false)))
$lease=Start-Process powershell.exe -WindowStyle Hidden -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+$script+'"')) -PassThru
$steps=@()
try{
 for($i=0;$i-lt50-and!(Test-Path $ready);$i++){Start-Sleep -Milliseconds 100}
 if(!(Test-Path $ready)){throw 'Hotkey lease did not start'}
 $null=Start-Process $exe -ArgumentList @('--dictation-lab-wav',$sample);Start-Sleep -Seconds 8
 $root=[Windows.Automation.AutomationElement]::RootElement
 $hidden=$root.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::Button)))|Where-Object {$_.Current.Name-match'^Show hidden icons$'}|Select-Object -First 1
 if($hidden){$hidden.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke();Start-Sleep -Milliseconds 300}
 $r=[HotkeyLab]::Rect([LabDriver]::ByClass('KoettOwnerWindow'));if($r[0]-ne0){throw 'Koett tray icon not found'}
 $null=[LabDriver]::SetCursorPos((($r[1]+$r[3])/2),(($r[2]+$r[4])/2));Start-Sleep -Milliseconds 1500
 $tips=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::ToolTip)))|ForEach-Object {$_.Current.Name})
 $warning=@($tips|Where-Object {$_-match'Shortcut unavailable'}).Count-gt0
 $steps+=@{step='hotkey-conflict-visible-tray-warning';focus=Get-LabFocus;tooltip=$tips;screenshot=(Get-LabCrop ([Math]::Max(0,$r[1]-450)) ([Math]::Max(0,$r[2]-180)) 550 220)}
 [Windows.Forms.SendKeys]::SendWait('{ESC}')
 $a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle
 Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus()
 try{[DictationLab]::Key(17,$true);[DictationLab]::Key(16,$true);[DictationLab]::Key(32,$true);Start-Sleep -Milliseconds 600;$blockedPhase=Get-DictationPhase}finally{[DictationLab]::Key(32,$false);[DictationLab]::Key(16,$false);[DictationLab]::Key(17,$false)}
 Send-DictationCommand 9;Start-Sleep -Milliseconds 500;$trayPhase=Get-DictationPhase
 Send-DictationCommand 8;Start-Sleep -Milliseconds 500
 $steps+=Get-LabStep 'tray-recording-remains-available-during-hotkey-conflict'
}finally{
 [IO.File]::WriteAllText($release,'release');$lease.WaitForExit(5000)|Out-Null
}
try{
 Send-DictationCommand 4;Start-Sleep -Seconds 1
 $null=Start-Process $exe -ArgumentList @('--dictation-lab-wav',$sample);Start-Sleep -Seconds 8
 Activate-LabEditor $a;$editor.SetFocus()
 try{[DictationLab]::Key(17,$true);[DictationLab]::Key(16,$true);[DictationLab]::Key(32,$true);Start-Sleep -Milliseconds 600;$recoveredPhase=Get-DictationPhase}finally{[DictationLab]::Key(32,$false);[DictationLab]::Key(16,$false);[DictationLab]::Key(17,$false)}
 Start-Sleep -Seconds 1;$releasedPhase=Get-DictationPhase;$steps+=Get-LabStep 'hotkey-recovers-after-conflict-is-released-and-app-restarts'
}finally{[IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))}
@{passed=($warning-and$blockedPhase-notmatch'Recording'-and$trayPhase-match'Recording'-and$recoveredPhase-match'Recording'-and$releasedPhase-notmatch'Recording');artifact=Get-LabArtifact;warning=$tips;conflict_phase=$blockedPhase;tray_phase=$trayPhase;recovered_phase=$recoveredPhase;released_phase=$releasedPhase;settings_restored=([IO.File]::ReadAllText($config)-ceq$original);lease_exited=$lease.HasExited;steps=$steps}
