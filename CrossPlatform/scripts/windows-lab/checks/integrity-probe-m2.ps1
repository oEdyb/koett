if(-not('IntegrityProbe'-as[type])){Add-Type @'
using System;using System.Runtime.InteropServices;
public class IntegrityProbe{
 [DllImport("kernel32.dll")]static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("advapi32.dll")]static extern bool OpenProcessToken(IntPtr p,uint access,out IntPtr t);
 [DllImport("advapi32.dll")]static extern bool GetTokenInformation(IntPtr t,int kind,IntPtr data,int size,out int required);
 [DllImport("kernel32.dll")]static extern bool CloseHandle(IntPtr h);
 [DllImport("advapi32.dll")]static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
 [DllImport("advapi32.dll")]static extern IntPtr GetSidSubAuthority(IntPtr sid,uint index);
 public static int Level(int pid){IntPtr p=OpenProcess(0x1000,false,pid),t=IntPtr.Zero,b=IntPtr.Zero;try{if(!OpenProcessToken(p,8,out t))return -1;int n;GetTokenInformation(t,25,IntPtr.Zero,0,out n);b=Marshal.AllocHGlobal(n);if(!GetTokenInformation(t,25,b,n,out n))return -2;IntPtr sid=Marshal.ReadIntPtr(b);byte count=Marshal.ReadByte(GetSidSubAuthorityCount(sid));return Marshal.ReadInt32(GetSidSubAuthority(sid,(uint)(count-1)));}finally{if(b!=IntPtr.Zero)Marshal.FreeHGlobal(b);CloseHandle(t);CloseHandle(p);}}
}
'@}
$a=Get-LabArtifact
@{koett_pid=$a.pid;koett_integrity=[IntegrityProbe]::Level($a.pid);editor_pid=(Get-LabFocus).pid;editor_integrity=[IntegrityProbe]::Level((Get-LabFocus).pid);focus=Get-LabFocus}
