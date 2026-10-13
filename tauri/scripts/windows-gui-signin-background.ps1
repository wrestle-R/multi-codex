param([string]$Binary,
      [Parameter(Mandatory=$true)][string]$Installer,
      [string]$CandidateEvidence)
$ErrorActionPreference = 'Stop'
$output = Join-Path $env:RUNNER_TEMP 'multi-codex-gui-signin-evidence'
New-Item -ItemType Directory -Force $output | Out-Null
$status = 1
try {
    & (Join-Path $PSScriptRoot 'windows-gui-signin.ps1') -Binary $Binary -Installer $Installer -CandidateEvidence $CandidateEvidence
    $status = 0
} catch {
    # Only exception classes/identifiers are public, never private log text.
    [ordered]@{ source=$env:GITHUB_SHA; exceptionType=$_.Exception.GetType().Name;
        failureId=$_.FullyQualifiedErrorId } | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $output 'background-failure.json')
    Write-Error -Message 'Native GUI acceptance failed; see sanitized result' -ErrorAction Continue
} finally {
    [IO.File]::WriteAllText((Join-Path $output 'exit-status'), "$status")
}
exit $status
