[CmdletBinding()]
param([Parameter(Mandatory)][string]$Installer)
$ErrorActionPreference = 'Stop'
$candidateInstallerPath = (Resolve-Path -LiteralPath $Installer).Path
$root = Join-Path $env:RUNNER_TEMP ('Multi Codex Windows 工具 O''Brien ' + [guid]::NewGuid())
$install = Join-Path $root 'application'
$evidence = Join-Path $env:RUNNER_TEMP 'multi-codex-windows-evidence'
New-Item -ItemType Directory -Path $root,$evidence | Out-Null
$oldRoaming = $env:APPDATA; $oldHome = $env:CODEX_HOME
$env:APPDATA = Join-Path $root 'roaming'
$env:CODEX_HOME = Join-Path $root 'global-home'
$data = Join-Path $env:APPDATA 'multi-codex'
$profile = Join-Path $data 'profiles/00000000-0000-4000-8000-000000000000/codex-home'
New-Item -ItemType Directory -Path $profile,$env:CODEX_HOME | Out-Null
$fixtures = @{
    (Join-Path $data 'profiles.json') = "[]`n"
    (Join-Path $data 'executables.json') = '{"codePath":null,"codexPath":null,"globalCodexHome":null}'
    (Join-Path $profile 'auth.json') = '{"fixture":"private-profile-sentinel"}'
    (Join-Path $profile 'session.jsonl') = '{"fixture":"conversation-preservation"}'
    (Join-Path $env:CODEX_HOME 'auth.json') = '{"fixture":"default-login-sentinel"}'
}
foreach ($entry in $fixtures.GetEnumerator()) { [IO.File]::WriteAllText($entry.Key,$entry.Value) }
function Assert-Fixtures {
    foreach ($entry in $fixtures.GetEnumerator()) {
        if ([IO.File]::ReadAllText($entry.Key) -cne $entry.Value) { throw "Installer/startup modified $($entry.Key)" }
    }
}
# Substitute only release transport. Both published scripts perform their real
# checksum validation and execute the exact candidate's native NSIS installer.
# Keep fixture transport state distinct from installer-local variables: PowerShell
# resolves names dynamically and the invoked script uses $installer for metadata.
$packageName = Split-Path $candidateInstallerPath -Leaf
$candidateChecksumValid = $true
function Invoke-RestMethod {
    param([string]$Uri)
    if ($Uri -ne 'https://api.github.com/repos/wrestle-R/multi-codex/releases/latest') { throw 'Unexpected metadata request' }
    return @{ tag_name = 'candidate-under-test'; assets = @(
        @{ name=$packageName; browser_download_url='https://fixture.invalid/installer' },
        @{ name='SHA256SUMS'; browser_download_url='https://fixture.invalid/sums' }
    ) }
}
function Invoke-WebRequest {
    param([string]$Uri,[string]$OutFile,[switch]$UseBasicParsing)
    if (!$UseBasicParsing) { throw 'Installer downloads must use basic parsing.' }
    if ($Uri -eq 'https://fixture.invalid/installer') { Copy-Item -LiteralPath $candidateInstallerPath -Destination $OutFile; return }
    if ($Uri -ne 'https://fixture.invalid/sums') { throw 'Unexpected asset request' }
    $hash = if ($candidateChecksumValid) { (Get-FileHash -LiteralPath $candidateInstallerPath -Algorithm SHA256).Hash } else { '0' * 64 }
    if (!$OutFile) { throw 'Checksum manifest must be downloaded to a file.' }
    [IO.File]::WriteAllBytes($OutFile, [Text.Encoding]::UTF8.GetBytes("$hash  $packageName`n"))
}
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class LauncherWindows {
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr window, uint msg, IntPtr wParam, IntPtr lParam);
}
'@
function Check-StartMenu {
    $application = Join-Path $install 'multi-codex-desktop.exe'
    $shell = New-Object -ComObject WScript.Shell
    try {
        $programs = [Environment]::GetFolderPath('Programs')
        $links = @(Get-ChildItem -LiteralPath $programs -Filter 'Multi Codex.lnk' -Recurse -File | Where-Object {
            $shell.CreateShortcut($_.FullName).TargetPath -ieq $application
        })
        if ($links.Count -ne 1) { throw 'The installer must create one Start menu shortcut to the installed app.' }
    } finally { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($shell) }
}
function Check-Window {
    $application = Join-Path $install 'multi-codex-desktop.exe'
    $app = Start-Process -FilePath $application -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(45)
        do {
            $app.Refresh()
            if ($app.HasExited) { throw "Packaged Windows app exited with $($app.ExitCode)" }
            if ($app.MainWindowHandle -ne 0 -and [LauncherWindows]::IsWindowVisible($app.MainWindowHandle)) { break }
            Start-Sleep -Milliseconds 250
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($app.MainWindowHandle -eq 0) { throw 'Packaged app has no native window' }
        if (![LauncherWindows]::IsZoomed($app.MainWindowHandle)) { throw 'Launcher did not start maximized' }
        Start-Sleep -Seconds 3
        $app.Refresh()
        if ($app.HasExited) { throw 'Launcher stopped after startup' }
        [void][LauncherWindows]::SendMessage($app.MainWindowHandle,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)
        if (!$app.WaitForExit(15000)) { throw 'Windows close did not exit the launcher' }
        Assert-Fixtures
    } finally { if (!$app.HasExited) { Stop-Process -Id $app.Id -Force; $app.WaitForExit() } }
}
try {
    $scriptsRoot = (Resolve-Path (Join-Path $PSScriptRoot '../../scripts')).Path
    $candidateChecksumValid = $false
    $rejected = $false
    try { & (Join-Path $scriptsRoot 'install-app.ps1') -NoLaunch -InstallDirectory $install }
    catch { if ($_.Exception.Message -notmatch 'checksum verification failed') { throw }; $rejected=$true }
    if (!$rejected -or (Test-Path $install)) { throw 'Checksum mismatch must reject installation before executing it' }
    $candidateChecksumValid = $true
    & (Join-Path $scriptsRoot 'install-app.ps1') -NoLaunch -InstallDirectory $install
    Assert-Fixtures
    Check-StartMenu
    Check-Window
    & (Join-Path $scriptsRoot 'update-app.ps1') -NoLaunch -InstallDirectory $install
    Assert-Fixtures
    Check-StartMenu
    Check-Window
    Check-Window
    $result = @{ passed=$true; install=$true; update=$true; restart=$true; nativeVisibleWindow=$true; startMenuShortcut=$true; maximized=$true; closeExits=$true; checksumMismatchRejected=$true; profileAndConversationFixturesPreserved=$true; unicodeAndSpacedPaths=$true; installerSHA256=(Get-FileHash $candidateInstallerPath -Algorithm SHA256).Hash.ToLower(); authenticatedAccounts='synthetic fixtures; no real credentials'; environment=[Environment]::OSVersion.VersionString; testedAt=[DateTime]::UtcNow.ToString('o') }
    $result | ConvertTo-Json | Set-Content (Join-Path $evidence 'installer-result.json')
    $result | ConvertTo-Json
} finally {
    $env:APPDATA=$oldRoaming; $env:CODEX_HOME=$oldHome
    Remove-Item -LiteralPath $root -Recurse -Force
}
