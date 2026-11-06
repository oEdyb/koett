$ErrorActionPreference='Stop';$dir=$env:USERPROFILE+'\KoettM5Media';$results=@();$passed=$true
foreach($case in @(@('audio::tests',10),@('runtime::tests',5),@('windows::sounds::tests',3),@('windows::recovery::tests',1),@('windows::recovery::keyboard_tests',1),@('windows::pin_reader::privacy_tests',1),@('windows::desktop::dictation_options::tests',1),@('windows::clipboard::tests::native_bundle_round_trip_and_newer_copy_preservation',1))){
 $info=New-Object Diagnostics.ProcessStartInfo;$info.FileName=$dir+'\MediaTests.exe';$info.Arguments=$case[0]+' --test-threads=1';$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $p=New-Object Diagnostics.Process;$p.StartInfo=$info;$null=$p.Start();$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();if(-not$p.WaitForExit(15000)){$p.Kill();throw 'Native timeout'}
 $text=[string]$out.Result;$ok=($p.ExitCode-eq0-and$text-match('test result: ok\. '+$case[1]+' passed; 0 failed;'));$passed=$passed-and$ok;$results+=@{filter=$case[0];exit=$p.ExitCode;expected=$case[1];passed=$ok;stdout=$text;stderr=[string]$err.Result};$p.Dispose()
}
@{passed=$passed;results=$results;focus=Get-LabFocus}

$r=Run-MediaCli 'recovery';Start-Sleep -Milliseconds 400;$h=[LabDriver]::ByClass('KoettRecoveryWindow');@{open=$r;step=Get-LabWindowStep 'recovery-empty' $h;status=Run-MediaCli 'status'}
