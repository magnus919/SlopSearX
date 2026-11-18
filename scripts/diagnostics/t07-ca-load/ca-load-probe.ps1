param([Parameter(Mandatory = $true)][string]$CaseRoot)
$ErrorActionPreference = 'Stop'
[Console]::Error.WriteLine('SSX_TRUST_STAGE script-entry')
[Console]::Error.WriteLine('SSX_TRUST_STAGE before-ca-load')
$caPath = Join-Path $CaseRoot 'ca.der'
[Console]::Error.WriteLine('SSX_TRUST_STAGE after-ca-path')
[Console]::Error.WriteLine('SSX_TRUST_STAGE before-ca-constructor')
$certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($caPath)
[Console]::Error.WriteLine('SSX_TRUST_STAGE after-ca-load')
$certificate.Dispose()
