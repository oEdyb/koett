param([Parameter(Mandatory)][string]$Manager)
$ErrorActionPreference='Stop'
$base=Join-Path $env:LOCALAPPDATA ('Koett Installer Recovery Lab '+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $base|Out-Null
$payload=Join-Path $base 'fixture';New-Item -ItemType Directory -Path $payload|Out-Null
$files=[ordered]@{}
foreach($name in @('0-first.txt','Koett.exe','koett-engine.exe','koett-transcribe-engine.exe','manage-preview.ps1','LICENSE','THIRD_PARTY_NOTICES.md','z-last.bin')){[IO.File]::WriteAllText((Join-Path $payload $name),('owned fixture '+$name));$files[$name]=(Get-FileHash (Join-Path $payload $name)).Hash.ToLowerInvariant()}
[IO.File]::WriteAllText((Join-Path $payload 'preview-release.json'),(@{format=1;signature='unsigned-preview';architecture='x86_64';files=$files}|ConvertTo-Json -Depth 5))
function Run-Manager([string]$Action,[string]$Root,[int]$FakeSession=-1){
 $wrapper=Join-Path $base 'invoke-manager.ps1'
 $code=if($FakeSession-ge0){'function global:Get-Process { [pscustomobject]@{SessionId='+$FakeSession+'} };'}else{''}
 $code+="& '"+$Manager.Replace("'","''")+"' -Action "+$Action+" -Root '"+$Root.Replace("'","''")+"' -Payload '"+$payload.Replace("'","''")+"' -MessagePath '"+(Join-Path $base 'message.txt').Replace("'","''")+"'"
 [IO.File]::WriteAllText($wrapper,($code+'; exit $LASTEXITCODE'))
 $i=New-Object Diagnostics.ProcessStartInfo;$i.FileName=$env:SystemRoot+'\System32\WindowsPowerShell\v1.0\powershell.exe';$i.Arguments='-NoProfile -ExecutionPolicy Bypass -File "'+$wrapper+'"';$i.UseShellExecute=$false;$i.CreateNoWindow=$true;$i.RedirectStandardOutput=$true;$i.RedirectStandardError=$true
 $p=[Diagnostics.Process]::Start($i);$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();if(-not$p.WaitForExit(20000)){$p.Kill();throw 'Manager exceeded 20 seconds'};$text=[string]$out.Result
 $r=@{exit=$p.ExitCode;result=($text|ConvertFrom-Json);stderr=[string]$err.Result};$p.Dispose();$r
}
function Require([bool]$Condition,[string]$Message){if(-not$Condition){throw $Message}}
function Current([string]$Root){$state=Get-Content (Join-Path $Root 'current.json') -Raw|ConvertFrom-Json;Join-Path $Root $state.current.Replace('/','\')}
$results=@();$shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) 'Koett Preview.lnk';$oldLink=if(Test-Path $shortcut){[IO.File]::ReadAllBytes($shortcut)}else{$null}
try{
 $root=Join-Path $base 'changed files';$install=Run-Manager Install $root;Require ($install.exit-eq0) 'Fixture install failed';$version=Current $root
 [IO.File]::WriteAllText((Join-Path $version 'LICENSE'),'changed owned file');Remove-Item (Join-Path $version 'Koett.exe');New-Item -ItemType Directory -Path (Join-Path $version 'tools')|Out-Null;[IO.File]::WriteAllText((Join-Path $version 'tools\personal.txt'),'keep my tools')
 $remove=Run-Manager Uninstall $root;Require ($remove.exit-eq0-and-not(Test-Path $root)) 'Changed/missing/unlisted uninstall stuck';$saved=$remove.result.preserved_files[0];Require (([IO.File]::ReadAllText((Join-Path $saved 'tools\personal.txt')))-eq'keep my tools') 'Added tools lost';Require (([IO.File]::ReadAllText((Join-Path $saved 'LICENSE')))-eq'changed owned file') 'Changed file lost';$results+=@{case='changed-missing-unlisted-tools';passed=$true;remove=$remove}
 foreach($kind in @('stage','removing')){
  $root=Join-Path $base $kind;$residue=Join-Path $root ($kind+'-'+[Guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $residue -Force|Out-Null;[IO.File]::WriteAllText((Join-Path $residue 'kept.txt'),'interrupted operation')
  $install=Run-Manager Install $root;Require ($install.exit-eq0) ('Residue repair failed: '+$kind);$saved=$install.result.preserved_files[0];Require (([IO.File]::ReadAllText((Join-Path $saved 'kept.txt')))-eq'interrupted operation') 'Residue bytes lost';$remove=Run-Manager Uninstall $root;Require ($remove.exit-eq0-and-not(Test-Path $root)) 'Residue reinstall/uninstall stuck';$results+=@{case=($kind+'-without-metadata');passed=$true;install=$install;remove=$remove}
 }
 $root=Join-Path $base 'same version repair';$install=Run-Manager Install $root;Require ($install.exit-eq0) 'Repair fixture install failed';$version=Current $root;[IO.File]::WriteAllText((Join-Path $version 'LICENSE'),'keep changed license');$repair=Run-Manager Install $root;Require ($repair.exit-eq0-and$repair.result.preserved_files.Count-eq1) 'Same-version repair failed';Require (([IO.File]::ReadAllText((Join-Path $repair.result.preserved_files[0] 'LICENSE')))-eq'keep changed license') 'Repair discarded changed files';$remove=Run-Manager Uninstall $root;Require ($remove.exit-eq0) 'Repaired fixture cleanup failed';$results+=@{case='same-version-repair-preserves-changes';passed=$true;repair=$repair;remove=$remove}
 $root=Join-Path $base 'partial delete';$install=Run-Manager Install $root;Require ($install.exit-eq0) 'Partial fixture install failed';$version=Current $root;$locked=Join-Path $version 'z-last.bin';$directoryAcl=Get-Acl $version;$fileAcl=Get-Acl $locked;$identity=[Security.Principal.WindowsIdentity]::GetCurrent().User
 $denyFile=New-Object Security.AccessControl.FileSystemAccessRule($identity,[Security.AccessControl.FileSystemRights]::Delete,[Security.AccessControl.AccessControlType]::Deny)
 $denyChild=New-Object Security.AccessControl.FileSystemAccessRule($identity,[Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles,[Security.AccessControl.AccessControlType]::Deny)
 $newDirectoryAcl=Get-Acl $version;$newDirectoryAcl.AddAccessRule($denyChild);Set-Acl $version $newDirectoryAcl;$newFileAcl=Get-Acl $locked;$newFileAcl.AddAccessRule($denyFile);Set-Acl $locked $newFileAcl
 try{$first=Run-Manager Uninstall $root}finally{$remaining=if(Test-Path $version){$version}else{(Get-ChildItem $root -Directory -Filter 'removing-*'|Select-Object -First 1).FullName};if($remaining){Set-Acl $remaining $directoryAcl;if(Test-Path (Join-Path $remaining 'z-last.bin')){Set-Acl (Join-Path $remaining 'z-last.bin') $fileAcl}}}
 [IO.File]::WriteAllText((Join-Path $base 'partial-delete-first.json'),($first|ConvertTo-Json -Depth 8))
 Require ($first.exit-ne0-and$first.result.error.Contains('removing-')-and$first.result.error.Contains('again')) ('Partial delete did not give a retryable path: '+($first|ConvertTo-Json -Depth 6 -Compress));$retry=Run-Manager Uninstall $root;Require ($retry.exit-eq0-and-not(Test-Path $root)) 'Partial-delete retry stuck';$saved=$retry.result.preserved_files[0];Require (Test-Path (Join-Path $saved 'z-last.bin')) 'Locked residue lost';$results+=@{case='partial-delete-and-retry';passed=$true;first=$first;retry=$retry}
 $root=Join-Path $base 'foreign session';$session=[Diagnostics.Process]::GetCurrentProcess().SessionId;$foreign=Run-Manager Install $root ($session+1);Require ($foreign.exit-eq0) 'Other session blocked install';$same=Run-Manager Uninstall $root $session;Require ($same.exit-ne0) 'Same-session Koett did not block removal';$remove=Run-Manager Uninstall $root;Require ($remove.exit-eq0) 'Final session fixture cleanup failed';$results+=@{case='session-filter-mocked-process-list';passed=$true;foreign=$foreign;same=$same;remove=$remove}
 @{passed=$true;cases=$results;manager_sha256=(Get-FileHash $Manager).Hash;fixture_root=$base}
}finally{if($oldLink){[IO.File]::WriteAllBytes($shortcut,$oldLink)}elseif(Test-Path $shortcut){Remove-Item $shortcut}}
