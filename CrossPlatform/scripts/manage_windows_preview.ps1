[CmdletBinding(SupportsShouldProcess)]
param(
 [ValidateSet('Install','Rollback','Uninstall','Status')][string]$Action='Status',
 [string]$Payload,
 [string]$Root=(Join-Path $env:LOCALAPPDATA 'Programs\Koett-Preview')
)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
function Read-Json([string]$Path){Get-Content -LiteralPath $Path -Raw -Encoding UTF8|ConvertFrom-Json}
function Hash([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
function Under-Root([string]$Path){[IO.Path]::GetFullPath($Path).StartsWith($Root+'\',[StringComparison]::OrdinalIgnoreCase)}
function Verify-Payload([string]$Directory){
 if((Get-Item -LiteralPath $Directory -Force).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Payload root is a reparse point'}
 $release=Read-Json (Join-Path $Directory 'preview-release.json')
 if($release.format-ne1-or$release.signature-ne'unsigned-preview'-or$release.architecture-notin@('x86_64','aarch64')){throw 'Unsupported preview format'}
 $nativeArch=[Environment]::GetEnvironmentVariable('PROCESSOR_ARCHITEW6432');if(-not$nativeArch){$nativeArch=$env:PROCESSOR_ARCHITECTURE}
 if(($release.architecture-eq'x86_64'-and$nativeArch-ne'AMD64')-or($release.architecture-eq'aarch64'-and$nativeArch-ne'ARM64')){throw 'Use the package built for this Windows architecture'}
 $expected=@('preview-release.json')
 foreach($entry in $release.files.PSObject.Properties){
  $name=$entry.Name
  if($name-match'\\|:|^/|(^|/)\.{1,2}(/|$)' -or ($name.Split('/')|Where-Object {$_-eq''-or$_-match'[. ]$|[<>:"|?*\x00-\x1f]|^(?i:con|prn|aux|nul|com[1-9\u00b9\u00b2\u00b3]|lpt[1-9\u00b9\u00b2\u00b3])(?:\.|$)'})){throw 'Unsafe payload path'}
  $path=Join-Path $Directory $name.Replace('/','\')
  if(-not[IO.Path]::GetFullPath($path).StartsWith([IO.Path]::GetFullPath($Directory)+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Payload path escapes its directory'}
  if($entry.Value-notmatch'^[a-f0-9]{64}$'-or(Hash $path)-cne$entry.Value){throw ('Payload hash mismatch: '+$name)}
  $expected+=$name
 }
 $all=@(Get-ChildItem -LiteralPath $Directory -Recurse -Force)
 if(@($all|Where-Object {$_.Attributes-band[IO.FileAttributes]::ReparsePoint}).Count){throw 'Payload contains a reparse point'}
 $actual=@($all|Where-Object {-not$_.PSIsContainer}|ForEach-Object {$_.FullName.Substring($Directory.Length+1).Replace('\','/')})
 if(Compare-Object $expected $actual){throw 'Payload contains missing or unlisted files'}
 foreach($name in @('Koett.exe','koett-engine.exe','koett-transcribe-engine.exe','manage-preview.ps1','LICENSE','THIRD_PARTY_NOTICES.md')){if(-not(Test-Path -LiteralPath (Join-Path $Directory $name) -PathType Leaf)){throw ('Missing payload file: '+$name)}}
 $release
}
function Read-State {
 foreach($name in @('preview-owner.json','current.json','versions')){
  $path=Join-Path $Root $name
  if((Get-Item -LiteralPath $path -Force).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Installation metadata or versions contain a reparse point'}
 }
 $owner=Read-Json (Join-Path $Root 'preview-owner.json')
 if($owner.format-ne1-or-not[String]::Equals($owner.root,$Root,[StringComparison]::OrdinalIgnoreCase)){throw 'This directory is not an owned Koett preview installation'}
 $state=Read-Json (Join-Path $Root 'current.json')
 foreach($version in @($state.current,$state.previous)|Where-Object {$_}){if($version-notmatch'^versions/[a-f0-9]{16}$'){throw 'Unsafe installed version reference'}}
 $state
}
function Own-Startup([string]$Value){$Value-and$Value.StartsWith('"'+$Root+'\versions\',[StringComparison]::OrdinalIgnoreCase)-and$Value.EndsWith('\Koett.exe"',[StringComparison]::OrdinalIgnoreCase)}
function Activate([string]$Current,[string]$Previous){
 $statePath=Join-Path $Root 'current.json';$oldState=if(Test-Path $statePath){[IO.File]::ReadAllBytes($statePath)}else{$null}
 $run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run';$oldRun=(Get-ItemProperty -LiteralPath $run -Name Koett -ErrorAction SilentlyContinue).Koett
 $shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) 'Koett Preview.lnk';$shell=New-Object -ComObject WScript.Shell
 if(Test-Path $shortcut){$target=$shell.CreateShortcut($shortcut).TargetPath;if(-not(Under-Root $target)){throw 'The Koett Preview shortcut belongs to another installation'}}
 $oldLink=if(Test-Path $shortcut){[IO.File]::ReadAllBytes($shortcut)}else{$null}
 $target=Join-Path (Join-Path $Root $Current.Replace('/','\')) 'Koett.exe'
 $temp=$statePath+'.'+[Guid]::NewGuid().ToString('N')
 try {
  $link=$shell.CreateShortcut($shortcut);$link.TargetPath=$target;$link.WorkingDirectory=[IO.Path]::GetDirectoryName($target);$link.Save()
  if(Own-Startup $oldRun){Set-ItemProperty -LiteralPath $run -Name Koett -Value ('"'+$target+'"')}
  $state=@{format=1;current=$Current;previous=$Previous};[IO.File]::WriteAllText($temp,($state|ConvertTo-Json -Compress))
  if(Test-Path $statePath){[IO.File]::Replace($temp,$statePath,[System.Management.Automation.Language.NullString]::Value)}else{[IO.File]::Move($temp,$statePath)}
 }catch{
  if($oldLink){[IO.File]::WriteAllBytes($shortcut,$oldLink)}elseif(Test-Path $shortcut){Remove-Item -LiteralPath $shortcut}
  if(Own-Startup $oldRun){Set-ItemProperty -LiteralPath $run -Name Koett -Value $oldRun}
  if($oldState){[IO.File]::WriteAllBytes($statePath,$oldState)}elseif(Test-Path $statePath){Remove-Item -LiteralPath $statePath}
  throw
 }finally{if(Test-Path $temp){Remove-Item -LiteralPath $temp}}
}
try {
 $Root=[IO.Path]::GetFullPath($Root).TrimEnd('\')
 $ancestor=$Root;while($ancestor){if((Test-Path -LiteralPath $ancestor)-and((Get-Item -LiteralPath $ancestor -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)){throw 'Preview install path contains a reparse point'};$ancestor=[IO.Path]::GetDirectoryName($ancestor)}
 if(-not$Root.StartsWith([IO.Path]::GetFullPath($env:LOCALAPPDATA)+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Preview root must be below LocalAppData'}
 if($Root-eq[IO.Path]::GetFullPath($env:LOCALAPPDATA)){throw 'Unsafe preview root'}
 if(Test-Path $Root){if((Get-Item -LiteralPath $Root -Force).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Preview root is a reparse point'};$state=Read-State}else{$state=$null}
 if($Action-eq'Status'){@{ok=$true;installed=[bool]($state-and$state.current);root=$Root;state=$state;signature='unsigned-preview';outputs=@();warnings=@()}|ConvertTo-Json -Depth 6 -Compress;exit 0}
 if(@(Get-Process Koett,koett-engine,koett-transcribe-engine -ErrorAction SilentlyContinue).Count){throw 'Close Koett and its transcription commands before changing the preview'}
 if(-not$PSCmdlet.ShouldProcess($Root,$Action)){@{ok=$true;changed=$false;outputs=@();warnings=@()}|ConvertTo-Json -Compress;exit 0}
 switch($Action){
  Install {
   if(-not$Payload){throw 'Install requires -Payload pointing at the extracted archive folder'}
   $Payload=[IO.Path]::GetFullPath($Payload).TrimEnd('\');$release=Verify-Payload $Payload
   $key=(Hash (Join-Path $Payload 'preview-release.json')).Substring(0,16);$relative='versions/'+$key;$version=Join-Path $Root $relative.Replace('/','\')
   if(-not$state){
    New-Item -ItemType Directory -Path (Join-Path $Root 'versions') -Force|Out-Null
    [IO.File]::WriteAllText((Join-Path $Root 'preview-owner.json'),(@{format=1;root=$Root}|ConvertTo-Json -Compress))
    [IO.File]::WriteAllText((Join-Path $Root 'current.json'),'{"format":1,"current":null,"previous":null}')
   }
   if(-not(Test-Path $version)){
    $stage=Join-Path $Root ('stage-'+[Guid]::NewGuid().ToString('N'))
    try{New-Item -ItemType Directory -Path $stage|Out-Null;Get-ChildItem -LiteralPath $Payload -Force|ForEach-Object {Copy-Item -LiteralPath $_.FullName -Destination $stage -Recurse -Force};$null=Verify-Payload $stage;[IO.Directory]::Move($stage,$version)}finally{if(Test-Path $stage){Remove-Item -LiteralPath $stage -Recurse -Force}}
   }else{$null=Verify-Payload $version;if((Hash (Join-Path $version 'preview-release.json'))-cne(Hash (Join-Path $Payload 'preview-release.json'))){throw 'Installed version identifier collision'}}
   $previous=if($state-and$state.current-ne$relative){$state.current}elseif($state){$state.previous}else{$null}
   Activate $relative $previous
  }
  Rollback {
   if(-not$state.previous){throw 'There is no previous preview version'}
   $null=Verify-Payload (Join-Path $Root $state.previous.Replace('/','\'));Activate $state.previous $state.current
  }
  Uninstall {
   if(-not$state){throw 'No preview installation'}
   $versions=@(Get-ChildItem -LiteralPath (Join-Path $Root 'versions') -Directory)
   foreach($version in $versions){if($version.Name-notmatch'^[a-f0-9]{16}$'){throw 'Unrecognized version directory'};$null=Verify-Payload $version.FullName}
   $shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) 'Koett Preview.lnk';$shell=New-Object -ComObject WScript.Shell
   if(Test-Path $shortcut){if(Under-Root ($shell.CreateShortcut($shortcut).TargetPath)){Remove-Item -LiteralPath $shortcut}}
   $run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run';$value=(Get-ItemProperty -LiteralPath $run -Name Koett -ErrorAction SilentlyContinue).Koett
   if(Own-Startup $value){Remove-ItemProperty -LiteralPath $run -Name Koett}
   foreach($version in $versions){Remove-Item -LiteralPath $version.FullName -Recurse -Force}
   Remove-Item -LiteralPath (Join-Path $Root 'current.json'),(Join-Path $Root 'preview-owner.json')
   foreach($path in @((Join-Path $Root 'versions'),$Root)){if(@(Get-ChildItem -LiteralPath $path -Force).Count-eq0){Remove-Item -LiteralPath $path}}
  }
 }
 @{ok=$true;action=$Action.ToLowerInvariant();root=$Root;data_preserved=$true;signature='unsigned-preview';outputs=@();warnings=@('Unsigned preview; media tools and optional formatter are not bundled')}|ConvertTo-Json -Depth 6 -Compress
}catch{
 @{ok=$false;code='preview_management_failed';error=$_.Exception.Message;fix='Close Koett, verify the extracted payload, and retry; application data is preserved';outputs=@();warnings=@()}|ConvertTo-Json -Compress;exit 1
}
