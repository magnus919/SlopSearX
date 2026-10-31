param([ValidateSet('install','remove')][string]$Mode, [string]$CaseRoot)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:SLOPSEARX_PLATFORM_TRUST_FIXTURE -ne '1') { throw 'Dedicated ephemeral CI fixture only' }
$certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new((Join-Path $CaseRoot 'ca.der'))
$thumbprint = $certificate.Thumbprint
if ($thumbprint -notmatch '^[A-Fa-f0-9]{40}$') { throw 'Invalid owned fingerprint' }
$marker = Join-Path $CaseRoot 'trust-active'
$storePath = "Cert:\CurrentUser\Root\$thumbprint"
if ($Mode -eq 'install') {
    if (Test-Path $storePath) { throw 'Fixture must not adopt an existing CA' }
    Set-Content -LiteralPath $marker -Value $thumbprint
    # Same CurrentUser Root scope; headless acceptance remains a hosted witness.
    $certutil = Join-Path $env:SystemRoot 'System32\certutil.exe'
    & $certutil '-user' '-addstore' 'Root' (Join-Path $CaseRoot 'ca.der')
    $importExit = $LASTEXITCODE
    if ($importExit -ne 0) { throw 'Owned CA import failed' }
    $installed = Get-Item -LiteralPath $storePath
    if ($installed.Thumbprint -ne $thumbprint) { throw 'Wrong certificate installed' }
    $expectedDer = $certificate.RawData
    $installedDer = $installed.RawData
    if ($installedDer.Length -ne $expectedDer.Length) { throw 'Wrong certificate bytes installed' }
    for ($index = 0; $index -lt $expectedDer.Length; $index++) {
        if ($installedDer[$index] -ne $expectedDer[$index]) { throw 'Wrong certificate bytes installed' }
    }
} elseif (Test-Path -LiteralPath $marker) {
    if ((Get-Content -LiteralPath $marker).Trim() -ne $thumbprint) { throw 'Cleanup ownership mismatch' }
    if (Test-Path $storePath) { Remove-Item -LiteralPath $storePath -Force }
    if (Test-Path $storePath) { throw 'Certificate remains trusted' }
    Remove-Item -LiteralPath $marker
}
$certificate.Dispose()
