[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\cppm\bin'),
    [switch]$NoPath,
    [switch]$WithDependencies
)

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$releaseBase = 'https://github.com/GaussNoob/cppm/releases'
$architecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$arch = switch ($architecture) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'arm64' }
    default { throw 'Unsupported architecture; expected Windows x64 or ARM64.' }
}
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('cppm-download-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    if (-not $Version) {
        $versionPath = Join-Path $temporary 'VERSION'
        Invoke-WebRequest -UseBasicParsing -Uri "$releaseBase/latest/download/VERSION" -OutFile $versionPath -TimeoutSec 15
        $Version = (Get-Content -LiteralPath $versionPath -Raw).Trim()
    }
    $Version = $Version -creplace '^v', ''
    if ($Version -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z') { throw 'Invalid release version; expected X.Y.Z.' }
    $stem = "cppm-$Version-windows-$arch"
    $archive = "$stem.zip"
    $archivePath = Join-Path $temporary $archive
    Write-Host "Downloading cppm $Version for Windows $arch..."
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseBase/download/v$Version/$archive" -OutFile $archivePath -TimeoutSec 180
    $checksumPath = Join-Path $temporary "$archive.sha256"
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseBase/download/v$Version/$archive.sha256" -OutFile $checksumPath -TimeoutSec 15
    $checksum = Get-Content -LiteralPath $checksumPath -Raw
    $expected = ($checksum.Trim() -split '\s+')[0]
    if ($expected -notmatch '^[0-9a-fA-F]{64}$') { throw 'Invalid SHA-256 file.' }
    if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash -ine $expected) { throw 'SHA-256 mismatch; installation cancelled.' }
    Expand-Archive -LiteralPath $archivePath -DestinationPath $temporary
    $package = Join-Path $temporary $stem
    $binary = Join-Path $package 'cppm.exe'
    $reportedVersion = & $binary --version
    if ($LASTEXITCODE -ne 0 -or $reportedVersion -cne "cppm $Version") { throw 'Release binary version mismatch.' }
    & (Join-Path $package 'install.ps1') -BinaryPath $binary -InstallDir $InstallDir -NoPath:$NoPath -WithDependencies:$WithDependencies
    Write-Host 'Use cppm update to install future releases.'
}
finally {
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
