[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$SetupPath)

$ErrorActionPreference = 'Stop'
$SetupPath = (Resolve-Path -LiteralPath $SetupPath).Path
$originalUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$originalProcessPath = $env:Path
$registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm'
if (Test-Path $registration) { throw 'Run this integration test on a disposable Windows runner without an installed cppm.' }
$directory = Join-Path ([IO.Path]::GetTempPath()) ('cppm setup test (spaces & symbols) ' + [Guid]::NewGuid().ToString('N'))
try {
    # Force the installer to obtain its own Python, Conan, CMake and Ninja.
    $env:Path = "$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"
    foreach ($attempt in 1..2) {
        Write-Host "Starting setup attempt $attempt..."
        # Start-Process -Wait also waits for descendants such as the resident
        # MSVC PDB server. Wait for setup itself, which already waits for its work.
        $process = Start-Process -FilePath $SetupPath -ArgumentList "/S /D=$directory" -PassThru
        if (-not $process.WaitForExit(600000)) { throw 'Setup did not exit within 10 minutes.' }
        $process.Refresh()
        Write-Host "Setup attempt $attempt exited with $($process.ExitCode)."
        if ($process.ExitCode -ne 0) {
            if (Test-Path (Join-Path $directory 'install.log')) { Get-Content (Join-Path $directory 'install.log') | Write-Host }
            throw "Setup failed (exit $($process.ExitCode))."
        }
    }
    if (-not (Test-Path $registration)) { throw 'Setup was not registered in Installed apps.' }
    $receipt = Get-Content -LiteralPath (Join-Path $directory 'bin\cppm.install.json') -Raw | ConvertFrom-Json
    if ($receipt.pathEntries.Count -ne 2) { throw 'Full setup did not record both PATH entries.' }
    foreach ($entry in $receipt.pathEntries) {
        $count = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ -ieq $entry }).Count
        if ($count -ne 1) { throw "Expected exactly one PATH entry for $entry, got $count." }
    }
    # Reconstruct a fresh terminal PATH, excluding everything inherited from CI.
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
    foreach ($tool in @('cppm', 'python', 'conan', 'cmake', 'ninja')) {
        if (-not (Test-Path -LiteralPath (Join-Path $directory $(if ($tool -eq 'cppm') { 'bin\cppm.exe' } else { "tools\venv\Scripts\$tool.exe" })))) {
            throw "Setup did not install $tool."
        }
    }
    $report = cppm doctor --json | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or -not $report.ok) { throw 'Fresh-terminal environment failed cppm doctor.' }
    $project = Join-Path $directory 'user-project'
    cppm init $project
    if ($LASTEXITCODE -ne 0) { throw 'Project creation failed.' }
    cppm -C $project run dev
    if ($LASTEXITCODE -ne 0) { throw 'Native C++ compilation/execution failed.' }
    cppm -C $project run test
    if ($LASTEXITCODE -ne 0) { throw 'Native C++ tests failed.' }
    Set-Content -LiteralPath (Join-Path $directory 'keep.txt') -Value 'user file'
    # _?= keeps NSIS in this process so Start-Process waits for all cleanup.
    $uninstaller = Join-Path $directory 'uninstall.exe'
    $process = Start-Process -FilePath $uninstaller -ArgumentList "/S _?=$directory" -PassThru
    if (-not $process.WaitForExit(120000)) { throw 'Uninstaller did not exit within 2 minutes.' }
    $process.Refresh()
    if ($process.ExitCode -ne 0) { throw 'Setup uninstaller failed.' }
    if (Test-Path $registration) { throw 'Uninstaller kept Installed apps registration.' }
    if (Test-Path -LiteralPath (Join-Path $directory 'bin\cppm.exe')) { throw 'Uninstaller kept cppm.' }
    if (Test-Path -LiteralPath (Join-Path $directory 'tools')) { throw 'Uninstaller kept its private tool environment.' }
    if (-not (Test-Path -LiteralPath $project) -or -not (Test-Path -LiteralPath (Join-Path $directory 'keep.txt'))) { throw 'Uninstaller removed user files.' }
    foreach ($entry in $receipt.pathEntries) {
        if (@([Environment]::GetEnvironmentVariable('Path', 'User') -split ';') -contains $entry) { throw 'Uninstaller kept an owned PATH entry.' }
    }
    Write-Host 'Setup integration passed: real downloads, reinstall, fresh PATH, native C++ build/test, Installed apps and selective uninstall.'
}
finally {
    [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User')
    $env:Path = $originalProcessPath
    if (Test-Path $registration) { Remove-Item $registration -Recurse }
    if (Test-Path 'HKCU:\Software\cppm') { Remove-Item 'HKCU:\Software\cppm' -Recurse }
    if (Test-Path -LiteralPath $directory) { Remove-Item -LiteralPath $directory -Recurse -Force }
}
