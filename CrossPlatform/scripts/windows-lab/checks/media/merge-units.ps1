$ErrorActionPreference='Stop';$dir=$env:USERPROFILE+'\KoettM5Media';$results=@();$passed=$true
foreach($case in @(@('windows::media_view::model::tests',6),@('windows::pill_buttons::tests',2),@('windows::pill_state::tests',8),@('windows::pill_state::delivery_tests',2),@('windows::media_control::tests',1),@('windows::media_cli::tests',2),@('windows::media_view::layout::tests',1))){
 $info=New-Object Diagnostics.ProcessStartInfo;$info.FileName=$dir+'\MediaTests.exe';$info.Arguments=$case[0]+' --test-threads=1';$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $p=New-Object Diagnostics.Process;$p.StartInfo=$info;$null=$p.Start();$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();if(-not$p.WaitForExit(15000)){$p.Kill();throw 'Native timeout'}
 $text=[string]$out.Result;$ok=($p.ExitCode-eq0-and$text-match('test result: ok\. '+$case[1]+' passed; 0 failed;'));$passed=$passed-and$ok;$results+=@{filter=$case[0];exit=$p.ExitCode;expected=$case[1];passed=$ok;stdout=$text;stderr=[string]$err.Result};$p.Dispose()
}
@{passed=$passed;results=$results;focus=Get-LabFocus}
