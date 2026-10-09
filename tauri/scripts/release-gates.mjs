import { createHash } from 'node:crypto'
import { readFileSync, existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../', import.meta.url))
export const requiredChecks = [
  'linux-build', 'macos-build', 'linux-installer', 'macos-installer',
  'linux-isolation', 'macos-isolation',
]
export const crossPlatformChecks = [
  'windows-build', 'windows-installer', 'windows-isolation',
  'extension-linux-x64', 'extension-darwin-arm64', 'extension-darwin-x64', 'extension-win32-x64',
  'launcher-window', 'macos-menu-bar',
]
export const extensionTargets = ['linux-x64', 'darwin-arm64', 'darwin-x64', 'win32-x64']
export const fullValidationChecks = [
  'packaged-macos-26-arm64',
  'packaged-debian-ubuntu', 'packaged-fedora', 'packaged-arch',
  'hyprland-placement', 'gnome-wayland-placement', 'gnome-x11-placement',
  'kde-wayland-placement', 'kde-x11-placement',
]

export function evaluate(report, version, artifactDirectory, { fullValidation = false } = {}) {
  const failures = []
  if (report.version !== version || report.schemaVersion !== 1) failures.push('Validation report version/schema does not match the candidate')
  const [major, minor] = version.split('.').map(Number)
  const crossPlatform = major > 1 || (major === 1 && minor >= 4)
  const checks = [...requiredChecks, ...(crossPlatform ? crossPlatformChecks : [])].map(id => [id, report.releaseChecks?.[id]])
  if (fullValidation) checks.push(...fullValidationChecks.map(id => [id, report.checks?.[id]]))
  for (const [id, check] of checks) {
    if (check?.status !== 'passed') failures.push(`${id}: ${check?.reason || 'functional validation is pending'}`)
    else if (!check.evidence || !(check.environment || check.osVersion) || !check.testedBy || !check.testedAt || Number.isNaN(Date.parse(check.testedAt))) failures.push(`${id}: evidence, environment, tester and timestamp are required`)
  }
  const assets = [
    `Multi.Codex_${version}_amd64.AppImage`,
    `Multi.Codex_${version}_amd64.deb`,
    `Multi.Codex-${version}-1.x86_64.rpm`,
    `Multi.Codex_${version}_aarch64.dmg`,
    'install-app.sh',
    'update-app.sh',
  ]
  if (crossPlatform) {
    const extensionVersion = report.extensionVersion
    if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(extensionVersion ?? '')) failures.push('Matching extension manifest version is required')
    else for (const target of extensionTargets) assets.push(`multi-codex-${target}-${extensionVersion}.vsix`)
    assets.push(`Multi.Codex_${version}_x64-setup.exe`, 'install-app.ps1', 'update-app.ps1')
  }
  for (const name of assets) {
    const expected = report.artifacts?.[name]
    if (!/^[a-f0-9]{64}$/.test(expected ?? '')) failures.push(`${name}: validated artifact SHA256 is missing`)
    if (artifactDirectory) {
      const path = resolve(artifactDirectory, name)
      if (!existsSync(path)) failures.push(`${name}: packaged artifact is missing`)
      else if (createHash('sha256').update(readFileSync(path)).digest('hex') !== expected) failures.push(`${name}: artifact differs from the tested candidate`)
    }
  }
  return failures
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const version = JSON.parse(readFileSync(resolve(root, 'tauri/package.json'), 'utf8')).version
  const report = JSON.parse(readFileSync(resolve(root, `release/evidence/v${version}-validation.json`), 'utf8'))
  const artifactDirectory = process.argv.slice(2).find(argument => argument !== '--full-validation')
  const failures = evaluate(report, version, artifactDirectory, { fullValidation: process.argv.includes('--full-validation') })
  if (failures.length) {
    console.error(`Release v${version} is blocked:\n${failures.map(f => `- ${f}`).join('\n')}`)
    process.exitCode = 1
  } else console.log(`Release v${version}: all required functional checks and artifact hashes passed`)
}
