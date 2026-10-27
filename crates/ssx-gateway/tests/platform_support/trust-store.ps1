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
    $installed = Import-Certificate -FilePath (Join-Path $CaseRoot 'ca.der') -CertStoreLocation 'Cert:\CurrentUser\Root'
    if ($installed.Thumbprint -ne $thumbprint) { throw 'Wrong certificate installed' }
} elseif (Test-Path -LiteralPath $marker) {
    if ((Get-Content -LiteralPath $marker).Trim() -ne $thumbprint) { throw 'Cleanup ownership mismatch' }
    if (Test-Path $storePath) { Remove-Item -LiteralPath $storePath -Force }
    if (Test-Path $storePath) { throw 'Certificate remains trusted' }
    Remove-Item -LiteralPath $marker
}
$certificate.Dispose()
