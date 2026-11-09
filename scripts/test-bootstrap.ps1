[CmdletBinding()]
param([string]$BinaryPath)

$ErrorActionPreference = 'Stop'
Import-Module Microsoft.PowerShell.Archive
if (-not $BinaryPath) { $BinaryPath = Join-Path $PSScriptRoot '..\target\release\cppm.exe' }
$BinaryPath = (Resolve-Path -LiteralPath $BinaryPath).Path
$version = ((& $BinaryPath --version) -replace '^cppm ', '').Trim()
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('cppm download test (spaces & symbols) ' + [Guid]::NewGuid().ToString('N'))
$installDir = Join-Path $temporary 'installed bin'
$originalArchitecture = $env:PROCESSOR_ARCHITECTURE
$originalWowArchitecture = $env:PROCESSOR_ARCHITEW6432
$originalLocalAppData = $env:LOCALAPPDATA
$testState = @{ Requests = 0 }

function Assert-Throws([scriptblock]$Action, [string]$Pattern) {
    $caught = $null
    try { & $Action | Out-Null } catch { $caught = $_ }
    if (-not $caught -or $caught.ToString() -notmatch $Pattern) { throw "Expected '$Pattern', got '$caught'." }
}
function New-TestPackage([string]$ReleaseVersion) {
    $stem = "cppm-$ReleaseVersion-windows-x86_64"
    $package = Join-Path $temporary $stem
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    Copy-Item -LiteralPath $BinaryPath -Destination (Join-Path $package 'cppm.exe')
    foreach ($file in @('install.ps1', 'windows-tools.ps1', 'windows-tools.json')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination $package
    }
    $testState.archive = Join-Path $temporary "$stem.zip"
    Compress-Archive -LiteralPath $package -DestinationPath $testState.archive -Force
    $testState.digest = (Get-FileHash -LiteralPath $testState.archive -Algorithm SHA256).Hash
    $testState.releaseVersion = $ReleaseVersion
}
# Replace only transport. Real ZIP extraction, hashing, validation and installation run.
function Invoke-WebRequest {
    param($Uri, $OutFile, [switch]$UseBasicParsing, $TimeoutSec)
    $testState.requests++
    if (-not $Uri.StartsWith('https://github.com/GaussNoob/cppm/releases/')) { throw "Unexpected URI: $Uri" }
    if ($Uri.EndsWith('/VERSION')) { [IO.File]::WriteAllText($OutFile, "$($testState.releaseVersion)`n") }
    elseif ($Uri.EndsWith('.sha256')) { [IO.File]::WriteAllText($OutFile, "$($testState.digest)  archive.zip`n") }
    elseif ($Uri.EndsWith('.zip')) { Copy-Item -LiteralPath $testState.archive -Destination $OutFile }
    else { throw "Unexpected download: $Uri" }
}
if ($PSVersionTable.PSEdition -eq 'Core' -and -not $IsWindows) {
    function Expand-Archive {
        param($LiteralPath, $DestinationPath)
        Microsoft.PowerShell.Archive\Expand-Archive -LiteralPath $LiteralPath -DestinationPath $DestinationPath
        Get-ChildItem -LiteralPath $DestinationPath -Filter cppm.exe -Recurse | ForEach-Object { & chmod +x $_.FullName }
    }
}

