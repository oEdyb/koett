if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$ErrorActionPreference='Stop';$null=Preview-Quit;$stateFile=$global:PreviewRoot+'\current.json';$stateHash=(Get-FileHash $stateFile).Hash
$unsafe=$global:PreviewDir+'\unsafe-payload';$payload=$global:PreviewDir+'\current\Koett-Windows-x86_64';Copy-Item -LiteralPath $payload -Destination $unsafe -Recurse
$release=Get-Content ($unsafe+'\preview-release.json') -Raw -Encoding UTF8|ConvertFrom-Json;$release.files|Add-Member -NotePropertyName 'CON.txt' -NotePropertyValue ('0'*64);$release|ConvertTo-Json -Depth 8|Set-Content ($unsafe+'\preview-release.json') -Encoding UTF8
$bad=Preview-Run 'Install' $unsafe;$unsafeGuard=($bad.exit-ne0-and$bad.json.error-eq'Unsafe payload path'-and$stateHash-eq(Get-FileHash $stateFile).Hash)
$versions=$global:PreviewRoot+'\versions';$outside=$global:PreviewDir+'\versions-junction-fixture';Move-Item -LiteralPath $versions -Destination $outside;$junctionGuard=$false
try{
 New-Item -ItemType Junction -Path $versions -Target $outside|Out-Null
 $beforeFiles=@(Get-ChildItem -LiteralPath $outside -Recurse -File).Count;$blocked=Preview-Run 'Uninstall' '';$afterFiles=@(Get-ChildItem -LiteralPath $outside -Recurse -File).Count
 $junctionGuard=($blocked.exit-ne0-and$blocked.json.error-match'reparse point'-and$beforeFiles-eq$afterFiles-and$beforeFiles-gt0-and$stateHash-eq(Get-FileHash $stateFile).Hash)
}finally{
 if(Test-Path -LiteralPath $versions){if((Get-Item -LiteralPath $versions -Force).Attributes-band[IO.FileAttributes]::ReparsePoint){[IO.Directory]::Delete($versions)}else{throw 'Refusing to replace an unexpected versions directory'}}
 Move-Item -LiteralPath $outside -Destination $versions
}
$before=Preview-Data;$uninstall=Preview-Run 'Uninstall' '';if($uninstall.exit-ne0-or-not$uninstall.json.ok){throw $uninstall.stdout};$after=Preview-Data;$preserved=($before.Count-eq$after.Count-and@($before.Keys|Where-Object {$before[$_]-cne$after[$_]}).Count-eq0)
$run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run';$startupGone=(-not(Get-ItemProperty $run -Name Koett -ErrorAction SilentlyContinue).Koett);$rootGone=(-not(Test-Path $global:PreviewRoot));$shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) 'Koett Preview.lnk';$shortcutGone=(-not(Test-Path $shortcut))
$installOff=Preview-Run 'Install' $payload;if($installOff.exit-ne0-or-not$installOff.json.ok){throw $installOff.stdout};$off=(-not(Get-ItemProperty $run -Name Koett -ErrorAction SilentlyContinue).Koett);$again=Preview-Run 'Uninstall' '';if($again.exit-ne0-or-not$again.json.ok){throw $again.stdout}
$afterAgain=Preview-Data;$keptAgain=($afterAgain.Count-eq$before.Count-and@($before.Keys|Where-Object {$before[$_]-cne$afterAgain[$_]}).Count-eq0)
$old=Get-Content ($global:PreviewDir+'\test-state.json') -Raw -Encoding UTF8|ConvertFrom-Json;if($old.old_run){Set-ItemProperty $run -Name Koett -Value $old.old_run}
@{passed=($unsafeGuard-and$junctionGuard-and$preserved-and$startupGone-and$rootGone-and$shortcutGone-and$off-and$keptAgain-and-not(Test-Path $global:PreviewRoot));unsafe_path_guard=$unsafeGuard;junction_guard=$junctionGuard;unsafe=$bad;junction=$blocked;outside_files_before=$beforeFiles;outside_files_after=$afterFiles;uninstall=$uninstall;startup_entry_removed=$startupGone;root_removed=$rootGone;shortcut_removed=$shortcutGone;fresh_install_startup_off=$off;reinstall=$installOff;second_uninstall=$again;profile_preserved=$preserved;profile_preserved_again=$keptAgain;profile_before=$before;profile_after=$afterAgain;startup_restored=$old.old_run;focus=Get-LabFocus}
