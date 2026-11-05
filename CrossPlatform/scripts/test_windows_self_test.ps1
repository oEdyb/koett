$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Join-Path $env:LOCALAPPDATA ('koett-ci-retry-' + [Guid]::NewGuid())
New-Item -ItemType Directory $root | Out-Null
try {
 foreach ($case in @(
  @{name='temporary';message='error: could not download the Koett voice detector: http status: 504';recover=$true;expected=2;code=0},
  @{name='permanent-http';message='error: could not download the Koett voice detector: http status: 404';recover=$false;expected=1;code=1},
  @{name='checksum';message='error: model checksum mismatch';recover=$false;expected=1;code=1},
  @{name='decode';message='error: unexpected transcript';recover=$false;expected=1;code=1},
  @{name='exhausted';message='error: could not download the Koett speech model: http status: 503';recover=$false;expected=3;code=1}
 )) {
  $count = Join-Path $root ($case.name+'.count')
  $exe = Join-Path $root ($case.name+'.cmd')
  $recover = if ($case.recover) { "if exist `"$count`" (echo attempt>>`"$count`" & exit /b 0)" } else { '' }
  [IO.File]::WriteAllText($exe,"@echo off`r`n$recover`r`necho attempt>>`"$count`"`r`necho $($case.message)`r`nexit /b 1`r`n")
  & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'run_windows_self_test.ps1') -Executable $exe -RetryDelaySeconds 0
  $code=$LASTEXITCODE
  $attempts=@(Get-Content $count).Count
  if($code -ne $case.code -or $attempts -ne $case.expected){throw "$($case.name): exit=$code attempts=$attempts"}
 }
 Write-Output 'Windows self-test retry policy passed (5 cases)'
} finally { Remove-Item $root -Recurse -Force }
