[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\cppm\bin'),
    [switch]$RemoveTools
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-tools.ps1')
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$binary = Join-Path $InstallDir 'cppm.exe'
$receiptPath = Join-Path $InstallDir 'cppm.install.json'
$ownedEntries = @()
$toolsDir = $null
if (Test-Path -LiteralPath $receiptPath -PathType Leaf) {
    $receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
    $ownedEntries = @($receipt.pathEntries | Where-Object { $_ })
    if ($receipt.pathAdded -eq $true -and $ownedEntries -notcontains $InstallDir) { $ownedEntries += $InstallDir }
    $toolsDir = $receipt.toolsDir
}
if ($RemoveTools -and $toolsDir -and (Test-Path -LiteralPath $toolsDir)) {
    $marker = Join-Path $toolsDir 'cppm.tools.json'
    if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) { throw 'Refusing to remove tools without a cppm ownership marker.' }
    $owner = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
    if ($owner.managedBy -ne 'cppm') { throw 'Refusing to remove a tools directory not owned by cppm.' }
    Remove-Item -LiteralPath $toolsDir -Recurse
}
if (Test-Path -LiteralPath $binary -PathType Leaf) {
    Remove-Item -LiteralPath $binary
}
$previous = Join-Path $InstallDir 'cppm.exe.previous'
if (Test-Path -LiteralPath $previous -PathType Leaf) { Remove-Item -LiteralPath $previous }
if ($ownedEntries.Count -gt 0) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    [Environment]::SetEnvironmentVariable('Path', (Remove-CppmPathEntries $userPath $ownedEntries), 'User')
    $env:Path = Remove-CppmPathEntries $env:Path $ownedEntries
    Send-CppmEnvironmentChange
}
if (Test-Path -LiteralPath $receiptPath) { Remove-Item -LiteralPath $receiptPath }
Write-Host 'cppm removed. Projects, Conan package caches and shared Microsoft Build Tools were kept.'
