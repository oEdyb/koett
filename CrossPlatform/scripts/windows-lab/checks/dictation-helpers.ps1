if(-not('DictationLab'-as[type])){Add-Type @'
using System;using System.Runtime.InteropServices;
public class DictationLab {
 [DllImport("user32.dll")]static extern void keybd_event(byte k,byte scan,uint flags,UIntPtr extra);
 public static void Key(byte key,bool down){keybd_event(key,0,down?0u:2u,UIntPtr.Zero);}
}
'@}
function Send-DictationCommand([int]$id){$h=[LabDriver]::ByClass('KoettOwnerWindow');if($h-eq[IntPtr]::Zero){throw 'Koett is not running'};$null=[LabDriver]::PostMessage($h,0x111,[IntPtr]$id,[IntPtr]::Zero)}
function Get-DictationPhase { $h=[LabDriver]::ByClass('KoettOverlayWindow');[Windows.Automation.AutomationElement]::FromHandle($h).Current.Name }
function Read-Editor([Windows.Automation.AutomationElement]$editor){
 $pattern=$null
 if($editor.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$pattern)){return $pattern.Current.Value}
 $pattern=$editor.GetCurrentPattern([Windows.Automation.TextPattern]::Pattern);$pattern.DocumentRange.GetText(-1)
}
function Find-Editor([IntPtr]$window){
 $root=[Windows.Automation.AutomationElement]::FromHandle($window)
 $condition=New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::Document)
 $editor=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
 if(!$editor){$condition=New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::ControlTypeProperty),([Windows.Automation.ControlType]::Edit);$editor=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)}
 if(!$editor){throw 'Editable UIA document not found'};$editor
}
