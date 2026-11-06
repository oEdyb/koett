$ErrorActionPreference='Stop'
$failures=@()
foreach($file in Get-ChildItem (Join-Path $PSScriptRoot 'windows-lab') -Filter *.ps1 -Recurse){
 $tokens=$null;$parseErrors=$null
 $null=[System.Management.Automation.Language.Parser]::ParseFile($file.FullName,[ref]$tokens,[ref]$parseErrors)
 foreach($problem in $parseErrors){$failures+="$($file.Name):$($problem.Extent.StartLineNumber): $($problem.Message)"}
}
if($failures.Count){throw ($failures-join"`n")}
Write-Output 'Windows lab PowerShell syntax passed'
