if($env:USERNAME-ne'KoettLab'-or$env:COMPUTERNAME-ne'KOETT-LAB'){throw 'These checks run only in the isolated Koett Windows lab guest'}
$web=New-Object Net.WebClient;try{$zip=$global:PreviewDir+'\reviewed.zip';$web.DownloadFile('http://10.0.2.2:18767/m6-package-reviewed/Koett-Windows-x86_64.zip',$zip);Expand-Archive -LiteralPath $zip -DestinationPath ($global:PreviewDir+'\current') -Force}finally{$web.Dispose()}
$tokens=$null;$errors=$null;$null=[Management.Automation.Language.Parser]::ParseFile(($global:PreviewDir+'\current\Koett-Windows-x86_64\manage-preview.ps1'),[ref]$tokens,[ref]$errors);if($errors.Count){throw ($errors|Out-String)}
$r=Preview-Run 'Status' '';@{passed=($r.exit-eq0-and$r.json.ok-and$r.json.installed);status=$r;profile=Preview-Data}