try {
    New-Item -ItemType Directory -Path $temporary | Out-Null
    $env:PROCESSOR_ARCHITECTURE = 'AMD64'
    $env:PROCESSOR_ARCHITEW6432 = $null
    if (-not $env:LOCALAPPDATA) { $env:LOCALAPPDATA = $temporary }
    New-TestPackage $version
    & "$PSScriptRoot/bootstrap.ps1" -InstallDir $installDir -NoPath
    $installed = Join-Path $installDir 'cppm.exe'
    if ((& $installed --version) -ne "cppm $version") { throw 'Remote installation failed.' }
    # A pinned installation must not need the latest-version endpoint.
    $testState.requests = 0
    & "$PSScriptRoot/bootstrap.ps1" -InstallDir $installDir -NoPath -Version "v$version"
    if ($testState.requests -ne 2) { throw 'Pinned installation fetched unexpected URLs.' }
    $receipt = Get-Content -LiteralPath (Join-Path $installDir 'cppm.install.json') -Raw | ConvertFrom-Json
    if ($receipt.pathAdded) { throw 'NoPath claimed ownership of PATH.' }
    $before = (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash
    if ($env:OS -eq 'Windows_NT') {
        # Keep cppm's image loaded while the installer replaces that exact path.
        $holdingProject = Join-Path $temporary 'holding project'
        New-Item -ItemType Directory -Path $holdingProject | Out-Null
        @'
[package]
name = "holding"
version = "0.1.0"
[scripts]
hold = ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "[IO.File]::WriteAllText('ready', ''); while (-not [IO.File]::Exists('release')) { Start-Sleep -Milliseconds 100 }"]
'@ | Set-Content -LiteralPath (Join-Path $holdingProject 'cppm.toml')
        $holding = Start-Process -FilePath $installed -ArgumentList @('--offline', 'run', 'hold') -WorkingDirectory $holdingProject -PassThru -WindowStyle Hidden
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(15)
            while (-not (Test-Path -LiteralPath (Join-Path $holdingProject 'ready'))) {
                if ($holding.HasExited -or [DateTime]::UtcNow -ge $deadline) { throw 'Could not keep the installed cppm image running.' }
                Start-Sleep -Milliseconds 100
            }
            & "$PSScriptRoot/bootstrap.ps1" -InstallDir $installDir -NoPath -Version $version
            if ((& $installed --version) -ne "cppm $version") { throw 'Replacing a running Windows executable failed.' }
        }
        finally {
            Set-Content -LiteralPath (Join-Path $holdingProject 'release') -Value 'stop'
            if (-not $holding.WaitForExit(15000)) { Stop-Process -Id $holding.Id -Force }
        }
    }
    # Inject a failure only at the final replacement, then require rollback.
    & {
        function Move-Item {
            param($LiteralPath, $Destination, [switch]$Force)
            if ($LiteralPath.EndsWith('.tmp')) { throw 'Simulated replacement failure' }
            Microsoft.PowerShell.Management\Move-Item -LiteralPath $LiteralPath -Destination $Destination -Force:$Force
        }
        Assert-Throws { & "$PSScriptRoot/bootstrap.ps1" -InstallDir $installDir -NoPath -Version $version } 'Simulated replacement failure'
    }
    if ((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -ne $before) { throw 'Failed replacement did not restore the original binary.' }
    $testState.digest = '0' * 64
    Assert-Throws { & "$PSScriptRoot/bootstrap.ps1" -InstallDir $installDir -NoPath } 'SHA-256 mismatch'
    if ((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -ne $before) { throw 'Checksum failure changed the installation.' }
    $testState.requests = 0
    Assert-Throws { & "$PSScriptRoot/bootstrap.ps1" -InstallDir $installDir -NoPath -Version '1.2.3;whoami' } 'Invalid release version'
    if ($testState.requests -ne 0) { throw 'Invalid version caused a download.' }
    New-TestPackage '999.0.0'
    Assert-Throws { & "$PSScriptRoot/bootstrap.ps1" -InstallDir $installDir -NoPath } 'Release binary version mismatch'
    if ((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash -ne $before) { throw 'Version mismatch changed the installation.' }
}
finally {
    $env:PROCESSOR_ARCHITECTURE = $originalArchitecture
    $env:PROCESSOR_ARCHITEW6432 = $originalWowArchitecture
    $env:LOCALAPPDATA = $originalLocalAppData
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
Write-Host 'Windows download tests passed: installation, pinned version, SHA-256, version validation and unchanged installation on failure.'
