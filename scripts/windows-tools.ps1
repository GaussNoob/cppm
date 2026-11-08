# Shared by the Windows setup and its tests. Loading this file has no side effects.
function Get-CppmPathKey([string]$Value) {
    [Environment]::ExpandEnvironmentVariables($Value).TrimEnd('\').ToLowerInvariant()
}

function Get-CppmPathEntries([string]$Value) {
    @($Value -split ';' | Where-Object { $_ })
}

function Add-CppmPathEntries([string]$Value, [string[]]$Directories) {
    $entries = @(Get-CppmPathEntries $Value)
    $originalEntries = @($entries)
    $added = @()
    foreach ($directory in $Directories) {
        $key = Get-CppmPathKey $directory
        if (-not @($entries | Where-Object { (Get-CppmPathKey $_) -eq $key }).Count) {
            $entries += $directory
            $added += $directory
        }
    }
    # New entries precede WindowsApps, whose Python execution alias can open the
    # Microsoft Store instead of the private interpreter. Existing order stays intact.
    @{ Value = ($added + $originalEntries) -join ';'; Added = $added }
}

function Remove-CppmPathEntries([string]$Value, [string[]]$Directories) {
    $keys = @($Directories | ForEach-Object { Get-CppmPathKey $_ })
    (@(Get-CppmPathEntries $Value | Where-Object { $keys -notcontains (Get-CppmPathKey $_) }) -join ';')
}

function Send-CppmEnvironmentChange {
    if ($env:OS -ne 'Windows_NT') { return }
    if (-not ('Cppm.EnvironmentBroadcast' -as [type])) {
        Add-Type @'
using System;
using System.Runtime.InteropServices;
namespace Cppm {
    public static class EnvironmentBroadcast {
        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr SendMessageTimeout(IntPtr window, uint message,
            UIntPtr wParam, string lParam, uint flags, uint timeout, out UIntPtr result);
    }
}
'@
    }
    $result = [UIntPtr]::Zero
    [void][Cppm.EnvironmentBroadcast]::SendMessageTimeout(
        [IntPtr]0xffff, 0x001a, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result)
}

function Invoke-CppmTool([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE." }
}

function Get-CppmDownload([string]$Url, [string]$Destination, [string]$Sha256) {
    if (Test-Path -LiteralPath $Destination -PathType Leaf) {
        if ($Sha256 -and (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ieq $Sha256) { return }
    }
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        try {
            Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing
            if ($Sha256 -and (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ine $Sha256) {
                throw "SHA-256 verification failed for $Url."
            }
            return
        }
        catch {
            if (Test-Path -LiteralPath $Destination) { Remove-Item -LiteralPath $Destination }
            if ($attempt -eq 3) { throw }
            Write-Host "Download failed; retrying ($attempt/3)..."
            Start-Sleep -Seconds 2
        }
    }
}

function Get-CppmVisualStudio {
    $programFiles = ${env:ProgramFiles(x86)}
    if (-not $programFiles) { return $null }
    $vswhere = Join-Path $programFiles 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) { return $null }
    $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath -utf8
    if ($LASTEXITCODE -ne 0 -or -not $installation) { return $null }
    $installation = ($installation -join '').Trim().TrimStart([char]0xfeff)
    if (-not (Test-Path -LiteralPath (Join-Path $installation 'Common7\Tools\VsDevCmd.bat'))) { return $null }
    $sdkRoot = Join-Path $programFiles 'Windows Kits\10\Lib'
    $sdk = @(Get-ChildItem -LiteralPath $sdkRoot -Directory -ErrorAction SilentlyContinue | Where-Object {
        (Test-Path -LiteralPath (Join-Path $_.FullName 'um\x64\kernel32.lib')) -and
        (Test-Path -LiteralPath (Join-Path $_.FullName 'ucrt\x64\ucrt.lib'))
    })
    if ($sdk.Count -eq 0) { return $null }
    $installation
}

function Install-CppmBuildTools([string]$DownloadDir, $Configuration) {
    if (Get-CppmVisualStudio) {
        Write-Host 'MSVC and Windows SDK already installed; keeping them.'
        return $false
    }
    Write-Host 'Downloading Microsoft C++ Build Tools. Windows will request administrator approval.'
    Write-Host 'The C++ compiler and Windows SDK can require several GB and take a while.'
    $bootstrapper = Join-Path $DownloadDir 'vs_BuildTools.exe'
    Get-CppmDownload $Configuration.buildToolsUrl $bootstrapper
    $signature = Get-AuthenticodeSignature -LiteralPath $bootstrapper
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '(?:^|,\s*)CN=Microsoft Corporation(?:,|$)') {
        throw 'The Microsoft Build Tools installer does not have a valid Microsoft signature.'
    }
    $process = Start-Process -FilePath $bootstrapper -Verb RunAs -ArgumentList @(
        '--passive', '--wait', '--norestart', '--add',
        'Microsoft.VisualStudio.Workload.VCTools', '--includeRecommended'
    ) -PassThru -Wait
    if ($process.ExitCode -eq 3010) { return $true }
    if ($process.ExitCode -ne 0) { throw "Microsoft Build Tools installation failed (exit $($process.ExitCode)). Rerun setup to retry." }
    if (-not (Get-CppmVisualStudio)) { throw 'MSVC or Windows SDK is still missing after installation.' }
    $false
}

function Install-CppmTools([string]$ToolsDir) {
    $configuration = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'windows-tools.json') -Raw | ConvertFrom-Json
    $marker = Join-Path $ToolsDir 'cppm.tools.json'
    if (Test-Path -LiteralPath $ToolsDir) {
        if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
            throw "Refusing to change an existing tools directory without a cppm ownership marker: $ToolsDir"
        }
        $owner = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
        if ($owner.managedBy -ne 'cppm') { throw 'Tools directory is not owned by cppm.' }
    }
    New-Item -ItemType Directory -Path $ToolsDir -Force | Out-Null
    @{ format = 1; managedBy = 'cppm' } | ConvertTo-Json | Set-Content -LiteralPath $marker -Encoding UTF8
    $downloads = Join-Path $ToolsDir 'downloads'
    New-Item -ItemType Directory -Path $downloads -Force | Out-Null
    $uvZip = Join-Path $downloads "uv-$($configuration.uvVersion).zip"
    Write-Host 'Preparing the private Python and build tools environment...'
    Get-CppmDownload $configuration.uvUrl $uvZip $configuration.uvSha256
    $uvDir = Join-Path $ToolsDir 'uv'
    Expand-Archive -LiteralPath $uvZip -DestinationPath $uvDir -Force
    $uv = Join-Path $uvDir 'uv.exe'
    if (-not (Test-Path -LiteralPath $uv -PathType Leaf)) { throw 'uv.exe missing from the verified archive.' }
    # Scope all uv changes to this process and this installation. Existing Python
    # installs, uv configuration and the global tool directory stay untouched.
    $variables = @{
        UV_PYTHON_INSTALL_DIR = (Join-Path $ToolsDir 'python')
        UV_PYTHON_INSTALL_BIN = '0'
        UV_CACHE_DIR = (Join-Path $ToolsDir 'cache')
        UV_NO_CONFIG = '1'
        UV_NO_PROGRESS = '1'
        UV_NATIVE_TLS = '1'
    }
    $original = @{}
    try {
        foreach ($name in $variables.Keys) {
            $original[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
            [Environment]::SetEnvironmentVariable($name, $variables[$name], 'Process')
        }
        Invoke-CppmTool $uv @('python', 'install', $configuration.python)
        $venv = Join-Path $ToolsDir 'venv'
        $python = Join-Path $venv 'Scripts\python.exe'
        if (-not (Test-Path -LiteralPath $python -PathType Leaf)) {
            Invoke-CppmTool $uv @('venv', '--managed-python', '--python', $configuration.python, $venv)
        }
        Invoke-CppmTool $uv (@('pip', 'install', '--python', $python, '--index-url', 'https://pypi.org/simple') + @($configuration.packages))
    }
    finally {
        foreach ($name in $original.Keys) { [Environment]::SetEnvironmentVariable($name, $original[$name], 'Process') }
    }
    $scripts = Join-Path $ToolsDir 'venv\Scripts'
    foreach ($tool in @('python', 'conan', 'cmake', 'ninja')) {
        Invoke-CppmTool (Join-Path $scripts "$tool.exe") @('--version')
    }
    Write-Host 'Installing the WebAssembly SDK and its private Node.js runtime...'
    Invoke-CppmTool (Join-Path $scripts 'python.exe') @(
        (Join-Path $PSScriptRoot 'setup-wasm.py'), '--sdk-dir', (Join-Path $ToolsDir 'emsdk')
    )
    $reboot = Install-CppmBuildTools $downloads $configuration
    @{ Scripts = $scripts; RebootRequired = $reboot }
}
