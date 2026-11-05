$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing,UIAutomationClient,UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class LabNative {
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int n);
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[LabNative]::SetProcessDPIAware() | Out-Null
function Get-LabFocus {
 $h=[LabNative]::GetForegroundWindow();$p=[uint32]0;[LabNative]::GetWindowThreadProcessId($h,[ref]$p)|Out-Null
 @{hwnd=$h.ToInt64();pid=$p;process=(Get-Process -Id $p -ErrorAction SilentlyContinue).ProcessName}
}
function Get-LabScreenshot {
 $b=[System.Windows.Forms.SystemInformation]::VirtualScreen
 $image=New-Object System.Drawing.Bitmap $b.Width,$b.Height
 $g=[System.Drawing.Graphics]::FromImage($image);$g.CopyFromScreen($b.Left,$b.Top,0,0,$image.Size)
 $m=New-Object IO.MemoryStream;$image.Save($m,[Drawing.Imaging.ImageFormat]::Png)
 $encoded=[Convert]::ToBase64String($m.ToArray());$g.Dispose();$image.Dispose();$m.Dispose();$encoded
}
$serial=New-Object IO.Ports.SerialPort COM1,115200,None,8,one
$serial.Encoding=New-Object Text.UTF8Encoding
$null=[Windows.Automation.AutomationElement]::RootElement
$proxy=[Reflection.Assembly]::Load('UIAutomationClientsideProviders, Version=4.0.0.0, Culture=neutral, PublicKeyToken=31bf3856ad364e35')
[Windows.Automation.ClientSettings]::RegisterClientSideProviderAssembly($proxy.GetName())
$serial.ReadTimeout=30000;$serial.WriteTimeout=60000;$serial.Open()
$serial.WriteLine('{"ready":true}')
while($true){
 try { $line=$serial.ReadLine() } catch [TimeoutException] {continue}
 try {
  $request=$line|ConvertFrom-Json
  $script=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($request.code))
  $result=&([scriptblock]::Create($script))
  $serial.WriteLine((@{id=$request.id;ok=$true;result=$result}|ConvertTo-Json -Depth 15 -Compress))
 } catch { $serial.WriteLine((@{id=$request.id;ok=$false;error=$_.ToString()}|ConvertTo-Json -Compress)) }
}
