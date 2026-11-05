$ErrorActionPreference='Stop'
$ready=$env:USERPROFILE+'\m2-sqlite-lock.ready';$release=$env:USERPROFILE+'\m2-sqlite-lock.release';$script=$env:USERPROFILE+'\m2-sqlite-lock.ps1'
foreach($p in @($ready,$release)){if(Test-Path $p){Remove-Item $p}}
if(!(Test-Path ($env:WINDIR+'\System32\winsqlite3.dll'))){throw 'Windows SQLite test helper unavailable; do not replace system DLLs'}
$code=@'
Add-Type @"
using System;using System.Runtime.InteropServices;
public class SqliteLease {
 [DllImport("winsqlite3.dll",CallingConvention=CallingConvention.Cdecl)]public static extern int sqlite3_open(byte[] name,out IntPtr db);
 [DllImport("winsqlite3.dll",CallingConvention=CallingConvention.Cdecl,CharSet=CharSet.Ansi)]public static extern int sqlite3_exec(IntPtr db,string sql,IntPtr callback,IntPtr arg,out IntPtr error);
 [DllImport("winsqlite3.dll",CallingConvention=CallingConvention.Cdecl)]public static extern int sqlite3_close(IntPtr db);
 [DllImport("winsqlite3.dll",CallingConvention=CallingConvention.Cdecl)]public static extern void sqlite3_free(IntPtr p);
}
"@
$db=[IntPtr]::Zero;$errorPtr=[IntPtr]::Zero
try {
 $rc=[SqliteLease]::sqlite3_open([Text.Encoding]::UTF8.GetBytes($env:APPDATA+'\DybergCo\Koett\data\History.sqlite3'+[char]0),[ref]$db)
 if($rc-ne0){throw "sqlite open failed: $rc"}
 $rc=[SqliteLease]::sqlite3_exec($db,'BEGIN EXCLUSIVE',[IntPtr]::Zero,[IntPtr]::Zero,[ref]$errorPtr)
 if($rc-ne0){throw "sqlite lock failed: $rc"}
 [IO.File]::WriteAllText($env:USERPROFILE+'\m2-sqlite-lock.ready','exclusive')
 $deadline=[DateTime]::UtcNow.AddSeconds(30)
 while(!(Test-Path ($env:USERPROFILE+'\m2-sqlite-lock.release'))-and[DateTime]::UtcNow-lt$deadline){Start-Sleep -Milliseconds 100}
}finally{
 if($errorPtr-ne[IntPtr]::Zero){[SqliteLease]::sqlite3_free($errorPtr);$errorPtr=[IntPtr]::Zero}
 if($db-ne[IntPtr]::Zero){$null=[SqliteLease]::sqlite3_exec($db,'ROLLBACK',[IntPtr]::Zero,[IntPtr]::Zero,[ref]$errorPtr);$null=[SqliteLease]::sqlite3_close($db)}
 if($errorPtr-ne[IntPtr]::Zero){[SqliteLease]::sqlite3_free($errorPtr)}
}
'@
[IO.File]::WriteAllText($script,$code)
$a=(Get-Process Notepad|Where-Object {$_.MainWindowHandle-ne0}|Select-Object -First 1).MainWindowHandle
Activate-LabEditor $a;$editor=Find-Editor $a;$editor.SetFocus();[Windows.Forms.SendKeys]::SendWait('^a{BACKSPACE}');Start-Sleep -Milliseconds 200
$db=$env:APPDATA+'\DybergCo\Koett\data\History.sqlite3';$beforeHash=(Get-FileHash $db).Hash
Send-DictationCommand 9;Start-Sleep -Seconds 8
$lease=Start-Process powershell.exe -WindowStyle Hidden -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+$script+'"')) -PassThru
try {
 for($i=0;$i-lt50-and!(Test-Path $ready);$i++){Start-Sleep -Milliseconds 100}
 if(!(Test-Path $ready)){throw 'Exclusive SQLite lease did not start'}
 Activate-LabEditor $a;$editor.SetFocus()
 $clock=[Diagnostics.Stopwatch]::StartNew();Send-DictationCommand 9
 $text='';for($i=0;$i-lt15;$i++){Start-Sleep -Milliseconds 100;$text=Read-Editor $editor;if($text-match'Phoebe'){break}}
 $visibleMs=$clock.Elapsed.TotalMilliseconds;$clip=[Windows.Forms.Clipboard]::GetText()
 $leaseHeld=(!$lease.HasExited-and(Test-Path $ready)-and!(Test-Path $release))
 $steps=@((Get-LabWindowStep 'text-visible-while-history-database-exclusively-locked' $a))
 $visibleWhileLocked=($text.TrimEnd([char[]]"`r`n")-ceq$clip-and$clip-match'Phoebe'-and$visibleMs-lt2000-and$leaseHeld)
}finally{[IO.File]::WriteAllText($release,'release');$lease.WaitForExit(5000)|Out-Null}
Start-Sleep -Seconds 1
$afterHash=(Get-FileHash $db).Hash
@{passed=($visibleWhileLocked-and$afterHash-ne$beforeHash);artifact=Get-LabArtifact;visible_before_unlock=$visibleWhileLocked;visible_ms=$visibleMs;text=$text;clipboard=$clip;exclusive_lease_held=$leaseHeld;database_changed_after_unlock=($afterHash-ne$beforeHash);lease_exited=$lease.HasExited;steps=$steps}
