$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-tools.ps1')

function Assert-True($Value, [string]$Message) {
    if (-not $Value) { throw $Message }
}
function Assert-Throws([scriptblock]$Action, [string]$Pattern) {
    $caught = $null
    try { & $Action | Out-Null } catch { $caught = $_ }
    if (-not $caught -or $caught.ToString() -notmatch $Pattern) { throw "Expected error matching '$Pattern', got '$caught'." }
}

$result = Add-CppmPathEntries 'C:\Windows;c:\Cppm\Bin\;C:\User Tools' @('C:\cppm\bin', 'C:\cppm\tools\venv\Scripts')
Assert-True ($result.Added.Count -eq 1) 'Existing PATH entries must not be claimed by cppm.'
Assert-True ($result.Value -eq 'C:\Windows;c:\Cppm\Bin\;C:\User Tools;C:\cppm\tools\venv\Scripts') 'PATH order changed.'
$removed = Remove-CppmPathEntries $result.Value $result.Added
Assert-True ($removed -eq 'C:\Windows;c:\Cppm\Bin\;C:\User Tools') 'Removing owned entries must preserve the original PATH.'
$again = Add-CppmPathEntries $result.Value @('C:\cppm\bin', 'C:\cppm\tools\venv\Scripts')
Assert-True ($again.Added.Count -eq 0) 'Reinstallation duplicated PATH entries.'

$temporary = Join-Path ([IO.Path]::GetTempPath()) ('cppm tools test ' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    # Download tests use actual files and hashing, with only transport replaced.
    & {
        function Invoke-WebRequest { param($Uri, $OutFile, [switch]$UseBasicParsing) [IO.File]::WriteAllText($OutFile, 'verified bytes') }
        function Start-Sleep { param($Seconds) }
        $file = Join-Path $temporary 'download.zip'
        [IO.File]::WriteAllText($file, 'verified bytes')
        $digest = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
        Get-CppmDownload 'https://example.invalid/archive' $file $digest
        Assert-Throws { Get-CppmDownload 'https://example.invalid/archive' $file ('0' * 64) } 'SHA-256'
        Assert-True (-not (Test-Path -LiteralPath $file)) 'An invalid download was retained.'
        Get-CppmDownload 'https://example.invalid/archive' $file $digest
        Assert-True ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -eq $digest) 'Valid retry failed.'
    }
    & {
        function Get-CppmVisualStudio { 'C:\VS' }
        function Get-CppmDownload { throw 'An installed compiler must not be downloaded again.' }
        Assert-True ((Install-CppmBuildTools $temporary @{}) -eq $false) 'Existing MSVC requested a restart.'
    }
    & {
        function Get-CppmVisualStudio { $null }
        function Get-CppmDownload { }
        function Get-AuthenticodeSignature {
            @{ Status = 'Valid'; SignerCertificate = @{ Subject = 'CN=Microsoft Corporation, O=Microsoft Corporation' } }
        }
        function Start-Process { param($FilePath, $Verb, $ArgumentList, [switch]$PassThru, [switch]$Wait) @{ ExitCode = 3010 } }
        Assert-True ((Install-CppmBuildTools $temporary @{ buildToolsUrl = 'https://example.invalid' }) -eq $true) 'MSVC restart code was lost.'
        function Start-Process { param($FilePath, $Verb, $ArgumentList, [switch]$PassThru, [switch]$Wait) @{ ExitCode = 1602 } }
        Assert-Throws { Install-CppmBuildTools $temporary @{} } 'exit 1602'
        function Get-AuthenticodeSignature { @{ Status = 'NotSigned'; SignerCertificate = @{ Subject = '' } } }
        Assert-Throws { Install-CppmBuildTools $temporary @{} } 'valid Microsoft signature'
        function Get-AuthenticodeSignature { @{ Status = 'Valid'; SignerCertificate = @{ Subject = 'CN=Another Publisher' } } }
        Assert-Throws { Install-CppmBuildTools $temporary @{} } 'valid Microsoft signature'
    }
    & {
        $unowned = Join-Path $temporary 'unowned'
        New-Item -ItemType Directory -Path $unowned | Out-Null
        Assert-Throws { Install-CppmTools $unowned } 'ownership marker'
        $originalNoConfig = $env:UV_NO_CONFIG
        $env:UV_NO_CONFIG = 'previous-value'
        try {
            function Get-CppmDownload { }
            function Expand-Archive {
                param($LiteralPath, $DestinationPath, [switch]$Force)
                New-Item -ItemType Directory -Path $DestinationPath -Force | Out-Null
                New-Item -ItemType File -Path (Join-Path $DestinationPath 'uv.exe') | Out-Null
            }
            function Invoke-CppmTool { throw 'Simulated Python download failure.' }
            Assert-Throws { Install-CppmTools (Join-Path $temporary 'managed') } 'Simulated Python download failure'
            Assert-True ($env:UV_NO_CONFIG -eq 'previous-value') 'Failed setup did not restore uv environment variables.'
        }
        finally { $env:UV_NO_CONFIG = $originalNoConfig }
    }
}
finally { Remove-Item -LiteralPath $temporary -Recurse }
Write-Host 'Windows tools tests passed: PATH ownership, retries, integrity, signature checks, cancellation, restart and environment isolation.'
