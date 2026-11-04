$serial.Encoding=New-Object Text.UTF8Encoding
if(-not $global:LabProxiesRegistered){$null=[Windows.Automation.AutomationElement]::RootElement;$proxy=[Reflection.Assembly]::Load('UIAutomationClientsideProviders, Version=4.0.0.0, Culture=neutral, PublicKeyToken=31bf3856ad364e35');[Windows.Automation.ClientSettings]::RegisterClientSideProviderAssembly($proxy.GetName());$global:LabProxiesRegistered=$true}
if(-not ('LabDriver' -as [type])) { Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class LabDriver {
 [StructLayout(LayoutKind.Sequential)] private struct R { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] private static extern IntPtr FindWindow(string c,string t);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] private static extern int GetWindowTextLength(IntPtr h);
 [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h,uint c);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")]public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 public static IntPtr ByClass(string c){return FindWindow(c,null);}
 public static IntPtr ByTitle(string t){return FindWindow(null,t);}
 public static int[] RectOf(IntPtr h){R r;GetWindowRect(h,out r);return new int[]{r.Left,r.Top,r.Right,r.Bottom};}
 public static string Text(IntPtr h){var s=new StringBuilder(GetWindowTextLength(h)+1);GetWindowText(h,s,s.Capacity);return s.ToString();}
}
'@
}
function Get-PillRect { $h=[LabDriver]::ByClass('KoettOverlayWindow');$r=[LabDriver]::RectOf($h);@{left=$r[0];top=$r[1];right=$r[2];bottom=$r[3];dpi=[LabDriver]::GetDpiForWindow($h);hwnd=$h.ToInt64()} }
function Set-LabPhase([int]$phase){$h=[LabDriver]::ByClass('KoettOwnerWindow');if($h-eq[IntPtr]::Zero){throw 'Koett owner window missing'};[LabDriver]::PostMessage($h,0x8052,[IntPtr]$phase,[IntPtr]::Zero)|Out-Null;Start-Sleep -Milliseconds 120}
function Get-LabCrop([int]$x,[int]$y,[int]$width,[int]$height){
 $image=New-Object Drawing.Bitmap $width,$height
 $g=[Drawing.Graphics]::FromImage($image);$g.CopyFromScreen($x,$y,0,0,$image.Size)
 $m=New-Object IO.MemoryStream;$image.Save($m,[Drawing.Imaging.ImageFormat]::Png)
 $encoded=[Convert]::ToBase64String($m.ToArray());$g.Dispose();$image.Dispose();$m.Dispose();$encoded
}
function Get-LabStep([string]$name){$r=Get-PillRect;@{step=$name;focus=Get-LabFocus;rect=$r;visible=[LabDriver]::IsWindowVisible([LabDriver]::ByClass('KoettOverlayWindow'));screenshot=(Get-LabCrop ($r.left-8) ($r.top-8) ($r.right-$r.left+16) ($r.bottom-$r.top+16))}}
function Get-LabWindowStep([string]$name,[IntPtr]$window){$r=[LabDriver]::RectOf($window);@{step=$name;focus=Get-LabFocus;rect=$r;screenshot=(Get-LabCrop $r[0] $r[1] ($r[2]-$r[0]) ($r[3]-$r[1]))}}
function Invoke-LabButton([IntPtr]$window,[string]$name){
 $root=[Windows.Automation.AutomationElement]::FromHandle($window)
 $condition=New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),$name
 $element=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
 if(!$element){throw "Native button missing: $name"}
 $pattern=$element.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
 $information=@{name=$element.Current.Name;control=$element.Current.ControlType.ProgrammaticName;invoke=$true}
 $pattern.Invoke();Start-Sleep -Milliseconds 120
 $information
}

[LabDriver]::SetThreadDpiAwarenessContext([IntPtr](-4))|Out-Null
