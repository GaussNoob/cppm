[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BinaryPath,
    [Parameter(Mandatory = $true)][string]$InstallRoot
)

$ErrorActionPreference = 'Stop'
$exitCode = 1
$transcript = $false
try {
    Start-Transcript -LiteralPath (Join-Path $InstallRoot 'install.log') -Append | Out-Null
    $transcript = $true
    & (Join-Path $PSScriptRoot 'install.ps1') -BinaryPath $BinaryPath `
        -InstallDir (Join-Path $InstallRoot 'bin') -ToolsDir (Join-Path $InstallRoot 'tools') -WithDependencies
    $receipt = Get-Content -LiteralPath (Join-Path $InstallRoot 'bin\cppm.install.json') -Raw | ConvertFrom-Json
    if ($receipt.rebootRequired) { $exitCode = 3010 } else { $exitCode = 0 }
}
catch {
    Write-Host "Installation failed: $_"
    Write-Host 'Check install.log in the installation folder and rerun setup to retry.'
}
finally {
    if ($transcript) { Stop-Transcript | Out-Null }
}
exit $exitCode
