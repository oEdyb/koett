
$ErrorActionPreference='Stop'
[Windows.Forms.SendKeys]::SendWait('{ESC}')
if(-not ('LabMotion' -as [type])){Add-Type @'
using System;using System.Runtime.InteropServices;
public class LabMotion{
 [DllImport("user32.dll",EntryPoint="SystemParametersInfoW")]public static extern bool Set(uint a,uint u,IntPtr v,uint f);
 [DllImport("user32.dll",EntryPoint="SystemParametersInfoW")]public static extern bool Get(uint a,uint u,out int v,uint f);
}
'@}
$old=0;[LabMotion]::Get(0x1042,0,[ref]$old,0)|Out-Null
$set=[LabMotion]::Set(0x1043,0,[IntPtr]::Zero,3)
$animation=1;[LabMotion]::Get(0x1042,0,[ref]$animation,0)|Out-Null
[LabDriver]::SetCursorPos(20,20)|Out-Null
$steps=@();Set-LabPhase 2;$steps+=Get-LabStep 'reduced-motion-transcribing-1'
Start-Sleep -Milliseconds 400;$steps+=Get-LabStep 'reduced-motion-transcribing-2'
Set-LabPhase 3;$steps+=Get-LabStep 'reduced-motion-done'
Start-Sleep -Milliseconds 1800;$steps+=Get-LabStep 'reduced-motion-done-expired'
[LabMotion]::Set(0x1043,0,[IntPtr]$old,3)|Out-Null
@{set=$set;animation_enabled=$animation;old=$old;steps=$steps;artifact='e63e65a'}
