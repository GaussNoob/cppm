[CmdletBinding()]
param(
    [string]$BinaryPath,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\cppm\bin'),
    [switch]$NoPath
)

$ErrorActionPreference = 'Stop'
if (-not $BinaryPath) {
    $BinaryPath = Join-Path $PSScriptRoot 'cppm.exe'
    if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
        $BinaryPath = Join-Path $PSScriptRoot '..\target\release\cppm.exe'
    }
}
if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
    throw 'cppm.exe not found. Extract the Windows package first, or build with cargo build --release --locked.'
}
$BinaryPath = (Resolve-Path -LiteralPath $BinaryPath).Path
$version = & $BinaryPath --version
if ($LASTEXITCODE -ne 0 -or $version -notmatch '^cppm \d+\.') {
    throw 'The supplied executable did not identify itself as cppm.'
}
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
$destination = Join-Path $InstallDir 'cppm.exe'
$receiptPath = Join-Path $InstallDir 'cppm.install.json'
$pathAdded = $false
if (Test-Path -LiteralPath $receiptPath -PathType Leaf) {
    $receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
    $pathAdded = $receipt.pathAdded -eq $true
}
if ($BinaryPath -ne $destination) {
    $temporary = Join-Path $InstallDir ('cppm-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    try {
        Copy-Item -LiteralPath $BinaryPath -Destination $temporary
        Move-Item -LiteralPath $temporary -Destination $destination -Force
    }
    finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
    }
}
if (-not $NoPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_ })
    $present = @($entries | Where-Object {
        [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ieq $InstallDir.TrimEnd('\')
    }).Count -gt 0
    if (-not $present) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $InstallDir) -join ';'), 'User')
        $pathAdded = $true
    }
    if (-not (@($env:Path -split ';') -contains $InstallDir)) {
        $env:Path = $InstallDir + ';' + $env:Path
    }
}
@{ format = 1; pathAdded = $pathAdded } | ConvertTo-Json | Set-Content -LiteralPath $receiptPath -Encoding UTF8
Write-Host "Installed $version in $InstallDir"
Write-Host 'Open a new terminal and run cppm doctor. CMake, Ninja, Conan 2 and a C++ toolchain are installed separately.'
