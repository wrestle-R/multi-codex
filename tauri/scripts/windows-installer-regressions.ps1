[CmdletBinding()]
param([string]$ScriptsDirectory = (Join-Path $PSScriptRoot '../../scripts'))
$ErrorActionPreference = 'Stop'
$ScriptsDirectory = (Resolve-Path -LiteralPath $ScriptsDirectory).Path
$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ("Multi Codex $([char]0x5DE5) O'Brien " + [guid]::NewGuid())
New-Item -ItemType Directory -Path $fixtureRoot | Out-Null
$fixturePackageName = 'Multi.Codex_1.4.0_x64-setup.exe'
$fixturePackagePath = Join-Path $fixtureRoot $fixturePackageName
[IO.File]::WriteAllBytes($fixturePackagePath, [Text.Encoding]::UTF8.GetBytes('MZ-checksum-regression-fixture'))
$fixtureHash = (Get-FileHash -LiteralPath $fixturePackagePath -Algorithm SHA256).Hash.ToLowerInvariant()
$fixtureRequests = New-Object System.Collections.ArrayList
$fixtureExecutions = New-Object System.Collections.ArrayList
$fixtureManifestBytes = $null
$fixtureOldEnvironment = @{}
foreach ($name in @('LOCALAPPDATA', 'PROCESSOR_ARCHITECTURE', 'PROCESSOR_ARCHITEW6432')) {
    $fixtureOldEnvironment[$name] = [Environment]::GetEnvironmentVariable($name)
}
$env:LOCALAPPDATA = $fixtureRoot
$env:PROCESSOR_ARCHITECTURE = 'AMD64'
Remove-Item Env:PROCESSOR_ARCHITEW6432 -ErrorAction SilentlyContinue

# Mock release transport and process execution only. Run the actual scripts,
# file reads, SHA-256 calculation and checksum parsing with no network or GUI.
function Get-Process {
    [CmdletBinding()]
    param([string]$Name)
    return @()
}
function Invoke-RestMethod {
    param([string]$Uri)
    if ($Uri -ne 'https://api.github.com/repos/wrestle-R/multi-codex/releases/latest') { throw 'Unexpected release request.' }
    return @{ tag_name = 'fixture'; assets = @(
        @{ name = $fixturePackageName; browser_download_url = 'https://fixture.invalid/installer' },
        @{ name = 'SHA256SUMS'; browser_download_url = 'https://fixture.invalid/sums' }
    ) }
}
function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    [void]$fixtureRequests.Add(@{ uri = $Uri; basicParsing = [bool]$UseBasicParsing })
    if ($Uri -eq 'https://fixture.invalid/installer') {
        Copy-Item -LiteralPath $fixturePackagePath -Destination $OutFile
        return
    }
    if ($Uri -ne 'https://fixture.invalid/sums') { throw 'Unexpected asset request.' }
    if ($OutFile) {
        [IO.File]::WriteAllBytes($OutFile, $fixtureManifestBytes)
    } else {
        # Match application/octet-stream responses, which exposed the old bug.
        return @{ Content = [byte[]]$fixtureManifestBytes }
    }
}
function Start-Process {
    param([string]$FilePath, [string]$ArgumentList, [switch]$Wait, [switch]$PassThru)
    if (!$Wait -or !$PassThru -or !$ArgumentList.StartsWith('/S /D=')) { throw 'Unexpected installer invocation.' }
    [void]$fixtureExecutions.Add($FilePath)
    $fixtureDestination = $ArgumentList.Substring('/S /D='.Length)
    New-Item -ItemType Directory -Path $fixtureDestination | Out-Null
    [IO.File]::WriteAllText((Join-Path $fixtureDestination 'multi-codex-desktop.exe'), 'installed fixture')
    return @{ ExitCode = 0 }
}

$fixtureCases = @(
    @{ name = 'binary LF manifest'; text = "$fixtureHash  $fixturePackageName`n"; succeeds = $true },
    @{ name = 'binary CRLF manifest'; text = "$fixtureHash *$fixturePackageName`r`n"; succeeds = $true },
    @{ name = 'incorrect package checksum'; text = "$('0' * 64)  $fixturePackageName`n"; succeeds = $false },
    @{ name = 'missing package entry'; text = "$fixtureHash  other.exe`n"; succeeds = $false },
    @{ name = 'duplicate package entries'; text = "$fixtureHash  $fixturePackageName`n$fixtureHash  $fixturePackageName`n"; succeeds = $false }
)
$fixtureChecks = 0
try {
    foreach ($script in @('install-app.ps1', 'update-app.ps1')) {
        foreach ($case in $fixtureCases) {
            $fixtureRequests.Clear()
            $fixtureExecutions.Clear()
            $fixtureManifestBytes = [Text.Encoding]::UTF8.GetBytes($case.text)
            $fixtureInstallDirectory = Join-Path $fixtureRoot ([guid]::NewGuid().ToString())
            $fixtureFailure = $null
            try { & (Join-Path $ScriptsDirectory $script) -NoLaunch -InstallDirectory $fixtureInstallDirectory }
            catch { $fixtureFailure = $_.Exception.Message }
            if ($case.succeeds) {
                if ($fixtureFailure) { throw "$script / $($case.name): $fixtureFailure" }
                if ($fixtureExecutions.Count -ne 1) { throw 'Verified package must execute exactly once.' }
                if (!(Test-Path -LiteralPath (Join-Path $fixtureInstallDirectory 'multi-codex-desktop.exe'))) { throw 'Installed executable was not checked.' }
            } else {
                if (!$fixtureFailure -or $fixtureFailure -notmatch 'checksum verification failed|checksum manifest must contain exactly one') { throw "$script / $($case.name): expected checksum rejection, got $fixtureFailure" }
                if ($fixtureExecutions.Count -ne 0 -or (Test-Path -LiteralPath $fixtureInstallDirectory)) { throw 'Invalid download must be rejected before execution.' }
            }
            if ($fixtureRequests.Count -ne 2 -or @($fixtureRequests | Where-Object { !$_.basicParsing }).Count -ne 0) { throw 'All asset downloads must use basic parsing.' }
            $fixtureChecks++
            Write-Host "PASS: $script / $($case.name)"
        }
    }
    Write-Host "$fixtureChecks installer regression checks passed on PowerShell $($PSVersionTable.PSVersion)."
} finally {
    foreach ($name in $fixtureOldEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $fixtureOldEnvironment[$name])
    }
    Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
}
