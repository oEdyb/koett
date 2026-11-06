if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop'
if(-not('M5Resolution' -as[type])){Add-Type @'
using System;using System.Runtime.InteropServices;public class M5Resolution{
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern bool EnumDisplaySettingsEx(string d,int mode,IntPtr dm,uint flags);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern int ChangeDisplaySettingsEx(string d,IntPtr dm,IntPtr h,uint flags,IntPtr p);
 public static int[] Set(int width,int height){IntPtr p=Marshal.AllocHGlobal(220);try{Marshal.Copy(new byte[220],0,p,220);Marshal.WriteInt16(p,68,220);if(!EnumDisplaySettingsEx(@"\\.\DISPLAY1",-1,p,0))throw new Exception("EnumDisplaySettings failed");int oldWidth=Marshal.ReadInt32(p,172),oldHeight=Marshal.ReadInt32(p,176);Marshal.WriteInt32(p,72,0x180000);Marshal.WriteInt32(p,172,width);Marshal.WriteInt32(p,176,height);int result=ChangeDisplaySettingsEx(@"\\.\DISPLAY1",p,IntPtr.Zero,1,IntPtr.Zero);return new int[]{oldWidth,oldHeight,width,height,result};}finally{Marshal.FreeHGlobal(p);}}
}
'@}
$r=[M5Resolution]::Set(1920,1080);if($r[4]-ne0){throw ('Guest mode change failed: '+$r[4])}
@{mode=$r;focus=Get-LabFocus}

$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json';$hash=(Get-FileHash $config).Hash;$backup=$env:APPDATA+'\DybergCo\Koett.m6-before-fresh';$run=(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Koett -ErrorAction SilentlyContinue).Koett
$apps=@(Get-Process Koett -ErrorAction SilentlyContinue);$narrator=@(Get-Process Narrator -ErrorAction SilentlyContinue)
@{passed=($hash-eq'2E95162308D31C1A0BE6460393C020FCFBEDE6E161E6BEC1CD0ECD2CF5B6D3FF'-and-not(Test-Path $backup)-and-not(Test-Path $global:PreviewRoot)-and-not$run-and$apps.Count-eq0-and$narrator.Count-eq0);config_hash=$hash;profile_backup_absent=(-not(Test-Path $backup));preview_removed=(-not(Test-Path $global:PreviewRoot));startup=$run;app_count=$apps.Count;narrator_count=$narrator.Count;mode=$r;shutdown_requested=$true;focus=Get-LabFocus}
Start-Process shutdown.exe -ArgumentList '/s /t 2' -WindowStyle Hidden
