$ErrorActionPreference='Stop'
$dir=$env:USERPROFILE+'\KoettM4Local'
$cases=@(
 @('windows::drawing::gesture::tests',15),@('windows::draw_input::tests',2),@('windows::clipboard::tests',4),
 @('windows::contact_sheet::tests',1),@('windows::paste_timing::tests',1),@('windows::paste_guard::tests',5),
 @('windows::pin_policy::tests',2),@('windows::pin_policy::video_tests',1),@('windows::drawing_render::tests',1),@('windows::pixels::tests',1),@('windows::foreground::tests',1),
 @('audio::windows_capture::tests',5),@('audio::windows_capture::preview_proxy::tests',1),@('audio::tests::drawing_clock',1),
 @('audio::fixture::tests::finite_fixture',1))
$results=@();$passed=$true
foreach($case in $cases){
 $filter=$case[0];$expected=$case[1]
 $info=New-Object Diagnostics.ProcessStartInfo
 $info.FileName=$dir+'\Koett-M4-Tests.exe';$info.Arguments=$filter+' --test-threads=1 --nocapture'
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true
 $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $process=New-Object Diagnostics.Process;$process.StartInfo=$info
 if(-not$process.Start()){throw ('Could not start '+$filter)}
 $stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
 if(-not$process.WaitForExit(30000)){$process.Kill();throw ('Test timeout: '+$filter)}
 $output=[string]$stdout.Result;$errorText=[string]$stderr.Result;$exit=$process.ExitCode
 $ok=($exit-eq0-and$output-match('test result: ok\. '+$expected+' passed; 0 failed;'))
 $results+=@{filter=$filter;expected=$expected;exit=$exit;passed=$ok;output=$output;error=$errorText}
 $passed=$passed-and$ok;$process.Dispose()
}
@{passed=$passed;focus=Get-LabFocus;results=$results;app_sha256=(Get-FileHash ($dir+'\Koett.exe')).Hash;tests_sha256=(Get-FileHash ($dir+'\Koett-M4-Tests.exe')).Hash}
