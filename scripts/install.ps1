[CmdletBinding()]
param(
    [string]$BinaryPath,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\cppm\bin'),
    [switch]$NoPath,
    [switch]$WithDependencies,
    [string]$ToolsDir
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-tools.ps1')
if ($WithDependencies -and $NoPath) { throw 'A complete installation needs PATH configuration; omit -NoPath.' }
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
$ownedEntries = @()
$managedToolsDir = $null
if (Test-Path -LiteralPath $receiptPath -PathType Leaf) {
    $receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
    $ownedEntries = @($receipt.pathEntries | Where-Object { $_ })
    if ($receipt.pathAdded -eq $true -and $ownedEntries -notcontains $InstallDir) { $ownedEntries += $InstallDir }
    $managedToolsDir = $receipt.toolsDir
}
$directories = @($InstallDir)
$rebootRequired = $false
if ($WithDependencies) {
    if (-not $ToolsDir) { $ToolsDir = Join-Path $InstallDir 'tools' }
    $ToolsDir = [IO.Path]::GetFullPath($ToolsDir)
    $tools = Install-CppmTools $ToolsDir
    $managedToolsDir = $ToolsDir
    $directories += $tools.Scripts
    $rebootRequired = $tools.RebootRequired
}
if ($BinaryPath -ne $destination) {
    $temporary = Join-Path $InstallDir ('cppm-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    try {
        Copy-Item -LiteralPath $BinaryPath -Destination $temporary
        # Windows can rename a running image, but cannot overwrite it in place.
        $previous = Join-Path $InstallDir 'cppm.exe.previous'
        if (Test-Path -LiteralPath $previous) { Remove-Item -LiteralPath $previous -Force }
        if (Test-Path -LiteralPath $destination) { Move-Item -LiteralPath $destination -Destination $previous }
        try {
            Move-Item -LiteralPath $temporary -Destination $destination
        }
        catch {
            if (Test-Path -LiteralPath $previous) { Move-Item -LiteralPath $previous -Destination $destination }
            throw
        }
        # During self-update the old process still owns this file until it exits.
        if (Test-Path -LiteralPath $previous) {
            try { Remove-Item -LiteralPath $previous -Force } catch { Write-Verbose 'Previous executable will be cleaned up at the next installation.' }
        }
    }
    finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
    }
}
if (-not $NoPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $updated = Add-CppmPathEntries $userPath $directories
    foreach ($entry in $updated.Added) {
        if ($ownedEntries -notcontains $entry) { $ownedEntries += $entry }
    }
    # Probe the same PATH a fresh terminal will receive, including machine entries.
    # This catches conflicts with older system-wide tools before reporting success.
    if ($WithDependencies) {
        $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + $updated.Value
    }
    else { $env:Path = (Add-CppmPathEntries $env:Path @($InstallDir)).Value }
    if ($WithDependencies -and -not $rebootRequired) {
        Write-Host 'Checking the complete C++ environment...'
        $report = & $destination doctor --json
        if ($LASTEXITCODE -ne 0) { throw "C++ environment validation failed:`n$($report -join "`n")" }
        $diagnostics = $report -join "`n" | ConvertFrom-Json
        if (-not $diagnostics.ok) { throw 'cppm doctor reported an incomplete environment.' }
    }
    [Environment]::SetEnvironmentVariable('Path', $updated.Value, 'User')
}
@{
    format = 2
    pathAdded = $ownedEntries -contains $InstallDir
    pathEntries = @($ownedEntries)
    toolsDir = $managedToolsDir
    rebootRequired = $rebootRequired
} | ConvertTo-Json | Set-Content -LiteralPath $receiptPath -Encoding UTF8
if (-not $NoPath) { Send-CppmEnvironmentChange }
Write-Host "Installed $version in $InstallDir"
if ($rebootRequired) {
    Write-Host 'Microsoft Build Tools requires a restart. Restart Windows, then open a new terminal and run cppm doctor.'
}
elseif ($WithDependencies) {
    Write-Host 'Ready. Open a new terminal and run cppm init my-app, then cd my-app and cppm run dev.'
}
else {
    Write-Host 'Open a new terminal and run cppm doctor. Use the setup .exe or -WithDependencies to install the build tools automatically.'
}
