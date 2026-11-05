param(
 [string]$Executable = './target/release/koett-engine.exe',
 [int]$RetryDelaySeconds = 2
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
for ($attempt = 1; $attempt -le 3; $attempt++) {
 $output = @(& $Executable --self-test 2>&1)
 $result = $LASTEXITCODE
 $output | ForEach-Object { Write-Host $_ }
 if ($result -eq 0) { exit 0 }
 # Only temporary HTTP download failures are retryable. Keep model integrity,
 # decoding, native crashes and all other self-test failures fatal.
 $temporaryDownload = ($output -join "`n") -match 'error: could not download the Koett (speech model|voice detector): http status: (429|502|503|504)(\r?\n|$)'
 if (!$temporaryDownload -or $attempt -eq 3) { exit $result }
 Write-Host "Temporary model download failure; retry $attempt of 2"
 Start-Sleep -Seconds $RetryDelaySeconds
}
