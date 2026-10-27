param([string]$BuildJson, [string]$OutputRoot, [string]$Manifest, [string]$Target)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $OutputRoot) { throw 'Refuse to overwrite shipping artifact custody root' }
$bin = Join-Path $OutputRoot 'bin'
New-Item -ItemType Directory -Path $bin | Out-Null
$rows = Get-Content -LiteralPath $BuildJson | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.executable -and -not $_.profile.test }
$artifacts = [ordered]@{}
foreach ($role in @('ssx-gateway','ssx-gateway-dns','ssx-gateway-input')) {
    $match = @($rows | Where-Object { $_.target.name -eq $role })
    if ($match.Count -ne 1) { throw "Expected one actual production Cargo artifact: $role" }
    $source = Get-Item -LiteralPath $match[0].executable
    if ($source.PSIsContainer -or ($source.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) { throw 'Invalid emitted executable type' }
    $sourceHash = (Get-FileHash -LiteralPath $source.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $destination = Join-Path $bin $source.Name
    Copy-Item -LiteralPath $source.FullName -Destination $destination
    $copy = Get-Item -LiteralPath $destination
    $copyHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($copyHash -ne $sourceHash -or $copy.Length -ne $source.Length) { throw 'Production artifact copy changed bytes' }
    $artifacts[$role] = [ordered]@{ path = $copy.FullName; bytes = $copy.Length; sha256 = $copyHash }
}
$osName = if ($IsWindows) { 'windows' } elseif ($IsMacOS) { 'macos' } elseif ($IsLinux) { 'linux' } else { throw 'Unsupported native platform' }
$archName = if ($Target -match '^x86_64-') { 'x86_64' } elseif ($Target -match '^aarch64-') { 'aarch64' } else { throw 'Unsupported native architecture' }
$version = (& ($artifacts['ssx-gateway'].path) --version)
if ($LASTEXITCODE -ne 0 -or $version -notmatch '^ssx-gateway (.+)$') { throw 'Preserved production image cannot invoke version' }
$record = [ordered]@{ schema = 1; head = (& git rev-parse HEAD).Trim(); target = $Target; os = $osName; arch = $archName; version = $Matches[1]; features = 'production-default-client-only'; artifacts = $artifacts }
$record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Manifest -Encoding utf8NoBOM
Get-FileHash -LiteralPath $Manifest -Algorithm SHA256
