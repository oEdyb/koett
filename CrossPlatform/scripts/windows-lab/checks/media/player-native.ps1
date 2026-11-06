$ErrorActionPreference='Stop';$dir=$env:USERPROFILE+'\KoettM5Media';$results=@();$passed=$true
foreach($name in @('native_player_pause_resumes_once','an_already_paused_player_is_never_resumed','user_playback_changes_revoke_resume','user_seek_revokes_resume')){
 $i=New-Object Diagnostics.ProcessStartInfo;$i.FileName=$dir+'\MediaTests.exe';$i.Arguments=('windows::media_control::tests::'+$name+' --ignored --exact --nocapture --test-threads=1');$i.UseShellExecute=$false;$i.CreateNoWindow=$true;$i.RedirectStandardOutput=$true;$i.RedirectStandardError=$true
 $p=New-Object Diagnostics.Process;$p.StartInfo=$i;$null=$p.Start();$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();if(-not$p.WaitForExit(15000)){$p.Kill();throw ('Native player check hung: '+$name)};$text=[string]$out.Result;$ok=$p.ExitCode-eq0-and$text-match'1 passed; 0 failed';$passed=$passed-and$ok;$results+=@{name=$name;passed=$ok;exit=$p.ExitCode;stdout=$text;stderr=[string]$err.Result;focus=Get-LabFocus};$p.Dispose()
}
@{passed=$passed;results=$results;artifact=Get-LabArtifact}
