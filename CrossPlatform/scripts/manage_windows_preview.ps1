[CmdletBinding(SupportsShouldProcess)]
param(
 [ValidateSet('Install','Rollback','Uninstall','Status')][string]$Action='Status',
 [string]$Payload,
 [string]$MessagePath,
 [ValidateSet('Koett Preview','Koett')][string]$ShortcutName='Koett Preview',
 [string]$Root=(Join-Path $env:LOCALAPPDATA 'Programs\Koett-Preview')
)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
function Read-Json([string]$Path){Get-Content -LiteralPath $Path -Raw -Encoding UTF8|ConvertFrom-Json}
function Hash([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()}
function Under-Root([string]$Path){[IO.Path]::GetFullPath($Path).StartsWith($Root+'\',[StringComparison]::OrdinalIgnoreCase)}
function Payload-Items([string]$Directory){
 $queue=New-Object 'Collections.Generic.Queue[string]';$queue.Enqueue($Directory)
 while($queue.Count){foreach($item in @(Get-ChildItem -LiteralPath $queue.Dequeue() -Force)){
  if($item.Attributes-band[IO.FileAttributes]::ReparsePoint){throw ('Link in payload at '+$item.FullName+'. Move the link outside the installation, then retry.')}
  $item
  if($item.PSIsContainer){$queue.Enqueue($item.FullName)}
 }}
}
function Verify-Payload([string]$Directory){
 if((Get-Item -LiteralPath $Directory -Force).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Payload root is a reparse point'}
 $all=@(Payload-Items $Directory)
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
 $actual=@($all|Where-Object {-not$_.PSIsContainer}|ForEach-Object {$_.FullName.Substring($Directory.Length+1).Replace('\','/')})
 if(Compare-Object $expected $actual){throw 'Payload contains missing or unlisted files'}
 foreach($name in @('Koett.exe','koett-engine.exe','koett-transcribe-engine.exe','manage-preview.ps1','LICENSE','THIRD_PARTY_NOTICES.md')){if(-not(Test-Path -LiteralPath (Join-Path $Directory $name) -PathType Leaf)){throw ('Missing payload file: '+$name)}}
 $release
}
function Read-State {
 foreach($name in @('preview-owner.json','current.json','versions')){
  $path=Join-Path $Root $name
  if((Test-Path -LiteralPath $path)-and((Get-Item -LiteralPath $path -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)){throw 'Installation metadata or versions contain a reparse point'}
 }
 $owner=Read-Json (Join-Path $Root 'preview-owner.json')
 if($owner.format-ne1-or-not[String]::Equals($owner.root,$Root,[StringComparison]::OrdinalIgnoreCase)){throw 'This directory is not an owned Koett preview installation'}
 $state=Read-Json (Join-Path $Root 'current.json')
 foreach($version in @($state.current,$state.previous)|Where-Object {$_}){if($version-notmatch'^versions/[a-f0-9]{16}$'){throw 'Unsafe installed version reference'}}
 $state
}
function Own-Startup([string]$Value){$Value-eq('"'+$Root+'\Koett.exe"')-or($Value-and$Value.StartsWith('"'+$Root+'\versions\',[StringComparison]::OrdinalIgnoreCase)-and$Value.EndsWith('\Koett.exe"',[StringComparison]::OrdinalIgnoreCase))}
function Activate([string]$Current,[string]$Previous){
 $statePath=Join-Path $Root 'current.json';$oldState=if(Test-Path $statePath){[IO.File]::ReadAllBytes($statePath)}else{$null}
 $run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run';$oldRun=(Get-ItemProperty -LiteralPath $run -Name Koett -ErrorAction SilentlyContinue).Koett
 $shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) ($ShortcutName+'.lnk');$shell=New-Object -ComObject WScript.Shell
 if(Test-Path $shortcut){$target=$shell.CreateShortcut($shortcut).TargetPath;if(-not(Under-Root $target)){throw 'The Koett Preview shortcut belongs to another installation'}}
 $oldLink=if(Test-Path $shortcut){[IO.File]::ReadAllBytes($shortcut)}else{$null}
 $target=if($ShortcutName-eq'Koett'){Join-Path $Root 'Koett.exe'}else{Join-Path (Join-Path $Root $Current.Replace('/','\')) 'Koett.exe'}
 $launch=Join-Path $Root 'launch.ini';$oldLaunch=if(Test-Path $launch){[IO.File]::ReadAllBytes($launch)}else{$null}
 $temp=$statePath+'.'+[Guid]::NewGuid().ToString('N');$launchTemp=$temp+'.launch'
 try {
  if($ShortcutName-eq'Koett'){[IO.File]::WriteAllText($launchTemp,('[Koett]'+[Environment]::NewLine+'Current='+$Current.Replace('/','\')+[Environment]::NewLine));if(Test-Path $launch){[IO.File]::Replace($launchTemp,$launch,[System.Management.Automation.Language.NullString]::Value)}else{[IO.File]::Move($launchTemp,$launch)}}
  $link=$shell.CreateShortcut($shortcut);$link.TargetPath=$target;$link.WorkingDirectory=[IO.Path]::GetDirectoryName($target);$link.Save()
  if(Own-Startup $oldRun){Set-ItemProperty -LiteralPath $run -Name Koett -Value ('"'+$target+'"')}
  $state=@{format=1;current=$Current;previous=$Previous};[IO.File]::WriteAllText($temp,($state|ConvertTo-Json -Compress))
  if(Test-Path $statePath){[IO.File]::Replace($temp,$statePath,[System.Management.Automation.Language.NullString]::Value)}else{[IO.File]::Move($temp,$statePath)}
 }catch{
  if($oldLaunch){[IO.File]::WriteAllBytes($launch,$oldLaunch)}elseif(Test-Path $launch){Remove-Item -LiteralPath $launch}
  if($oldLink){[IO.File]::WriteAllBytes($shortcut,$oldLink)}elseif(Test-Path $shortcut){Remove-Item -LiteralPath $shortcut}
  if(Own-Startup $oldRun){Set-ItemProperty -LiteralPath $run -Name Koett -Value $oldRun}
  if($oldState){[IO.File]::WriteAllBytes($statePath,$oldState)}elseif(Test-Path $statePath){Remove-Item -LiteralPath $statePath}
  throw
 }finally{foreach($path in @($temp,$launchTemp)){if(Test-Path $path){Remove-Item -LiteralPath $path}}}
}
function Prune-Versions([string]$Current,[string]$Previous){
 foreach($item in @(Get-ChildItem -LiteralPath (Join-Path $Root 'versions') -Directory)){
  if($item.Name-notmatch'^[a-f0-9]{16}$'-or('versions/'+$item.Name)-in@($Current,$Previous)){continue}
  try{$null=Verify-Payload $item.FullName;Remove-Item -LiteralPath $item.FullName -Recurse -Force}catch{'An older preview was preserved because its files changed or are busy'}
 }
}
$cleanupWarnings=@()
$preserved=@()
if($MessagePath){[IO.File]::WriteAllText($MessagePath,'',[Text.Encoding]::Unicode)}
function Message([string]$Text){if($MessagePath){[IO.File]::WriteAllText($MessagePath,$Text,[Text.Encoding]::Unicode)}}
function Preserve([string]$Path){
 $destination=Join-Path (Join-Path $env:LOCALAPPDATA 'Koett Preserved Files') ([Guid]::NewGuid().ToString('N'))
 try{New-Item -ItemType Directory -Path $destination -Force|Out-Null;$target=Join-Path $destination ([IO.Path]::GetFileName($Path));if([IO.Directory]::Exists($Path)){[IO.Directory]::Move($Path,$target)}else{[IO.File]::Move($Path,$target)}}catch{throw ('Could not keep '+$Path+'. Close programs using that path, then retry uninstall. '+$_.Exception.Message)}
 $script:preserved+=Join-Path $destination ([IO.Path]::GetFileName($Path))
 [IO.File]::WriteAllText((Join-Path $destination 'preserved-from.txt'),$Path)
 Message ('Added or changed app files were kept in:'+([Environment]::NewLine)+(Join-Path $env:LOCALAPPDATA 'Koett Preserved Files'))
}
function Residue-State {
 if(Test-Path (Join-Path $Root 'current.json')){throw ('Installation metadata is incomplete at '+$Root+'. Move this installation folder to a safe location, then run setup again; application data is stored separately.')}
 foreach($item in @(Get-ChildItem -LiteralPath $Root -Force)){
  $allowed=$item.Name-in@('Koett.ico','Koett.exe','Uninstall.exe','manage-install.ps1','launch.ini','preview-owner.json')-or$item.Name-match'^(stage|removing)-[a-f0-9]{16,32}$'
  if($item.Name-eq'versions'-and$item.PSIsContainer){$allowed=@(Get-ChildItem -LiteralPath $item.FullName -Force).Count-eq0}
  if(-not$allowed-or($item.Attributes-band[IO.FileAttributes]::ReparsePoint)){throw ('Unrecognized path: '+$item.FullName+'. Move that path outside '+$Root+', then retry; it has not been deleted.')}
 }
 $null
}
function Sweep-Residue {
 foreach($item in @(Get-ChildItem -LiteralPath $Root -Directory -Force|Where-Object {$_.Name-match'^(stage|removing)-[a-f0-9]{16,32}$'})){
  if($item.Attributes-band[IO.FileAttributes]::ReparsePoint){throw ('Link at '+$item.FullName+'. Move the link outside the installation, then retry.')}
  Preserve $item.FullName
 }
}
function Remove-Versions {
 $directory=Join-Path $Root 'versions';if(-not(Test-Path $directory)){return}
 $pending=@()
 foreach($item in @(Get-ChildItem -LiteralPath $directory -Force)){
  if($item.Attributes-band[IO.FileAttributes]::ReparsePoint){throw ('Link at '+$item.FullName+'. Move the link outside the installation, then retry.')}
  $verified=$false
  if($item.PSIsContainer-and$item.Name-match'^[a-f0-9]{16}$'){try{$null=Verify-Payload $item.FullName;$verified=$true}catch{}}
  $quarantine=Join-Path $Root ('removing-'+[Guid]::NewGuid().ToString('N'))
  try{if($item.PSIsContainer){[IO.Directory]::Move($item.FullName,$quarantine)}else{[IO.File]::Move($item.FullName,$quarantine)}}catch{throw ('Cannot prepare '+$item.FullName+' for removal. Close programs using that path, then retry. '+$_.Exception.Message)}
  $pending+=@{path=$quarantine;verified=$verified}
 }
 foreach($item in $pending){
  if(-not$item.verified){Preserve $item.path;continue}
  try{$null=Verify-Payload $item.path}catch{Preserve $item.path;continue}
  try{Remove-Item -LiteralPath $item.path -Recurse -Force -ErrorAction Stop}catch{throw ('Removal stopped at '+$item.path+'. Close programs using that path and run Uninstall again; the retry keeps any remaining files. '+$_.Exception.Message)}
 }
}


try {
 $Root=[IO.Path]::GetFullPath($Root).TrimEnd('\')
 $ancestor=$Root;while($ancestor){if((Test-Path -LiteralPath $ancestor)-and((Get-Item -LiteralPath $ancestor -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)){throw 'Preview install path contains a reparse point'};$ancestor=[IO.Path]::GetDirectoryName($ancestor)}
 if(-not$Root.StartsWith([IO.Path]::GetFullPath($env:LOCALAPPDATA)+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Preview root must be below LocalAppData'}
 if($Root-eq[IO.Path]::GetFullPath($env:LOCALAPPDATA)){throw 'Unsafe preview root'}
 if(Test-Path $Root){if((Get-Item -LiteralPath $Root -Force).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Preview root is a reparse point'};if((Test-Path (Join-Path $Root 'current.json'))-and(Test-Path (Join-Path $Root 'preview-owner.json'))){$state=Read-State}else{$state=Residue-State}}else{$state=$null}
 if($Action-eq'Status'){@{ok=$true;installed=[bool]($state-and$state.current);root=$Root;state=$state;signature='unsigned-preview';outputs=@();warnings=@()}|ConvertTo-Json -Depth 6 -Compress;exit 0}
 if(@(Get-Process Koett,koett-engine,koett-transcribe-engine -ErrorAction SilentlyContinue|Where-Object {$_.SessionId-eq[Diagnostics.Process]::GetCurrentProcess().SessionId}).Count){throw ('Close Koett and its transcription commands in this Windows session, then retry '+$Root)}
 if(-not$PSCmdlet.ShouldProcess($Root,$Action)){@{ok=$true;changed=$false;outputs=@();warnings=@()}|ConvertTo-Json -Compress;exit 0}
 if(Test-Path $Root){Sweep-Residue}
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
   $repair=$false
   if(Test-Path $version){try{$null=Verify-Payload $version;if((Hash (Join-Path $version 'preview-release.json'))-cne(Hash (Join-Path $Payload 'preview-release.json'))){throw 'Installed version identifier collision'}}catch{$repair=$true}}
   if(-not(Test-Path $version)-or$repair){
    $stage=Join-Path $Root ('stage-'+[Guid]::NewGuid().ToString('N'))
    try{
     New-Item -ItemType Directory -Path $stage|Out-Null
     Get-ChildItem -LiteralPath $Payload -Force|ForEach-Object {Copy-Item -LiteralPath $_.FullName -Destination $stage -Recurse -Force}
     $null=Verify-Payload $stage
     if($repair){Preserve $version}
     [IO.Directory]::Move($stage,$version)
    }finally{if(Test-Path $stage){Remove-Item -LiteralPath $stage -Recurse -Force}}
   }
   $previous=if($state-and$state.current-ne$relative){$state.current}elseif($state){$state.previous}else{$null}
   Activate $relative $previous
   $cleanupWarnings=@(Prune-Versions $relative $previous)
  }
  Rollback {
   if(-not$state.previous){throw 'There is no previous preview version'}
   $null=Verify-Payload (Join-Path $Root $state.previous.Replace('/','\'));Activate $state.previous $state.current
  }
  Uninstall {
   if(-not(Test-Path $Root)){throw ('No installation at '+$Root)}
   foreach($item in @(Get-ChildItem -LiteralPath $Root -Force)){if($item.Name-notin@('versions','current.json','preview-owner.json','Koett.ico','Koett.exe','Uninstall.exe','manage-install.ps1','launch.ini')){Preserve $item.FullName}}
   Remove-Versions
   $shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) ($ShortcutName+'.lnk');$shell=New-Object -ComObject WScript.Shell
   if(Test-Path $shortcut){if(Under-Root ($shell.CreateShortcut($shortcut).TargetPath)){Remove-Item -LiteralPath $shortcut}}
   $run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run';$value=(Get-ItemProperty -LiteralPath $run -Name Koett -ErrorAction SilentlyContinue).Koett
   if(Own-Startup $value){Remove-ItemProperty -LiteralPath $run -Name Koett}
   foreach($name in @('current.json','preview-owner.json','launch.ini')){$path=Join-Path $Root $name;if(Test-Path $path){Remove-Item -LiteralPath $path}}
   foreach($path in @((Join-Path $Root 'versions'),$Root)){if((Test-Path $path)-and@(Get-ChildItem -LiteralPath $path -Force).Count-eq0){Remove-Item -LiteralPath $path}}
  }
 }
 @{ok=$true;action=$Action.ToLowerInvariant();root=$Root;data_preserved=$true;signature='unsigned-preview';outputs=@();warnings=(@('Unsigned preview; media tools and optional formatter are not bundled')+$cleanupWarnings);preserved_files=$preserved}|ConvertTo-Json -Depth 6 -Compress
}catch{
 Message ($_.Exception.Message+([Environment]::NewLine)+'Your application data is kept. '+($preserved-join[Environment]::NewLine))
 @{ok=$false;code='preview_management_failed';error=$_.Exception.Message;fix='Follow the path-specific error above and retry; application data is preserved';outputs=@();warnings=@()}|ConvertTo-Json -Compress;exit 1
}
