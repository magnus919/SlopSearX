param([ValidateSet('install','remove')][string]$Mode, [string]$CaseRoot)
[Console]::Error.WriteLine('SSX_TRUST_STAGE script-entry')
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:SLOPSEARX_PLATFORM_TRUST_FIXTURE -ne '1') { throw 'Dedicated ephemeral CI fixture only' }
[Console]::Error.WriteLine('SSX_TRUST_STAGE before-ca-load')
$certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new((Join-Path $CaseRoot 'ca.der'))
[Console]::Error.WriteLine('SSX_TRUST_STAGE after-ca-load')
$thumbprint = $certificate.Thumbprint
if ($thumbprint -notmatch '^[A-Fa-f0-9]{40}$') { throw 'Invalid owned fingerprint' }
$marker = Join-Path $CaseRoot 'trust-active'
$storePath = "Cert:\CurrentUser\Root\$thumbprint"
if ($Mode -eq 'install') {
    [Console]::Error.WriteLine('SSX_TRUST_STAGE before-current-user-store-probe')
    $alreadyPresent = Test-Path $storePath
    [Console]::Error.WriteLine('SSX_TRUST_STAGE after-current-user-store-probe')
    if ($alreadyPresent) { throw 'Fixture must not adopt an existing CA' }
    [Console]::Error.WriteLine('SSX_TRUST_STAGE before-marker')
    Set-Content -LiteralPath $marker -Value $thumbprint
    # Same CurrentUser Root scope; headless acceptance remains a hosted witness.
    $certutil = Join-Path $env:SystemRoot 'System32\certutil.exe'
    [Console]::Error.WriteLine('SSX_TRUST_STAGE before-certutil')
    & $certutil '-user' '-addstore' 'Root' (Join-Path $CaseRoot 'ca.der')
    $importExit = $LASTEXITCODE
    [Console]::Error.WriteLine('SSX_TRUST_STAGE after-certutil')
    if ($importExit -ne 0) { throw 'Owned CA import failed' }
    [Console]::Error.WriteLine('SSX_TRUST_STAGE before-installed-verification')
    $installed = Get-Item -LiteralPath $storePath
    if ($installed.Thumbprint -ne $thumbprint) { throw 'Wrong certificate installed' }
    $expectedDer = $certificate.RawData
    $installedDer = $installed.RawData
    if ($installedDer.Length -ne $expectedDer.Length) { throw 'Wrong certificate bytes installed' }
    for ($index = 0; $index -lt $expectedDer.Length; $index++) {
        if ($installedDer[$index] -ne $expectedDer[$index]) { throw 'Wrong certificate bytes installed' }
    }
    [Console]::Error.WriteLine('SSX_TRUST_STAGE after-installed-verification')
} elseif (Test-Path -LiteralPath $marker) {
    [Console]::Error.WriteLine('SSX_TRUST_STAGE before-removal')
    if ((Get-Content -LiteralPath $marker).Trim() -ne $thumbprint) { throw 'Cleanup ownership mismatch' }
    if (Test-Path $storePath) { Remove-Item -LiteralPath $storePath -Force }
    [Console]::Error.WriteLine('SSX_TRUST_STAGE before-removal-verification')
    if (Test-Path $storePath) { throw 'Certificate remains trusted' }
    [Console]::Error.WriteLine('SSX_TRUST_STAGE after-removal-verification')
    Remove-Item -LiteralPath $marker
    [Console]::Error.WriteLine('SSX_TRUST_STAGE after-removal')
}
$certificate.Dispose()
