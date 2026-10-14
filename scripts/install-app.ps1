[CmdletBinding()]
param([switch]$NoLaunch, [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA 'Multi Codex'))
$ErrorActionPreference = 'Stop'
if (![Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -notin @('AMD64', 'x86') -or ($env:PROCESSOR_ARCHITEW6432 -and $env:PROCESSOR_ARCHITEW6432 -ne 'AMD64')) { throw 'Multi Codex requires native Windows x64.' }
if (Get-Process -Name 'multi-codex-desktop' -ErrorAction SilentlyContinue) { throw 'Close Multi Codex before installing or updating. Saved accounts will be preserved.' }
if (![System.IO.Path]::IsPathRooted($InstallDirectory)) { throw 'InstallDirectory must be an absolute path.' }
$release = Invoke-RestMethod 'https://api.github.com/repos/wrestle-R/multi-codex/releases/latest'
$installer = @($release.assets | Where-Object { $_.name -match '_x64-setup\.exe$' })
$sums = @($release.assets | Where-Object { $_.name -eq 'SHA256SUMS' })
if ($installer.Count -ne 1 -or $sums.Count -ne 1) { throw 'The latest release has no unique Windows installer and checksum manifest.' }
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('multi-codex-install-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $package = Join-Path $temporary $installer[0].name
    Invoke-WebRequest $installer[0].browser_download_url -OutFile $package
    $manifest = (Invoke-WebRequest $sums[0].browser_download_url).Content
    $pattern = '(?m)^([a-fA-F0-9]{64})\s+\*?' + [regex]::Escape($installer[0].name) + '\r?$'
    $matches = [regex]::Matches($manifest, $pattern)
    if ($matches.Count -ne 1 -or (Get-FileHash $package -Algorithm SHA256).Hash -ne $matches[0].Groups[1].Value) { throw 'Windows installer checksum verification failed.' }
    # NSIS requires /D to be the final argument; it handles spaces itself.
    $process = Start-Process -FilePath $package -ArgumentList "/S /D=$InstallDirectory" -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Windows installer failed with exit code $($process.ExitCode)." }
    $application = Join-Path $InstallDirectory 'multi-codex-desktop.exe'
    if (!(Test-Path -LiteralPath $application -PathType Leaf)) { throw 'The installed app executable is missing.' }
    if (!$NoLaunch) { Start-Process -FilePath $application }
    Write-Host "Multi Codex $($release.tag_name) installed. Saved accounts and profile data were preserved."
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
