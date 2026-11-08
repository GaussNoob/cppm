[CmdletBinding()]
param([string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\cppm\bin'))

$ErrorActionPreference = 'Stop'
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$binary = Join-Path $InstallDir 'cppm.exe'
$receiptPath = Join-Path $InstallDir 'cppm.install.json'
$pathAdded = $false
if (Test-Path -LiteralPath $receiptPath -PathType Leaf) {
    $receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
    $pathAdded = $receipt.pathAdded -eq $true
}
if (Test-Path -LiteralPath $binary -PathType Leaf) {
    Remove-Item -LiteralPath $binary
}
if ($pathAdded) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object {
        $_ -and [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ine $InstallDir.TrimEnd('\')
    })
    [Environment]::SetEnvironmentVariable('Path', ($entries -join ';'), 'User')
    $env:Path = (@($env:Path -split ';' | Where-Object {
        $_ -and $_.TrimEnd('\') -ine $InstallDir.TrimEnd('\')
    }) -join ';')
}
if (Test-Path -LiteralPath $receiptPath) { Remove-Item -LiteralPath $receiptPath }
Write-Host 'cppm removed. Projects, dependencies and build tools were kept.'
