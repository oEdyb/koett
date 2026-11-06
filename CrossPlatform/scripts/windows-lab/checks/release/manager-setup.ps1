if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
if(Test-Path $global:PreviewRoot){$existing=Get-Content ($global:PreviewRoot+'\current.json') -Raw -Encoding UTF8|ConvertFrom-Json;if($existing.current){throw 'Private preview is already active'}}
New-Item -ItemType Directory -Force $global:PreviewDir|Out-Null
$web=New-Object Net.WebClient;try{foreach($case in @(@('m6-package-atomic','current'),@('m6-package-previous','previous'))){$zip=$global:PreviewDir+'\'+$case[1]+'.zip';$web.DownloadFile(('http://10.0.2.2:18767/'+$case[0]+'/Koett-Windows-x86_64.zip'),$zip);Expand-Archive -LiteralPath $zip -DestinationPath ($global:PreviewDir+'\'+$case[1]) -Force}}finally{$web.Dispose()}
@'
param([string]$RequestBase64)
[Console]::OutputEncoding=New-Object Text.UTF8Encoding($false)
$r=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($RequestBase64))|ConvertFrom-Json
& $r.script -Action $r.action -Payload $r.payload -Root $r.root
exit $LASTEXITCODE
'@|Set-Content -LiteralPath ($global:PreviewDir+'\manager-wrapper.ps1') -Encoding UTF8
$r=Preview-Run 'Status' ''; $hashes=Preview-Data;if($hashes.Count-lt3){throw 'Profile hash fixture is incomplete'}
@{passed=($r.exit-eq0-and$r.json.ok-and-not$r.json.installed);status=$r;hashes=$hashes;artifact=if(@(Get-Process Koett -ErrorAction SilentlyContinue).Count){Get-LabArtifact}else{$null}}
