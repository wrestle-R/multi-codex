param([string]$Binary,
      [Parameter(Mandatory=$true)][string]$Installer,
      [string]$CandidateEvidence)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP -or !$env:MULTI_CODEX_TEST_ENGINE -or !$env:MULTI_CODEX_TEST_VSCODE) {
    throw 'Run only on a disposable native runner with explicit pinned executables'
}
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public class ObservedWindow {
    public long Handle;
    public uint ProcessId;
    public string Class;
}
public static class SigninWindows {
    private delegate bool WindowCallback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(WindowCallback callback, IntPtr data);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] private static extern bool BringWindowToTop(IntPtr window);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder name, int count);
    public static ObservedWindow[] Visible() {
        var result = new List<ObservedWindow>();
        EnumWindows((window, data) => {
            if (!IsWindowVisible(window)) return true;
            uint pid; GetWindowThreadProcessId(window, out pid);
            var name = new StringBuilder(256); GetClassName(window, name, name.Capacity);
            result.Add(new ObservedWindow { Handle=window.ToInt64(), ProcessId=pid, Class=name.ToString() });
            return true;
        }, IntPtr.Zero);
        return result.ToArray();
    }
    public static void Focus(long handle) {
        BringWindowToTop(new IntPtr(handle));
        SetForegroundWindow(new IntPtr(handle));
    }
}
'@
$fixtureRoot = Join-Path $env:RUNNER_TEMP ("gui-signin-" + [Guid]::NewGuid().ToString())
$output = Join-Path $env:RUNNER_TEMP 'multi-codex-gui-signin-evidence'
$driver = Join-Path $env:RUNNER_TEMP 'multi-codex-gui-driver'
New-Item -ItemType Directory -Force $fixtureRoot,$output | Out-Null
$baseline = @([SigninWindows]::Visible() | ForEach-Object { $_.Handle })
$baselineBrowserPids = @(Get-Process -Name msedge,chrome,firefox -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
$ownedBrowsers = @{}
$observedConsole = @{}
$observedBrowser = @{}
$samples = 0
$visibleAppSamples = 0
$scriptPath = Join-Path $PSScriptRoot 'windows-gui-signin.mjs'
$installedRoot = Join-Path $fixtureRoot 'installed app with spaces'
$installedBinary = Join-Path $installedRoot 'multi-codex-desktop.exe'
$appPolicyName = 'multi-codex-desktop.exe'
$policyValues = @{
    'HKLM:\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments' = '--remote-debugging-address=127.0.0.1 --remote-debugging-port=19384'
    'HKLM:\Software\Policies\Microsoft\Edge\WebView2\UserDataFolder' = (Join-Path $fixtureRoot 'webview')
}
$savedPolicies = @{}
$child = $null
$result = $null
try {
    $setup = Start-Process -FilePath $Installer -ArgumentList "/S /D=$installedRoot" -Wait -PassThru
    if ($setup.ExitCode -ne 0 -or !(Test-Path -LiteralPath $installedBinary -PathType Leaf)) {
        throw 'Exact candidate NSIS installation failed'
    }
    # Tauri patches only its package-type marker in the NSIS payload and then
    # restores the raw build binary. Require the exact expected packaged bytes.
    if ($CandidateEvidence) {
        $candidate = Get-Content -LiteralPath $CandidateEvidence -Raw | ConvertFrom-Json
        if (!$candidate.passed -or !$candidate.checks.exactSignedCandidateInstalled -or
            $candidate.payloadSha256 -ne (Get-FileHash -LiteralPath $Installer).Hash) {
            throw 'Reused installer differs from the SDK-verified signed candidate'
        }
        $env:MULTI_CODEX_GUI_CANDIDATE_SOURCE = $candidate.candidateSource
    } else {
        if (!$Binary) { throw 'An exact build binary or signed candidate evidence is required' }
        $expectedPackagedDigest = & python (Join-Path $PSScriptRoot 'nsis_candidate_digest.py') $Binary
        if ($LASTEXITCODE -ne 0) { throw 'Could not fingerprint the candidate NSIS executable' }
        if ($expectedPackagedDigest -ne (Get-FileHash -LiteralPath $installedBinary).Hash) {
            throw 'Installed executable differs from the exact candidate build'
        }
    }
    # Elevated WebView2 ignores ENV/HKCU overrides. Apply exact-app HKLM overrides
    # only on this disposable Actions VM, save previous values and always restore.
    # No production flags or wildcard policies are changed.
    foreach ($path in $policyValues.Keys) {
        $existed = Test-Path -LiteralPath $path
        if (!$existed) { New-Item -Path $path -Force | Out-Null }
        $key = Get-Item -LiteralPath $path
        $hadValue = $key.GetValueNames() -contains $appPolicyName
        $savedPolicies[$path] = @{ keyExisted=$existed; hadValue=$hadValue;
            value=$(if ($hadValue) { $key.GetValue($appPolicyName) } else { $null });
            kind=$(if ($hadValue) { $key.GetValueKind($appPolicyName).ToString() } else { 'String' }) }
        New-ItemProperty -LiteralPath $path -Name $appPolicyName -Value $policyValues[$path] -PropertyType String -Force | Out-Null
    }
    $arguments = @($scriptPath,$installedBinary,$fixtureRoot,$output,$driver) | ForEach-Object { '"' + $_ + '"' }
    $child = Start-Process -FilePath (Get-Command node).Source -ArgumentList $arguments -NoNewWindow -PassThru `
        -RedirectStandardOutput (Join-Path $fixtureRoot 'driver-private.log') -RedirectStandardError (Join-Path $fixtureRoot 'driver-private-error.log')
    $deadline = [DateTime]::UtcNow.AddMinutes($(if ($env:GUI_AUTH_PUBLIC_KEY) { 20 } else { 5 }))
    while (!$child.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        $samples++
        foreach ($window in [SigninWindows]::Visible()) {
            if ($baseline -contains $window.Handle) { continue }
            if ($window.Class -in @('ConsoleWindowClass','CASCADIA_HOSTING_WINDOW_CLASS','VirtualConsoleClass')) {
                $observedConsole[$window.Handle] = $true
            }
            $process = Get-Process -Id $window.ProcessId -ErrorAction SilentlyContinue
            if ($process -and $process.ProcessName -eq 'multi-codex-desktop') { $visibleAppSamples++ }
            if ($process -and $process.ProcessName -in @('msedge','chrome','firefox')) {
                $firstObservation = !$observedBrowser.ContainsKey($window.Handle)
                $observedBrowser[$window.Handle] = $true
                if ($firstObservation) {
                    # Opening the real browser can occlude WebView2 while CDP is
                    # awaiting the click response. Return to the app, as a user
                    # does before cancelling/retrying, after observing the browser.
                    foreach ($appWindow in [SigninWindows]::Visible()) {
                        $appProcess = Get-Process -Id $appWindow.ProcessId -ErrorAction SilentlyContinue
                        if ($appProcess -and $appProcess.ProcessName -eq 'multi-codex-desktop') {
                            [SigninWindows]::Focus($appWindow.Handle)
                        }
                    }
                }
                if ($baselineBrowserPids -notcontains $process.Id) {
                    try {
                        $ownedBrowsers[$process.Id] = @{ name=$process.ProcessName; started=$process.StartTime.ToUniversalTime().Ticks }
                    } catch { } # A sampled browser can exit before its identity read.
                }
            }
        }
        if ($samples % 12 -eq 0) {
            [ordered]@{ source=$env:GITHUB_SHA; observationSamples=$samples;
                visibleApplicationSamples=$visibleAppSamples; newVisibleConsoleWindows=$observedConsole.Count;
                actualVisibleBrowserWindows=$observedBrowser.Count } | ConvertTo-Json |
                Set-Content -LiteralPath (Join-Path $output 'window-progress.json')
        }
        Start-Sleep -Milliseconds 75
        $child.Refresh()
    }
    if (!$child.HasExited) { throw 'Real GUI acceptance timed out' }
    $child.WaitForExit()
    $gui = Get-Content -LiteralPath (Join-Path $output 'gui-result.json') -Raw | ConvertFrom-Json
    $result = [ordered]@{
        source = $env:GITHUB_SHA
        platform = 'win32'
        passed = ($gui.passed -and $child.ExitCode -eq 0 -and $observedConsole.Count -eq 0 -and $visibleAppSamples -gt 10 -and $observedBrowser.Count -gt 0)
        nativeGui = $gui
        observationSamples = $samples
        visibleApplicationSamples = $visibleAppSamples
        newVisibleConsoleWindows = $observedConsole.Count
        actualVisibleBrowserWindows = $observedBrowser.Count
        exactNsisInstalledExecutable = $true
        exactTauriNsisMarkerPatchVerified = [bool]$Binary
        reusedSignedCandidatePayloadVerified = [bool]$CandidateEvidence
        candidateSource = $env:MULTI_CODEX_GUI_CANDIDATE_SOURCE
        scopedRunnerWebviewPolicies = $true
        authenticationCompletion = $gui.authenticationCompletion
    }
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'result.json')
    $result | ConvertTo-Json -Depth 8 | Write-Output
    if (!$result.passed) { throw 'Native sign-in GUI or visible-window acceptance failed; see sanitized evidence' }
} finally {
    if ($child -and !$child.HasExited) { & taskkill.exe /PID $child.Id /T /F | Out-Null }
    foreach ($path in $savedPolicies.Keys) {
        $previous = $savedPolicies[$path]
        if ($previous.hadValue) {
            New-ItemProperty -LiteralPath $path -Name $appPolicyName -Value $previous.value -PropertyType $previous.kind -Force | Out-Null
        } else {
            Remove-ItemProperty -LiteralPath $path -Name $appPolicyName -ErrorAction SilentlyContinue
        }
        $key = Get-Item -LiteralPath $path
        if (!$previous.keyExisted -and $key.GetValueNames().Count -eq 0 -and $key.GetSubKeyNames().Count -eq 0) {
            Remove-Item -LiteralPath $path
        }
    }
    # Shell-opened browsers outlive the app and hold their disposable profile
    # cache open. Stop only new observed browser processes with unchanged start
    # identity; never stop a browser that existed before the test.
    foreach ($browserId in $ownedBrowsers.Keys) {
        $browserProcess = Get-Process -Id $browserId -ErrorAction SilentlyContinue
        if ($browserProcess -and $browserProcess.ProcessName -eq $ownedBrowsers[$browserId].name -and
            $browserProcess.StartTime.ToUniversalTime().Ticks -eq $ownedBrowsers[$browserId].started) {
            $stopped = Start-Process -FilePath taskkill.exe -ArgumentList @('/PID',"$browserId",'/T','/F') -Wait -PassThru -WindowStyle Hidden `
                -RedirectStandardOutput (Join-Path $fixtureRoot 'browser-stop-private.log') -RedirectStandardError (Join-Path $fixtureRoot 'browser-stop-private-error.log')
            if ($stopped.ExitCode -ne 0 -and (Get-Process -Id $browserId -ErrorAction SilentlyContinue)) {
                throw 'Could not stop an owned fixture browser'
            }
        }
    }
    $uninstaller = Join-Path $installedRoot 'uninstall.exe'
    if (Test-Path -LiteralPath $uninstaller) { Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait | Out-Null }
    # Exact created fixture only; native account/keyring cleanup runs in the driver.
    $cleanupDeadline = [DateTime]::UtcNow.AddSeconds(15)
    while (Test-Path -LiteralPath $fixtureRoot) {
        try { Remove-Item -LiteralPath $fixtureRoot -Recurse -Force }
        catch {
            if ([DateTime]::UtcNow -ge $cleanupDeadline) { throw }
            Start-Sleep -Milliseconds 100
        }
    }
    if ($result) {
        $result['disposableFixtureCleanup'] = $true
        $result['runnerWebviewPoliciesRestored'] = $true
        $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'result.json')
    }
}
