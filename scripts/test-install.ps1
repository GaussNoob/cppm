$ErrorActionPreference = 'Stop'
$originalUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$originalProcessPath = $env:Path
$directory = Join-Path ([IO.Path]::GetTempPath()) ('cppm install test (spaces & symbols) ' + [Guid]::NewGuid().ToString('N'))
try {
    & "$PSScriptRoot/install.ps1" -InstallDir $directory
    & "$PSScriptRoot/install.ps1" -InstallDir $directory
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ -ieq $directory })
    if ($entries.Count -ne 1) { throw 'Installer did not persist exactly one PATH entry.' }
    $version = & (Join-Path $directory 'cppm.exe') --version
    if ($LASTEXITCODE -ne 0 -or $version -notmatch '^cppm ') { throw 'Installed binary failed.' }
    $doctor = & (Join-Path $directory 'cppm.exe') doctor --json
    $doctorExit = $LASTEXITCODE
    $report = ($doctor -join "`n") | ConvertFrom-Json
    if ($doctorExit -ne 0 -or -not $report.ok) {
        # Show which check failed; the JSON is otherwise swallowed by ConvertFrom-Json.
        Write-Host ($doctor -join "`n")
        throw 'Installed binary failed the native compiler probe.'
    }
    & "$PSScriptRoot/uninstall.ps1" -InstallDir $directory
    if (Test-Path (Join-Path $directory 'cppm.exe')) { throw 'Uninstaller kept the binary.' }
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ -ieq $directory })
    if ($entries.Count -ne 0) { throw 'Uninstaller kept the PATH entry.' }
    # A shared/pre-existing PATH entry belongs to the user, not to this installer.
    [Environment]::SetEnvironmentVariable('Path', (($originalUserPath, $directory | Where-Object { $_ }) -join ';'), 'User')
    & "$PSScriptRoot/install.ps1" -InstallDir $directory
    Set-Content -LiteralPath (Join-Path $directory 'unrelated.txt') -Value 'keep'
    & "$PSScriptRoot/uninstall.ps1" -InstallDir $directory
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ -ieq $directory })
    if ($entries.Count -ne 1) { throw 'Uninstaller removed a pre-existing PATH entry.' }
    if (-not (Test-Path (Join-Path $directory 'unrelated.txt'))) { throw 'Uninstaller removed an unrelated file.' }
    & "$PSScriptRoot/install.ps1" -InstallDir $directory -NoPath
    $receipt = Get-Content -LiteralPath (Join-Path $directory 'cppm.install.json') -Raw | ConvertFrom-Json
    if ($receipt.pathAdded) { throw 'NoPath installation claimed ownership of the user PATH.' }
    & "$PSScriptRoot/uninstall.ps1" -InstallDir $directory
}
finally {
    [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User')
    $env:Path = $originalProcessPath
    if (Test-Path -LiteralPath $directory) { Remove-Item -LiteralPath $directory -Recurse }
}
